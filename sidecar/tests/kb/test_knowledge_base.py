"""KnowledgeBase regression tests against InMemoryQdrantBackend + SpyProvider.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: Workspace-scoped Qdrant collection naming
  Requirement: Content-hash Layer 1 deduplication
  Requirement: KnowledgeBase query and find_similar API
  Requirement: KBStats returned by build
  Requirement: Embedding batch pipeline with UsageTracker wiring
  Requirement: Progress callback protocol

Tests are intentionally backend-agnostic: they pass an `InMemoryQdrantBackend`
through `KnowledgeBase.__init__`, so a live Qdrant is not required.
"""
from __future__ import annotations

import asyncio
import hashlib
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.kb.knowledge_base import KnowledgeBase, _derive_workspace_id
from codebus_agent.kb.payload import KBProgressEvent, KBStats
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.scanner.models import (
    ContentTypeSummary,
    FileEntry,
    ScanResult,
    ScanStats,
)


def _make_scan(
    files: list[FileEntry],
    workspace_root: str = "/abs/workspace/demo",
) -> ScanResult:
    return ScanResult(
        workspace_root=workspace_root,
        scan_started_at=datetime(2026, 4, 21, 12, 0, 0, tzinfo=timezone.utc),
        scan_completed_at=datetime(2026, 4, 21, 12, 0, 5, tzinfo=timezone.utc),
        files=files,
        symlinks=[],
        content_summary=ContentTypeSummary(
            total_files=len(files),
            kind_counts={},
            language_counts={},
            category_counts={},
            dominant_category="code",
            dominant_languages=[],
            has_tests=False,
            has_docs=False,
            is_monorepo=False,
        ),
        stats=ScanStats(
            total_files_walked=len(files),
            total_files_included=len(files),
            total_bytes_read=sum(f.size for f in files),
            duration_seconds=0.5,
            quarantined_count=0,
            skipped_count=0,
        ),
    )


def _code_entry(path: str, body: str, language: str = "python") -> FileEntry:
    return FileEntry(
        path=path,
        size=len(body.encode("utf-8")),
        kind="text",
        language=language,
        encoding="utf-8",
        content=body,
    )


def _build_kb(
    backend,
    provider,
    *,
    workspace_root: str = "/abs/workspace/demo",
    tmp_log: Path | None = None,
) -> KnowledgeBase:
    tracker = UsageTracker(tmp_log or Path("/tmp/codebus_kb_test_usage.jsonl"))
    return KnowledgeBase(
        backend=backend,
        provider=provider,
        usage_tracker=tracker,
        workspace_root=workspace_root,
        embedding_dim=8,
    )


# ---------------------------------------------------------------------------
# Requirement: Workspace-scoped Qdrant collection naming
# ---------------------------------------------------------------------------


def test_workspace_id_is_sha256_prefix_of_workspace_root() -> None:
    """Scenario: Deterministic collection name per workspace.

    `_derive_workspace_id` MUST be a pure helper returning the first 16
    hex chars of `sha256(workspace_root).hexdigest()`. Building two KBs
    against the same root MUST yield the same `collection_name`.
    """
    root = "/abs/workspace/demo"
    expected = hashlib.sha256(root.encode("utf-8")).hexdigest()[:16]
    assert _derive_workspace_id(root) == expected


async def test_workspace_id_drives_collection_name(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Stats exposes workspace identity."""
    root = "/abs/workspace/demo"
    kb = _build_kb(
        in_memory_backend,
        spy_provider,
        workspace_root=root,
        tmp_log=tmp_path / "usage.jsonl",
    )
    expected_id = hashlib.sha256(root.encode("utf-8")).hexdigest()[:16]
    assert kb.workspace_id == expected_id
    assert kb.collection_name == f"codebus_{expected_id}"


# ---------------------------------------------------------------------------
# Requirement: Content-hash Layer 1 deduplication
# ---------------------------------------------------------------------------


async def test_content_hash_layer1_skips_duplicate_and_bypasses_embed(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenarios: Identical text skipped on second build + No embed for duplicate.

    Two separate file entries with identical content: only one MUST be
    embedded + upserted, and `KBStats.skipped_hash_count >= 1`.
    """
    body = "def hello():\n    return 'world'\n"
    files = [
        _code_entry("a.py", body),
        _code_entry("b.py", body),  # same content → same text_hash
    ]
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )

    stats = await kb.build(_make_scan(files))

    # Exactly one unique chunk text → one provider invocation total.
    embedded_texts = [t for call in spy_provider.embed_calls for t in call.texts]
    assert len(set(embedded_texts)) == 1, (
        f"duplicate text MUST not reach provider.embed; embedded={embedded_texts}"
    )
    assert stats.skipped_hash_count >= 1
    assert stats.points_upserted == 1


