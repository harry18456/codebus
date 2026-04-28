"""Tests for `add_to_kb` Q&A tool five-stage pipeline.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: add_to_kb pipeline runs sanitize, validate, upsert, growth-log in order
  Requirement: Q&A budget constants are module-level
  Requirement: QATools exposes seven tools with audit_fields declared (audit_fields portion)
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from codebus_agent.agent.tools.add_to_kb import (
    AddToKBArgs,
    AddToKBChunk,
    add_to_kb,
)


@dataclass
class _FakeState:
    add_to_kb_question_count: int = 0
    add_to_kb_session_count: int = 0


@dataclass
class _FakeCtx:
    sanitizer: Any
    sanitizer_audit: Any
    kb: Any
    kb_growth_logger: Any
    qa_state: Any
    session_id: str = "qa_sess_01"
    question: str = "how does X work"
    originating_station_id: str = "s02-storage"


def _make_sanitizer(*, redact_to: str = "clean text", entries: int = 1) -> Any:
    sanitizer = MagicMock()

    async def _sanitize(text, source):
        result = MagicMock()
        result.text = redact_to
        result.entries = [MagicMock() for _ in range(entries)]
        return result

    sanitizer.sanitize = AsyncMock(side_effect=_sanitize)
    return sanitizer


def _build_ctx(
    *,
    upsert_return: tuple[str, str] = ("new", "new-pt-01"),
    sanitizer: Any = None,
    state: Any = None,
) -> tuple[_FakeCtx, MagicMock]:
    if sanitizer is None:
        sanitizer = _make_sanitizer()
    sanitizer_audit = MagicMock()
    kb = MagicMock()
    kb.upsert_chunk = AsyncMock(return_value=upsert_return)
    growth_logger = MagicMock()
    if state is None:
        state = _FakeState()
    ctx = _FakeCtx(
        sanitizer=sanitizer,
        sanitizer_audit=sanitizer_audit,
        kb=kb,
        kb_growth_logger=growth_logger,
        qa_state=state,
    )
    # Build a recorder that tracks call order across all four collaborators.
    recorder = MagicMock()
    recorder.attach_mock(sanitizer.sanitize, "sanitize")
    recorder.attach_mock(sanitizer_audit.append, "audit_append")
    recorder.attach_mock(kb.upsert_chunk, "upsert_chunk")
    recorder.attach_mock(growth_logger.write, "growth_write")
    return ctx, recorder


@pytest.mark.anyio("asyncio")
async def test_pipeline_order() -> None:
    ctx, recorder = _build_ctx()
    chunk = AddToKBChunk(
        text="hello world",
        source="src/x.py:10-20",
        related_stations=["s02-storage"],
    )
    args = AddToKBArgs(chunks=[chunk], source="src/x.py", reason="reusable")

    await add_to_kb(args, ctx)

    seen = [call[0] for call in recorder.mock_calls]
    # Filter to top-level method calls (no nested attribute lookups).
    seen = [name for name in seen if "." not in name]
    # Expected sequence: sanitize → audit_append (≥1) → upsert_chunk → growth_write
    sanitize_idx = seen.index("sanitize")
    audit_idx = seen.index("audit_append")
    upsert_idx = seen.index("upsert_chunk")
    growth_idx = seen.index("growth_write")
    assert sanitize_idx < audit_idx < upsert_idx < growth_idx


@pytest.mark.anyio("asyncio")
async def test_audit_append_uses_pass_num_3() -> None:
    ctx, _ = _build_ctx()
    chunk = AddToKBChunk(text="hello", source="src/x.py:1-5", related_stations=[])
    await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    args_list = ctx.sanitizer_audit.append.call_args_list
    assert args_list, "sanitizer_audit.append MUST be called"
    for call in args_list:
        kwargs = call.kwargs
        assert kwargs.get("pass_num") == 3


@pytest.mark.anyio("asyncio")
async def test_empty_post_sanitize_chunk_skipped() -> None:
    sanitizer = _make_sanitizer(redact_to="", entries=1)
    ctx, _ = _build_ctx(sanitizer=sanitizer)
    chunk = AddToKBChunk(text="<REDACTED>", source="src/x.py:1-5", related_stations=[])
    out = await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    assert "skipped_empty" in out
    ctx.kb.upsert_chunk.assert_not_awaited()
    ctx.kb_growth_logger.write.assert_not_called()


@pytest.mark.anyio("asyncio")
async def test_dedup_hit_records_growth_log_with_dedup_skipped_true() -> None:
    real_existing_pt = "00000000-1111-2222-3333-444444444444"
    ctx, _ = _build_ctx(upsert_return=("dedup_hash", real_existing_pt))
    chunk = AddToKBChunk(text="hi", source="src/x.py:1-5", related_stations=[])
    out = await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    assert "dedup_hash" in out
    ctx.kb_growth_logger.write.assert_called_once()
    kwargs = ctx.kb_growth_logger.write.call_args.kwargs
    assert kwargs["dedup_skipped"] is True
    # `entry_id` MUST be the real existing point id, not a sentinel.
    assert kwargs["point_id"] == real_existing_pt
    assert not kwargs["point_id"].startswith("dedup:")


@pytest.mark.anyio("asyncio")
async def test_invalid_station_id_aborts_before_upsert() -> None:
    ctx, _ = _build_ctx()
    chunk = AddToKBChunk(
        text="hello", source="src/x.py:1-5", related_stations=["s2-bad"]
    )
    out = await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    assert out.startswith("invalid station_id:")
    assert "s2-bad" in out
    ctx.kb.upsert_chunk.assert_not_awaited()


def test_audit_fields_excludes_chunks() -> None:
    fields = add_to_kb.audit_fields
    assert "chunks" not in fields
    for required in ("source", "reason", "related_stations"):
        assert required in fields


@pytest.mark.anyio("asyncio")
async def test_per_session_budget_returns_string_error() -> None:
    state = _FakeState(add_to_kb_session_count=20, add_to_kb_question_count=0)
    ctx, _ = _build_ctx(state=state)
    chunk = AddToKBChunk(text="hi", source="src/x.py:1-5", related_stations=[])
    out = await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    assert out.startswith("budget exhausted:")
    ctx.sanitizer.sanitize.assert_not_called()
    ctx.kb.upsert_chunk.assert_not_awaited()
    ctx.kb_growth_logger.write.assert_not_called()


@pytest.mark.anyio("asyncio")
async def test_oversize_chunk_rejected_without_kb_or_growth_log() -> None:
    sanitizer = _make_sanitizer(redact_to="x" * 2001, entries=0)
    ctx, _ = _build_ctx(sanitizer=sanitizer)
    chunk = AddToKBChunk(text="hi", source="src/x.py:1-5", related_stations=[])
    out = await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)
    assert "skipped_oversize" in out
    ctx.kb.upsert_chunk.assert_not_awaited()
    ctx.kb_growth_logger.write.assert_not_called()
