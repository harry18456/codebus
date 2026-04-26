"""Tests for `kb_search` Q&A tool.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: kb_search invokes KnowledgeBase query with optional station filter
  Requirement: QATools exposes seven tools with audit_fields declared (audit_fields portion)
"""
from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any
from unittest.mock import AsyncMock

import pytest
from pydantic import ValidationError

from codebus_agent.agent.tools.kb_search import KBSearchArgs, kb_search
from codebus_agent.kb.payload import KBHit, KBPayload


@dataclass
class _FakeCtx:
    kb: Any
    workspace_root: Any = None
    workspace_type: str = "folder"


def _hit(text: str, related_stations: list[str], score: float = 0.8) -> KBHit:
    payload = KBPayload(
        source_kind="code",
        file_path="src/x.py",
        line_start=10,
        line_end=12,
        text=text,
        text_hash="0" * 64,
        added_by="qa_agent",
        chunk_index=0,
        chunk_total=1,
        created_at=datetime.now(timezone.utc),
        related_stations=related_stations,
    )
    return KBHit(point_id="pt-01", score=score, payload=payload)


def test_kbsearchargs_validates_station_id_format() -> None:
    KBSearchArgs(query="x", station_filter=["s02-storage"])
    with pytest.raises(ValidationError):
        KBSearchArgs(query="x", station_filter=["bad"])


@pytest.mark.anyio("asyncio")
async def test_forwards_station_filter_to_kb_query() -> None:
    fake_kb = AsyncMock()
    fake_kb.query.return_value = [_hit("snippet", ["s02-x"])]
    ctx = _FakeCtx(kb=fake_kb)
    args = KBSearchArgs(query="x", station_filter=["s02-x"])

    await kb_search(args, ctx)

    fake_kb.query.assert_awaited_once()
    call_kwargs = fake_kb.query.await_args.kwargs
    assert call_kwargs.get("filter_stations") == ["s02-x"]


@pytest.mark.anyio("asyncio")
async def test_hit_rendering_omits_empty_station_list() -> None:
    fake_kb = AsyncMock()
    fake_kb.query.return_value = [_hit("snippet text", related_stations=[])]
    ctx = _FakeCtx(kb=fake_kb)
    args = KBSearchArgs(query="x")
    rendered = await kb_search(args, ctx)
    assert "stations=" not in rendered


@pytest.mark.anyio("asyncio")
async def test_hit_rendering_includes_station_list_when_nonempty() -> None:
    fake_kb = AsyncMock()
    fake_kb.query.return_value = [_hit("body", ["s02-storage"])]
    ctx = _FakeCtx(kb=fake_kb)
    args = KBSearchArgs(query="x")
    rendered = await kb_search(args, ctx)
    assert "stations=[s02-storage]" in rendered


def test_audit_fields_declaration() -> None:
    assert kb_search.audit_fields == ["query", "top_k", "station_filter"]
