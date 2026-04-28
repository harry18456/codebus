"""SSE event integration tests for `run_qa`.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A run emits SSE events on the task channel
"""
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

from codebus_agent.agent.qa import run_qa
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.tools.add_to_kb import (
    AddToKBArgs,
    AddToKBChunk,
    add_to_kb,
)
from codebus_agent.agent.types import (
    QAAction,
    QAAnswer,
    QAState,
    ToolCall,
)
from codebus_agent.kb.payload import KBHit, KBPayload


class _SpyEmitter:
    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


def _hit(score: float, text: str = "x") -> KBHit:
    payload = KBPayload(
        source_kind="code",
        file_path="src/x.py",
        line_start=1,
        line_end=10,
        text=text,
        text_hash="0" * 64,
        added_by="qa_agent",
        chunk_index=0,
        chunk_total=1,
        created_at=datetime.now(timezone.utc),
    )
    return KBHit(point_id="pt-x", score=score, payload=payload)


@pytest.mark.anyio("asyncio")
async def test_rag_hits_emitted_once_after_initial_query(tmp_path: Path) -> None:
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.9, "abundant data")])
    provider = AsyncMock()
    provider.chat = AsyncMock(return_value=QAAnswer(answer="x", citations=[]))
    state = QAState(question="data", session_id="s")
    emitter = _SpyEmitter()

    await run_qa(
        question="data",
        state=state,
        kb=kb,
        tools=None,
        provider=provider,
        emitter=emitter,
    )
    rag_events = [e for e in emitter.events if e["type"] == "rag_hits"]
    assert len(rag_events) == 1
    assert "hits" in rag_events[0]


@pytest.mark.anyio("asyncio")
async def test_rag_hits_precedes_any_agent_thought(tmp_path: Path) -> None:
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.30, "weak")])
    chat_responses = [
        QAAction(thought="think 1", tool_calls=[]),
        QAAnswer(answer="done", citations=[]),
    ]

    async def _chat(messages, *, response_model):
        return chat_responses.pop(0)

    provider = AsyncMock()
    provider.chat = _chat
    state = QAState(question="weak topic", session_id="s")
    emitter = _SpyEmitter()
    await run_qa(
        question="weak topic",
        state=state,
        kb=kb,
        tools=None,
        provider=provider,
        emitter=emitter,
    )
    types = [e["type"] for e in emitter.events]
    rag_idx = types.index("rag_hits")
    if "agent_thought" in types:
        thought_idx = types.index("agent_thought")
        assert rag_idx < thought_idx


@pytest.mark.anyio("asyncio")
async def test_kb_growth_event_emitted_on_new_chunk(tmp_path: Path) -> None:
    """Successful add_to_kb new-point write MUST emit one `kb_growth` event."""
    sanitizer = MagicMock()
    sanitizer.sanitize = AsyncMock(return_value=MagicMock(text="hello", entries=[]))
    sanitizer_audit = MagicMock()
    kb = MagicMock()
    kb.upsert_chunk = AsyncMock(return_value=("new", "new-pt-01"))
    growth_logger = MagicMock()
    emitter = _SpyEmitter()

    class _Ctx:
        def __init__(self):
            self.sanitizer = sanitizer
            self.sanitizer_audit = sanitizer_audit
            self.kb = kb
            self.kb_growth_logger = growth_logger
            self.qa_state = QAState(question="q", session_id="s")
            self.question = "q"
            self.originating_station_id = "s02-storage"
            self.session_id = "s"
            self.emitter = emitter

    ctx = _Ctx()
    chunk = AddToKBChunk(
        text="hello", source="src/x.py:1-5", related_stations=["s02-storage"]
    )
    await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)

    growth_events = [e for e in emitter.events if e["type"] == "kb_growth"]
    assert len(growth_events) == 1
    payload = growth_events[0]
    assert payload["entry_id"] == "new-pt-01"
    assert payload["source"] == "src/x.py:1-5"
    assert payload["related_stations"] == ["s02-storage"]
    assert payload["originating_station_id"] == "s02-storage"


@pytest.mark.anyio("asyncio")
async def test_kb_growth_event_omitted_on_dedup_skip(tmp_path: Path) -> None:
    """`upsert_chunk` returning `("dedup_hash", real_id)` MUST NOT emit
    `kb_growth` (dedup-skipped writes still hit the audit log but skip
    the SSE event per spec scenario `kb_growth event omitted on dedup skip`).
    """
    sanitizer = MagicMock()
    sanitizer.sanitize = AsyncMock(return_value=MagicMock(text="hello", entries=[]))
    sanitizer_audit = MagicMock()
    kb = MagicMock()
    kb.upsert_chunk = AsyncMock(
        return_value=("dedup_hash", "11111111-2222-3333-4444-555555555555")
    )
    growth_logger = MagicMock()
    emitter = _SpyEmitter()

    class _Ctx:
        def __init__(self):
            self.sanitizer = sanitizer
            self.sanitizer_audit = sanitizer_audit
            self.kb = kb
            self.kb_growth_logger = growth_logger
            self.qa_state = QAState(question="q", session_id="s")
            self.question = "q"
            self.originating_station_id = "s02-storage"
            self.session_id = "s"
            self.emitter = emitter

    ctx = _Ctx()
    chunk = AddToKBChunk(
        text="hello", source="src/x.py:1-5", related_stations=["s02-storage"]
    )
    await add_to_kb(AddToKBArgs(chunks=[chunk]), ctx)

    growth_events = [e for e in emitter.events if e["type"] == "kb_growth"]
    assert growth_events == []
    # But growth_logger.write MUST still be called with dedup_skipped=True.
    growth_logger.write.assert_called_once()
    assert growth_logger.write.call_args.kwargs["dedup_skipped"] is True


@pytest.mark.anyio("asyncio")
async def test_qa_answer_payload_schema_p0(tmp_path: Path) -> None:
    """Final `qa_answer` event payload MUST contain `answer` + `citations`."""
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.9, "data")])
    provider = AsyncMock()
    provider.chat = AsyncMock(return_value=QAAnswer(answer="text", citations=[]))
    emitter = _SpyEmitter()
    state = QAState(question="data", session_id="s")
    await run_qa(
        question="data",
        state=state,
        kb=kb,
        tools=None,
        provider=provider,
        emitter=emitter,
    )
    answer_events = [e for e in emitter.events if e["type"] == "qa_answer"]
    assert len(answer_events) == 1
    assert "answer" in answer_events[0]
    assert "citations" in answer_events[0]
    assert isinstance(answer_events[0]["citations"], list)