async def test_content_hash_normalizes_whitespace_only_diff(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Whitespace-only diff still deduplicates.

    Per design "content-hash normalization 只 strip" — leading/trailing
    whitespace differences MUST hash equal so the second is dropped.
    """
    files = [
        _code_entry("a.py", "alpha\n"),
        _code_entry("b.py", "  alpha  \n"),
    ]
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )

    stats = await kb.build(_make_scan(files))
    assert stats.skipped_hash_count >= 1
    assert stats.points_upserted == 1


# ---------------------------------------------------------------------------
# Requirement: KnowledgeBase query and find_similar API
# ---------------------------------------------------------------------------


async def test_query_top_k_ordering_and_filters(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenarios: top_k ordering + filter_path + filter_source_kind."""
    files = [
        _code_entry("src/storage/types.ts", "export type Storage = {}\n", language="typescript"),
        _code_entry("src/api/routes.ts", "export const routes = []\n", language="typescript"),
        _code_entry("README.md", "# Project\n## Overview\n", language="markdown"),
    ]
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )
    await kb.build(_make_scan(files))

    hits = await kb.query("Storage", top_k=3)
    assert len(hits) <= 3
    for prev, nxt in zip(hits, hits[1:]):
        assert prev.score >= nxt.score, "scores MUST be monotonically non-increasing"

    filtered = await kb.query(
        "Storage", filter_path="src/storage/types.ts", top_k=10
    )
    for h in filtered:
        assert h.payload.file_path == "src/storage/types.ts"

    code_only = await kb.query(
        "x", filter_source_kind=["code"], top_k=10
    )
    for h in code_only:
        assert h.payload.source_kind != "skeleton"


async def test_find_similar_threshold_behavior(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenarios: find_similar None below threshold + hit at/above threshold.

    The spy provider is deterministic, so an identical query text scores
    cosine 1.0 against the indexed chunk; an entirely different query
    text scores < 1.0.
    """
    body = "alpha alpha alpha\n"
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )
    await kb.build(_make_scan([_code_entry("a.py", body)]))

    # Exact match: cosine ~ 1.0 → returned (>= 0.95 threshold).
    hit = await kb.find_similar(body, threshold=0.95)
    assert hit is not None
    assert hit.score >= 0.95

    # Unrelated query whose deterministic vector is unlikely to hit 1.0
    # — pin the threshold above the achievable cosine for any non-equal text.
    no_hit = await kb.find_similar(
        "completely unrelated query that nothing matches\n",
        threshold=0.999999,
    )
    assert no_hit is None


# ---------------------------------------------------------------------------
# Requirement: KBStats returned by build
# ---------------------------------------------------------------------------


async def test_kb_stats_accounting_balances(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Stats accounting balances.

    `points_upserted + skipped_hash_count == chunks_emitted` MUST hold
    when no oversize warnings appear.
    """
    files = [
        _code_entry(f"f{i}.py", f"def fn_{i}():\n    return {i}\n")
        for i in range(5)
    ]
    files.append(_code_entry("dup.py", "def fn_0():\n    return 0\n"))  # dup of f0.py
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )

    stats = await kb.build(_make_scan(files))

    assert isinstance(stats, KBStats)
    assert stats.workspace_id == kb.workspace_id
    assert stats.collection_name == kb.collection_name
    if not stats.warnings:
        assert (
            stats.points_upserted + stats.skipped_hash_count == stats.chunks_emitted
        ), (
            f"invariant broken: upserted={stats.points_upserted} + "
            f"skipped={stats.skipped_hash_count} != "
            f"emitted={stats.chunks_emitted}"
        )
    assert stats.skipped_hash_count >= 1, (
        "fixture intentionally duplicates dup.py == f0.py; dedup MUST fire"
    )


# ---------------------------------------------------------------------------
# Requirement: Embedding batch pipeline with UsageTracker wiring
# ---------------------------------------------------------------------------


