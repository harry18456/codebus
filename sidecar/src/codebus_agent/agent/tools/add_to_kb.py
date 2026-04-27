"""`add_to_kb` Q&A tool — five-stage pipeline (sanitize → validate → upsert → log).

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order
  Requirement: Q&A budget constants are module-level (per-session / per-question caps)

Per Decision 3 (Pass 3 sanitize source label): we pass
``FileSource(path=chunk.source, pass_="qa_add_to_kb")`` to
``ctx.sanitizer.sanitize`` so the structured ``{"pass": "qa_add_to_kb",
"path": ...}`` audit form lands in `sanitize_audit.jsonl` per
sanitizer-safety-chain conventions.

Per Decision 8 (audit_fields excludes free-text):
``audit_fields = ["source", "reason", "related_stations"]`` — chunk
text already lands on `sanitize_audit.jsonl` (Pass 3) and the Qdrant
point payload, so duplicating it into `tool_audit.jsonl` would create
a parallel audit surface that violates the single-write-path invariant.
"""
from __future__ import annotations

import hashlib
from datetime import datetime, timezone
from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from codebus_agent.agent.qa import (
    _QA_MAX_ADD_TO_KB_PER_QUESTION,
    _QA_MAX_ADD_TO_KB_PER_SESSION,
    _QA_MAX_CHUNK_SIZE_CHARS,
)
from codebus_agent.agent.station_id import _STATION_ID_RE
from codebus_agent.kb.payload import KBPayload
from codebus_agent.sanitizer import RULES_VERSION, FileSource


__all__ = ["AddToKBArgs", "AddToKBChunk", "add_to_kb"]


class AddToKBChunk(BaseModel):
    """One chunk in an `add_to_kb` invocation."""

    model_config = ConfigDict(extra="forbid")

    text: str
    source: str  # `<file>:<line_start>-<line_end>` form per spec
    related_stations: list[str] = Field(default_factory=list)
    line_start: int = Field(default=0, ge=0)
    line_end: int = Field(default=0, ge=0)


class AddToKBArgs(BaseModel):
    """Pydantic schema for the `add_to_kb` tool's arguments."""

    model_config = ConfigDict(extra="forbid")

    chunks: list[AddToKBChunk]
    source: str = ""  # operation-level provenance label (e.g. file path)
    reason: str = ""  # zh-TW reason for persistence


_AUDIT_FIELDS: list[str] = ["source", "reason", "related_stations"]


def _validate_station_ids(related_stations: list[str]) -> str | None:
    for sid in related_stations:
        if not isinstance(sid, str) or not _STATION_ID_RE.fullmatch(sid):
            return sid
    return None


