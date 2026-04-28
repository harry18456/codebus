"""End-to-end integration tests for the Q&A pipeline.

Backs SHALL clauses across module-8-qa-p0:
- qa-agent capability: full SSE event chain (rag_hits / agent_thought / qa_answer)
- kb-growth capability: kb_growth.jsonl integration
- sidecar-runtime capability: error containment

These tests exercise `run_qa` end-to-end with scripted KB / provider /
tools but real `KBGrowthLogger` and real ReasoningLogger writers, so
the per-task audit chain integrity (kb_growth.jsonl + reasoning_log.jsonl)
is asserted alongside the SSE event sequence.
"""
from __future__ import annotations

import json
import re
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
from codebus_agent.agent.types import QAAction, QAAnswer, QAState, ToolCall
from codebus_agent.kb.growth_logger import KBGrowthLogger
from codebus_agent.kb.payload import KBHit, KBPayload

_UUID_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
)


class _SpyEmitter:
    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


def _hit(score: float, text: str = "data") -> KBHit:
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
    return KBHit(point_id="pt", score=score, payload=payload)


@pytest.mark.anyio("asyncio")
async def test_confident_path_full_stack(tmp_path: Path) -> None:
    """Confident hits → cheap path; no agent_thought, no kb_growth."""
    kb = AsyncMock()
    kb.query = AsyncMock(
        return_value=[
            _hit(0.85, "abundant data adapter"),
            _hit(0.80, "abundant data utilities"),
            _hit(0.72, "data interface"),
        ]
    )
    provider = AsyncMock()
    provider.chat = AsyncMock(return_value=QAAnswer(answer="cheap", citations=[]))
    emitter = _SpyEmitter()
    state = QAState(question="how does data work", session_id="qa_e2e")

    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")
    answer = await run_qa(
        question="how does data work",
        state=state,
        kb=kb,
        tools=None,
        provider=provider,
        logger=logger,
        emitter=emitter,
    )
    assert isinstance(answer, QAAnswer)
    types = [e["type"] for e in emitter.events]
    assert "rag_hits" in types
    assert "qa_answer" in types
    assert "agent_thought" not in types
    assert "kb_growth" not in types
    assert "error" not in types
    # reasoning_log.jsonl MUST be empty (cheap path doesn't write Steps).
    assert not log_path.exists() or log_path.read_text(encoding="utf-8") == ""


