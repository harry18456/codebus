"""KnowledgeBase build pipeline + query / find_similar surface.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: KnowledgeBase query and find_similar API
  Requirement: Workspace-scoped Qdrant collection naming
  Requirement: Content-hash Layer 1 deduplication
  Requirement: Embedding batch pipeline with UsageTracker wiring
  Requirement: Progress callback protocol
  Requirement: KBStats returned by build
"""
from __future__ import annotations

import asyncio
import hashlib
import time
import uuid
from datetime import datetime, timezone
from typing import Any

import tiktoken

from codebus_agent.kb.backend import KBQdrantBackend
from codebus_agent.kb.chunker import dispatch_for_file_entry
from codebus_agent.kb.payload import (
    ChunkDraft,
    KBHit,
    KBPayload,
    KBProgressEvent,
    KBStats,
    ProgressCallback,
    SourceKind,
)
from codebus_agent.providers.protocol import LLMProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.scanner.models import FileEntry, ScanResult

_BATCH_SIZE = 32
_INFLIGHT_LIMIT = 3
_TOKEN_ENCODING = "cl100k_base"


def _provider_max_input_tokens(provider: LLMProvider) -> int | None:
    """Return the provider's declared input cap, or `None` if unbounded.

    Per spec `Embedding batch pipeline with UsageTracker wiring`: the
    builder consults this attribute to decide whether a chunk needs the
    halve-then-skip fallback. Providers that don't declare a cap (M1
    `MockProvider`) opt out of the check.
    """
    return getattr(provider, "max_input_tokens", None)


def _halve_text(text: str) -> tuple[str, str] | None:
    """Split `text` into two halves at the nearest line boundary in the
    first half; return `(first, second)` or `None` if no split is sensible.
    """
    if not text:
        return None
    midpoint = len(text) // 2
    cut = text.rfind("\n", 0, midpoint + 1)
    if cut <= 0:
        # Fallback: pure character midpoint when no newline lands in the
        # first half. Returning the same text twice would loop forever.
        if midpoint <= 0:
            return None
        return text[:midpoint], text[midpoint:]
    return text[: cut + 1], text[cut + 1 :]


def _derive_workspace_id(workspace_root: str) -> str:
    """Pure helper — `sha256(workspace_root).hexdigest()[:16]`.

    Per design `workspace_id 算法`: `workspace_root` is treated as an
    opaque byte string; callers MUST pre-resolve to an absolute path so
    different mount points don't collide.
    """
    return hashlib.sha256(workspace_root.encode("utf-8")).hexdigest()[:16]


def _normalize_text(text: str) -> str:
    """Per design `content-hash normalization 只 strip` — strip only."""
    return text.strip()


def _hash_text(text: str) -> str:
    return hashlib.sha256(_normalize_text(text).encode("utf-8")).hexdigest()


def _source_kind_for(file_entry: FileEntry, draft: ChunkDraft) -> SourceKind:
    if "skeleton" in draft.flags:
        return "skeleton"
    if file_entry.kind == "text" and file_entry.language in {
        "markdown", "rst", "asciidoc", "plaintext"
    }:
        return "doc"
    return "code"