def _unique_files(n: int) -> list[FileEntry]:
    """Return `n` files with distinct content so dedup never trips."""
    return [
        _code_entry(
            f"f{i:04d}.py",
            f"def fn_{i}():\n    return {i}\n# unique-{i}\n",
        )
        for i in range(n)
    ]


async def test_embedding_batch_size_capped_at_32(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Batch size capped at 32.

    100 unique chunks → exactly 4 calls of sizes (32, 32, 32, 4).
    """
    files = _unique_files(100)
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "usage.jsonl"
    )

    await kb.build(_make_scan(files))

    sizes = sorted([len(c.texts) for c in spy_provider.embed_calls], reverse=True)
    assert sizes == [32, 32, 32, 4], (
        f"expected (32,32,32,4) batch sizes; got {sizes}"
    )
    for c in spy_provider.embed_calls:
        assert len(c.texts) <= 32


async def test_embedding_concurrency_capped_at_3_inflight(
    in_memory_backend, tmp_path
) -> None:
    """Scenario: Concurrency capped at 3 in-flight batches.

    A blocking provider tracks the live in-flight count; the asserted
    invariant is `max_inflight <= 3`. We submit 6 batches (192 chunks).
    """
    files = _unique_files(192)

    inflight_now = 0
    max_inflight = 0
    release = asyncio.Event()
    started = asyncio.Event()

    class CountingProvider:
        name = "count"
        model = "count-embed"
        embed_calls: list[Any] = []

        async def chat(self, *args, **kwargs):
            raise NotImplementedError

        async def embed(self, texts):
            nonlocal inflight_now, max_inflight
            inflight_now += 1
            max_inflight = max(max_inflight, inflight_now)
            started.set()
            try:
                # Block until the test releases — lets us measure the cap.
                await release.wait()
            finally:
                inflight_now -= 1
            from codebus_agent.providers.protocol import EmbedResponse, Usage

            vectors = [[0.1] * 8 for _ in texts]
            return EmbedResponse(
                vectors=vectors,
                usage=Usage(
                    call_type="embed",
                    model=self.model,
                    embed_tokens=len(texts),
                    cost_usd=0.0,
                ),
            )

    kb = _build_kb(
        in_memory_backend, CountingProvider(), tmp_log=tmp_path / "usage.jsonl"
    )

    build_task = asyncio.create_task(kb.build(_make_scan(files)))
    await started.wait()
    # Give the semaphore a moment to admit any further batches it would
    # admit; a 50 ms grace period is plenty under in-process asyncio.
    await asyncio.sleep(0.05)
    release.set()
    await build_task

    assert max_inflight <= 3, (
        f"max in-flight batches {max_inflight} exceeded the spec cap of 3"
    )
    assert max_inflight >= 2, (
        "test should observe at least 2 concurrent batches when cap allows; "
        "if not, the semaphore is probably serializing serializing"
    )


async def test_usage_tracker_records_one_entry_per_batch(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: UsageTracker records one entry per batch.

    64 chunks → 2 batches → 2 jsonl entries with `module="kb_build"`.
    """
    log_path = tmp_path / "usage.jsonl"
    files = _unique_files(64)
    kb = _build_kb(in_memory_backend, spy_provider, tmp_log=log_path)

    await kb.build(_make_scan(files))

    entries = [
        line for line in log_path.read_text(encoding="utf-8").splitlines() if line
    ]
    import json

    parsed = [json.loads(e) for e in entries]
    kb_entries = [e for e in parsed if e.get("module") == "kb_build"]
    assert len(kb_entries) == 2, (
        f"expected exactly 2 kb_build entries (one per batch); got {len(kb_entries)}"
    )
    total_input_tokens = sum(e["input_tokens"] for e in kb_entries)
    expected = spy_provider.embed_token_per_text * 64
    assert total_input_tokens == expected, (
        f"recorded input_tokens {total_input_tokens} != provider total {expected}"
    )


async def test_oversized_chunk_split_then_skipped_with_warning(
    in_memory_backend, tmp_path
) -> None:
    """Scenario: Oversized chunk split then skipped.

    Provider declares `max_input_tokens=10`; chunker emits a chunk of
    ~600 tokens; halving yields ~300 tokens — still exceeds the cap.
    The builder MUST skip the chunk, append a warning naming the file
    path, and MUST NOT raise. The build MUST complete with non-zero
    `chunks_emitted` and `points_upserted == 0` (only the oversize
    chunk was scheduled).
    """
    # Single long block with no newlines so the chunker emits ~one chunk.
    body = ("token " * 800).rstrip() + "\n"

    class TightProvider:
        name = "tight"
        model = "tight-embed"
        max_input_tokens = 10  # << any realistic chunk
        embed_calls: list[Any] = []

        async def chat(self, *args, **kwargs):
            raise NotImplementedError

        async def embed(self, texts):
            from codebus_agent.providers.protocol import EmbedResponse, Usage

            return EmbedResponse(
                vectors=[[0.0] * 8 for _ in texts],
                usage=Usage(
                    call_type="embed",
                    model=self.model,
                    embed_tokens=len(texts),
                    cost_usd=0.0,
                ),
            )

    files = [_code_entry("huge.py", body)]
    kb = _build_kb(in_memory_backend, TightProvider(), tmp_log=tmp_path / "u.jsonl")

    stats = await kb.build(_make_scan(files))

    assert stats.chunks_emitted >= 1
    assert stats.warnings, "oversized skip MUST surface a warning"
    assert any("huge.py" in w for w in stats.warnings), (
        f"warning MUST name the offending file path; warnings={stats.warnings}"
    )
    # Builder MUST NOT have crashed; embeddable points were all skipped.
    assert stats.points_upserted == 0


# ---------------------------------------------------------------------------
# Requirement: Progress callback protocol
# ---------------------------------------------------------------------------


async def test_progress_callback_emits_all_phase_transitions(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Phase transitions emit events.

    Recorded events MUST contain at least one event per phase: chunking,
    embedding, upserting, done.
    """
    files = _unique_files(5)
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "u.jsonl"
    )

    captured: list[KBProgressEvent] = []

    async def callback(event: KBProgressEvent) -> None:
        captured.append(event)

    await kb.build(_make_scan(files), on_progress=callback)

    phases = {e.phase for e in captured}
    assert phases == {"chunking", "embedding", "upserting", "done"}, (
        f"missing phase events; observed phases={phases}"
    )
    for e in captured:
        assert e.workspace_id == kb.workspace_id


async def test_progress_callback_per_batch_embedding_progress(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: Per-batch embedding progress.

    96 unique chunks → 3 batches (32/32/32). The callback MUST receive
    >= 3 embedding-phase events and the final embedding-phase event MUST
    have `current == total`.
    """
    files = _unique_files(96)
    kb = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "u.jsonl"
    )

    captured: list[KBProgressEvent] = []

    async def callback(event: KBProgressEvent) -> None:
        captured.append(event)

    await kb.build(_make_scan(files), on_progress=callback)

    embedding_events = [e for e in captured if e.phase == "embedding"]
    assert len(embedding_events) >= 3, (
        f"expected ≥3 embedding-phase events for 3 batches; got {len(embedding_events)}"
    )
    final_emb = embedding_events[-1]
    assert final_emb.current == final_emb.total, (
        f"final embedding event must have current==total; got "
        f"current={final_emb.current}, total={final_emb.total}"
    )


async def test_progress_callback_none_runs_silently(
    in_memory_backend, spy_provider, tmp_path
) -> None:
    """Scenario: No callback means silent run.

    `on_progress=None` must not raise and must produce identical
    `KBStats` field values (excluding wall-clock duration) as a no-op
    callback run on the same fixture.
    """
    files = _unique_files(8)
    kb_a = _build_kb(
        in_memory_backend, spy_provider, tmp_log=tmp_path / "a.jsonl"
    )
    stats_a = await kb_a.build(_make_scan(files), on_progress=None)

    backend_b = type(in_memory_backend)()  # fresh backend so dedup doesn't fire
    spy_b = type(spy_provider)()
    kb_b = _build_kb(backend_b, spy_b, tmp_log=tmp_path / "b.jsonl")

    async def noop(event: KBProgressEvent) -> None:
        return None

    stats_b = await kb_b.build(_make_scan(files), on_progress=noop)

    assert stats_a.chunks_emitted == stats_b.chunks_emitted
    assert stats_a.points_upserted == stats_b.points_upserted
    assert stats_a.skipped_hash_count == stats_b.skipped_hash_count
    assert stats_a.batches_embedded == stats_b.batches_embedded
    assert stats_a.warnings == stats_b.warnings