@pytest.mark.anyio("asyncio")
async def test_react_path_with_add_to_kb_full_stack(tmp_path: Path) -> None:
    """Weak hits → ReAct loop; kb_growth event + kb_growth.jsonl line on new chunk."""
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.30, "weak")])

    # Provider script: 1 think with add_to_kb tool call → 1 think with empty (stop) → 1 final answer
    chat_responses = [
        QAAction(
            thought="add this fact",
            tool_calls=[
                ToolCall(
                    id="tc_1",
                    name="add_to_kb",
                    arguments={
                        "chunks": [
                            {
                                "text": "stable fact about storage",
                                "source": "src/y.py:1-3",
                                "related_stations": ["s02-storage"],
                            }
                        ],
                        "source": "src/y.py",
                        "reason": "reusable",
                    },
                )
            ],
        ),
        QAAction(thought="ok, done", tool_calls=[]),
        QAAnswer(answer="loop answer", citations=[]),
    ]

    async def _chat(messages, *, response_model):
        return chat_responses.pop(0)

    provider = AsyncMock()
    provider.chat = _chat

    # Build a tools façade with a real add_to_kb wired against a real KBGrowthLogger.
    from codebus_agent.agent.tools.add_to_kb import (
        AddToKBArgs,
        add_to_kb as add_to_kb_func,
    )
    from codebus_agent.kb.growth_logger import KBGrowthLogger

    growth_path = tmp_path / ".codebus" / "kb_growth.jsonl"
    growth_logger = KBGrowthLogger(growth_path)

    # Sanitizer mock that returns text unchanged
    class _Sanitizer:
        async def sanitize(self, text, source):
            return MagicMock(text=text, entries=[])

    class _SanitizerAudit:
        def append(self, **kwargs):
            pass

    fake_kb = MagicMock()
    fake_kb.upsert_chunk = AsyncMock(return_value=("new", "new-pt-01"))

    emitter = _SpyEmitter()

    class _Ctx:
        def __init__(self):
            self.sanitizer = _Sanitizer()
            self.sanitizer_audit = _SanitizerAudit()
            self.kb = fake_kb
            self.kb_growth_logger = growth_logger
            self.qa_state = QAState(question="q", session_id="qa_e2e")
            self.question = "q"
            self.originating_station_id = "s02-storage"
            self.session_id = "qa_e2e"
            self.emitter = emitter

    add_ctx = _Ctx()

    class _Tools:
        async def add_to_kb(self, **kwargs):
            args = AddToKBArgs(**kwargs)
            return await add_to_kb_func(args, add_ctx)

    state = QAState(question="abstruse", session_id="qa_e2e")
    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")

    await run_qa(
        question="abstruse",
        state=state,
        kb=kb,
        tools=_Tools(),
        provider=provider,
        logger=logger,
        emitter=emitter,
    )

    types = [e["type"] for e in emitter.events]
    assert "rag_hits" in types
    assert "agent_thought" in types
    assert "kb_growth" in types
    assert "qa_answer" in types
    assert "error" not in types

    # kb_growth.jsonl: at least one line with required keys.
    assert growth_path.exists()
    lines = [json.loads(line) for line in growth_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert len(lines) >= 1
    assert lines[0]["entry_id"] == "new-pt-01"
    assert lines[0]["event_type"] == "add"

    # reasoning_log.jsonl: at least one Step.
    assert log_path.exists()
    log_lines = [line for line in log_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert len(log_lines) >= 1


@pytest.mark.anyio("asyncio")
async def test_dedup_path_writes_real_point_id_to_kb_growth(tmp_path: Path) -> None:
    """Dedup-skipped writes MUST record the real Qdrant point id (UUID
    format) — never the legacy `"dedup:hash"` / `"dedup:sim"` sentinels.

    Spec scenario `Dedup-skipped write records existing point id`
    (kb-growth capability), driving the round-trip from KB tuple return
    through `add_to_kb` into `kb_growth.jsonl`.
    """
    existing_pt_id = "deadbeef-0000-1111-2222-333344445555"

    growth_path = tmp_path / ".codebus" / "kb_growth.jsonl"
    growth_logger = KBGrowthLogger(growth_path)

    class _Sanitizer:
        async def sanitize(self, text, source):
            return MagicMock(text=text, entries=[])

    class _SanitizerAudit:
        def append(self, **kwargs):
            pass

    fake_kb = MagicMock()
    fake_kb.upsert_chunk = AsyncMock(
        return_value=("dedup_hash", existing_pt_id)
    )

    class _Ctx:
        def __init__(self):
            self.sanitizer = _Sanitizer()
            self.sanitizer_audit = _SanitizerAudit()
            self.kb = fake_kb
            self.kb_growth_logger = growth_logger
            self.qa_state = QAState(question="q", session_id="qa_dedup")
            self.question = "q"
            self.originating_station_id = "s02-storage"
            self.session_id = "qa_dedup"
            self.emitter = None

    chunk = AddToKBChunk(
        text="duplicate content",
        source="src/y.py:1-3",
        related_stations=["s02-storage"],
    )
    await add_to_kb(AddToKBArgs(chunks=[chunk], source="src/y.py", reason="r"), _Ctx())

    assert growth_path.exists()
    lines = [
        json.loads(line)
        for line in growth_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert len(lines) == 1
    line = lines[0]
    assert _UUID_RE.match(line["entry_id"]), line["entry_id"]
    assert line["entry_id"] == existing_pt_id
    assert line["dedup_skipped"] is True
    assert not line["entry_id"].startswith("dedup:")


@pytest.mark.anyio("asyncio")
async def test_pass3_sanitize_audit_records_real_rules_version(tmp_path: Path) -> None:
    """Pass 3 sanitize hit MUST stamp the real `RULES_VERSION` constant
    into `sanitize_audit.jsonl.rules_version` — never the legacy
    `"rules-unknown"` placeholder.

    Spec invariant 9 (CLAUDE.md) + `Single source of truth for
    rules_version constant` Scenario.
    """
    from codebus_agent.sanitizer import (
        RULES_VERSION,
        SanitizerAuditLogger,
        SanitizerEngine,
    )

    audit_path = tmp_path / ".codebus" / "sanitize_audit.jsonl"
    growth_path = tmp_path / ".codebus" / "kb_growth.jsonl"

    sanitizer = SanitizerEngine()
    sanitizer_audit = SanitizerAuditLogger(audit_path)
    growth_logger = KBGrowthLogger(growth_path)

    fake_kb = MagicMock()
    fake_kb.upsert_chunk = AsyncMock(return_value=("new", "11111111-2222-3333-4444-555555555555"))

    class _Ctx:
        def __init__(self):
            self.sanitizer = sanitizer
            self.sanitizer_audit = sanitizer_audit
            self.kb = fake_kb
            self.kb_growth_logger = growth_logger
            self.qa_state = QAState(question="q", session_id="qa_pass3")
            self.question = "q"
            self.originating_station_id = "s02-storage"
            self.session_id = "qa_pass3"
            self.emitter = None

    chunk = AddToKBChunk(
        text="contact alice@example.com for details",
        source="src/notes.md:1-1",
        related_stations=["s02-storage"],
    )
    await add_to_kb(
        AddToKBArgs(chunks=[chunk], source="src/notes.md", reason="r"),
        _Ctx(),
    )

    assert audit_path.exists()
    audit_lines = [
        json.loads(line)
        for line in audit_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert len(audit_lines) >= 1
    first = audit_lines[0]
    assert first["rules_version"] == RULES_VERSION
    assert first["rules_version"] != "rules-unknown"
    assert first["pass"] == 3