async def add_to_kb(args: AddToKBArgs, ctx: Any) -> str:
    """Drive the five-stage pipeline for each chunk.

    Stage order is fixed: budget check → sanitize (Pass 3) → validate
    related_stations → KB upsert → growth log. Per spec, this order
    MUST NOT be reordered: validate-before-sanitize would emit audit
    lines for chunks the call ultimately rejects, while skip-empty
    must observe sanitize result first.
    """
    state = getattr(ctx, "qa_state", None)
    session_id = getattr(ctx, "session_id", None) or "qa-sess-unknown"
    question = getattr(ctx, "question", None)
    originating_station_id = getattr(ctx, "originating_station_id", None)

    sanitizer = getattr(ctx, "sanitizer", None)
    sanitizer_audit = getattr(ctx, "sanitizer_audit", None)
    kb = getattr(ctx, "kb", None)
    growth_logger = getattr(ctx, "kb_growth_logger", None)

    if sanitizer is None or sanitizer_audit is None or kb is None or growth_logger is None:
        return (
            "add_to_kb unavailable: ctx.sanitizer / ctx.sanitizer_audit / "
            "ctx.kb / ctx.kb_growth_logger MUST all be configured"
        )

    # Per-question budget check (state may be missing in unit tests; treat absent
    # state as zero-count so test fixtures don't need to thread state through).
    question_count = getattr(state, "add_to_kb_question_count", 0) if state else 0
    session_count = getattr(state, "add_to_kb_session_count", 0) if state else 0

    if session_count >= _QA_MAX_ADD_TO_KB_PER_SESSION:
        return (
            f"budget exhausted: per-session add_to_kb cap "
            f"{_QA_MAX_ADD_TO_KB_PER_SESSION} reached"
        )
    if question_count >= _QA_MAX_ADD_TO_KB_PER_QUESTION:
        return (
            f"budget exhausted: per-question add_to_kb cap "
            f"{_QA_MAX_ADD_TO_KB_PER_QUESTION} reached"
        )

    # Pre-validate station ids across ALL chunks before any sanitize runs —
    # spec Requirement: invalid station id aborts the whole invocation
    # before upsert. (Earlier chunks in the same call that already
    # committed are not transactional; here we have processed none yet.)
    for chunk in args.chunks:
        bad = _validate_station_ids(chunk.related_stations)
        if bad is not None:
            return f"invalid station_id: {bad}"

    rules_version = RULES_VERSION

    emitter = getattr(ctx, "emitter", None)

    response_tokens: list[str] = []
    for chunk in args.chunks:
        # Stage 1: Sanitize Pass 3 + write audit lines.
        result = sanitizer.sanitize(
            chunk.text,
            source=FileSource(path=chunk.source, pass_="qa_add_to_kb"),
        )
        for entry in result.entries:
            sanitizer_audit.append(
                entry=entry,
                pass_num=3,
                rules_version=rules_version,
                session_id=session_id,
            )

        clean = result.text
        # Stage 1.5: empty post-sanitize → skip without KB / growth log writes.
        if not clean.strip():
            response_tokens.append("skipped_empty")
            continue

        # Stage 1.6: oversize post-sanitize chunk → reject without writes.
        if len(clean) > _QA_MAX_CHUNK_SIZE_CHARS:
            response_tokens.append(
                f"skipped_oversize: chunk text > {_QA_MAX_CHUNK_SIZE_CHARS} chars"
            )
            continue

        # Stage 2: validate related_stations was already done above.
        # Stage 3: KB upsert.
        text_hash = hashlib.sha256(clean.strip().encode("utf-8")).hexdigest()
        payload = KBPayload(
            source_kind="code",
            file_path=chunk.source.split(":", 1)[0] if ":" in chunk.source else chunk.source,
            line_start=chunk.line_start,
            line_end=chunk.line_end,
            text=clean,
            text_hash=text_hash,
            added_by="qa_agent",
            session_id=session_id,
            chunk_index=0,
            chunk_total=1,
            created_at=datetime.now(timezone.utc),
            related_stations=list(chunk.related_stations),
        )
        outcome, real_point_id = await kb.upsert_chunk(clean, payload=payload)
        dedup_skipped = outcome.startswith("dedup_")
        # Stage 4: growth log — written for both new and dedup-skipped paths.
        # `entry_id` MUST be the real Qdrant point id (never a sentinel)
        # so Trust Layer R-01 can join `kb_growth.jsonl` rows back to KB.
        growth_logger.write(
            point_id=real_point_id,
            source=chunk.source,
            reason=args.reason,
            related_stations=list(chunk.related_stations),
            originating_station_id=originating_station_id,
            sanitize_stats={"hits": len(result.entries)},
            chunk_size_chars=len(clean),
            dedup_skipped=dedup_skipped,
            session_id=session_id,
            question=question,
        )
        # SSE event: emit `kb_growth` only on new-point writes; dedup
        # MUST NOT emit per spec scenario `kb_growth event omitted on
        # dedup skip`.
        if not dedup_skipped and emitter is not None:
            try:
                emitter.emit(
                    {
                        "type": "kb_growth",
                        "entry_id": real_point_id,
                        "source": chunk.source,
                        "related_stations": list(chunk.related_stations),
                        "originating_station_id": originating_station_id,
                    }
                )
            except Exception:
                # Emitter failures must not break the tool body.
                pass

        # Track per-question / per-session counts so subsequent chunks
        # in the same loop honor the budget.
        if state is not None:
            try:
                state.add_to_kb_question_count = question_count + 1
                state.add_to_kb_session_count = session_count + 1
                question_count += 1
                session_count += 1
            except Exception:
                pass

        response_tokens.append(outcome)

    return ", ".join(response_tokens) if response_tokens else "no chunks processed"


add_to_kb.audit_fields = _AUDIT_FIELDS  # type: ignore[attr-defined]