class KnowledgeBase:
    """Workspace-bound vector index façade over a `KBQdrantBackend`.

    Per design `KnowledgeBase 建構時綁定 workspace，不採全域 singleton`:
    each instance binds to one workspace at construction; collection
    provisioning and payload-index creation happen lazily on first
    backend use so `__init__` stays synchronous.
    """

    def __init__(
        self,
        *,
        backend: KBQdrantBackend,
        provider: LLMProvider,
        usage_tracker: UsageTracker,
        workspace_root: str,
        embedding_dim: int,
    ) -> None:
        self._backend = backend
        self._provider = provider
        self._tracker = usage_tracker
        self._workspace_root = workspace_root
        self._embedding_dim = embedding_dim
        self.workspace_id = _derive_workspace_id(workspace_root)
        self.collection_name = f"codebus_{self.workspace_id}"
        self._indices_ready = False

    async def _ensure_ready(self) -> None:
        if self._indices_ready:
            return
        await self._backend.ensure_indices(self.collection_name)
        self._indices_ready = True

    async def build(
        self,
        scan_result: ScanResult,
        *,
        on_progress: ProgressCallback | None = None,
    ) -> KBStats:
        await self._ensure_ready()
        start_t = time.monotonic()

        # ---- Phase: chunking -------------------------------------------
        per_file_drafts: list[tuple[FileEntry, list[ChunkDraft]]] = []
        for file_entry in scan_result.files:
            drafts = dispatch_for_file_entry(file_entry)
            if drafts:
                per_file_drafts.append((file_entry, drafts))

        chunks_emitted = sum(len(d) for _, d in per_file_drafts)
        await self._emit(
            on_progress,
            KBProgressEvent(
                phase="chunking",
                current=chunks_emitted,
                total=chunks_emitted,
                workspace_id=self.workspace_id,
            ),
        )

        # ---- Dim-mismatch guard (D-032 decision 4) ---------------------
        # Check BEFORE embedding so a collection/model mismatch fails
        # loudly without burning any OpenAI API calls. Happens after
        # chunking (which is local / free) so the caller sees how big the
        # build would have been. Skip when no chunks were produced —
        # empty workspace has nothing to collide with.
        if chunks_emitted > 0:
            await self._backend.ensure_collection(
                self.collection_name, expected_dim=self._embedding_dim
            )

        # ---- Hash + dedup ----------------------------------------------
        embeddable: list[tuple[FileEntry, ChunkDraft, str]] = []
        warnings: list[str] = []
        skipped_hash = 0
        seen_in_run: set[str] = set()

        max_input = _provider_max_input_tokens(self._provider)
        enc = tiktoken.get_encoding(_TOKEN_ENCODING) if max_input else None

        for file_entry, drafts in per_file_drafts:
            total = len(drafts)
            for idx, draft in enumerate(drafts):
                draft.chunk_index = idx
                draft.chunk_total = total
                text_hash = _hash_text(draft.text)
                if text_hash in seen_in_run:
                    skipped_hash += 1
                    continue
                if await self._backend.exists_by_hash(
                    self.collection_name, text_hash
                ):
                    skipped_hash += 1
                    seen_in_run.add(text_hash)
                    continue
                seen_in_run.add(text_hash)

                if (
                    max_input is not None
                    and enc is not None
                    and "skeleton" not in draft.flags
                    and len(enc.encode(draft.text)) > max_input
                ):
                    halved = _halve_text(draft.text)
                    if halved is not None and all(
                        len(enc.encode(part)) <= max_input for part in halved
                    ):
                        # Halving brought both pieces under the cap — embed both.
                        for part in halved:
                            part_hash = _hash_text(part)
                            if part_hash in seen_in_run:
                                skipped_hash += 1
                                continue
                            seen_in_run.add(part_hash)
                            sub_draft = ChunkDraft(
                                text=part,
                                line_start=draft.line_start,
                                line_end=draft.line_end,
                                token_count=len(enc.encode(part)),
                                chunk_index=draft.chunk_index,
                                chunk_total=draft.chunk_total,
                                flags=list(draft.flags),
                            )
                            embeddable.append((file_entry, sub_draft, part_hash))
                        continue

                    # Halving didn't help (or wasn't possible) — skip + warn.
                    warnings.append(
                        f"oversized chunk skipped in {file_entry.path}: "
                        f"token_count exceeds provider max_input_tokens={max_input}"
                    )
                    continue

                embeddable.append((file_entry, draft, text_hash))

        # ---- Phase: embedding (batched, in-flight capped) --------------
        await self._emit(
            on_progress,
            KBProgressEvent(
                phase="embedding",
                current=0,
                total=len(embeddable),
                workspace_id=self.workspace_id,
            ),
        )

        all_points: list[dict[str, Any]] = []
        batches_embedded = 0
        prompt_tokens_total = 0

        if embeddable:
            batches = [
                embeddable[i : i + _BATCH_SIZE]
                for i in range(0, len(embeddable), _BATCH_SIZE)
            ]
            sem = asyncio.Semaphore(_INFLIGHT_LIMIT)
            done_counter = 0

            async def _run_batch(batch):
                nonlocal done_counter, batches_embedded, prompt_tokens_total
                async with sem:
                    # Embed the raw chunk text (not the normalized form):
                    # normalization is dedup-only per spec "Content-hash
                    # Layer 1 deduplication". A `find_similar(text)` call
                    # passes raw text to `provider.embed`, so storing the
                    # raw form keeps query and indexed vectors aligned.
                    texts = [item[1].text for item in batch]
                    response = await self._provider.embed(texts)
                    self._tracker.record(
                        usage=response.usage, module="kb_build"
                    )
                    batches_embedded += 1
                    prompt_tokens_total += int(response.usage.embed_tokens)
                    points = []
                    for (file_entry, draft, text_hash), vector in zip(
                        batch, response.vectors
                    ):
                        points.append(
                            self._build_point(file_entry, draft, text_hash, vector)
                        )
                    done_counter += len(batch)
                    await self._emit(
                        on_progress,
                        KBProgressEvent(
                            phase="embedding",
                            current=done_counter,
                            total=len(embeddable),
                            workspace_id=self.workspace_id,
                        ),
                    )
                    return points

            results = await asyncio.gather(*[_run_batch(b) for b in batches])
            for batch_points in results:
                all_points.extend(batch_points)

        # ---- Phase: skeleton points (no embed needed) ------------------
        skeleton_points: list[dict[str, Any]] = []
        for file_entry, drafts in per_file_drafts:
            for draft in drafts:
                if "skeleton" not in draft.flags:
                    continue
                text_hash = _hash_text(draft.text)
                # Skeleton chunks share the empty-string hash; if we've
                # already upserted one in this run we still keep it
                # (each skeleton refers to a distinct file_path).
                skeleton_points.append(
                    self._build_point(
                        file_entry, draft, text_hash, [0.0] * self._embedding_dim
                    )
                )

        # ---- Phase: upserting ------------------------------------------
        merged = all_points + skeleton_points
        await self._emit(
            on_progress,
            KBProgressEvent(
                phase="upserting",
                current=0,
                total=len(merged),
                workspace_id=self.workspace_id,
            ),
        )
        if merged:
            await self._backend.upsert_points(self.collection_name, merged)

        # Skeleton points count toward both upserted and chunks_emitted but
        # MUST NOT inflate skipped_hash_count — they were never deduplicated.
        points_upserted = len(merged)

        await self._emit(
            on_progress,
            KBProgressEvent(
                phase="done",
                current=points_upserted,
                total=chunks_emitted,
                workspace_id=self.workspace_id,
            ),
        )

        return KBStats(
            chunks_emitted=chunks_emitted,
            points_upserted=points_upserted,
            skipped_hash_count=skipped_hash,
            batches_embedded=batches_embedded,
            prompt_tokens_total=prompt_tokens_total,
            warnings=warnings,
            duration_seconds=max(0.0, time.monotonic() - start_t),
            workspace_id=self.workspace_id,
            collection_name=self.collection_name,
        )

    def _build_point(
        self,
        file_entry: FileEntry,
        draft: ChunkDraft,
        text_hash: str,
        vector: list[float],
    ) -> dict[str, Any]:
        payload = KBPayload(
            source_kind=_source_kind_for(file_entry, draft),
            file_path=file_entry.path,
            line_start=draft.line_start,
            line_end=draft.line_end,
            text=draft.text,
            text_hash=text_hash,
            language=file_entry.language,
            added_by="scanner",
            chunk_index=draft.chunk_index,
            chunk_total=draft.chunk_total,
            created_at=datetime.now(timezone.utc),
            sanitize_stats=dict(file_entry.sanitize_stats),
        )
        return {"id": str(uuid.uuid4()), "vector": vector, "payload": payload}

    @staticmethod
    async def _emit(
        callback: ProgressCallback | None, event: KBProgressEvent
    ) -> None:
        if callback is None:
            return
        await callback(event)

    # -- Query API ----------------------------------------------------------

    async def query(
        self,
        text: str,
        *,
        top_k: int = 8,
        filter_path: str | None = None,
        filter_source_kind: list[str] | None = None,
    ) -> list[KBHit]:
        await self._ensure_ready()
        response = await self._provider.embed([text])
        vector = response.vectors[0]

        query_filter: dict[str, Any] = {}
        if filter_path is not None:
            query_filter["file_path"] = filter_path
        if filter_source_kind:
            query_filter["source_kind"] = list(filter_source_kind)

        return await self._backend.search_points(
            self.collection_name,
            vector,
            limit=top_k,
            query_filter=query_filter or None,
        )

    async def find_similar(
        self,
        text: str,
        *,
        threshold: float = 0.95,
    ) -> KBHit | None:
        hits = await self.query(text, top_k=1)
        if not hits:
            return None
        top = hits[0]
        if top.score < threshold:
            return None
        return top


__all__ = ["KnowledgeBase", "_derive_workspace_id"]
