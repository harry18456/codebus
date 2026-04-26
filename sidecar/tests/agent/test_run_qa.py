"""Tests for `run_qa` two-stage flow.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A loop entry point with two-stage RAG-first flow
"""
from __future__ import annotations

import ast
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock

import pytest

from codebus_agent.agent.qa import (
    QA_PROMPT_VERSION,
    _QA_MAX_STEPS,
    run_qa,
)
from codebus_agent.agent.reasoning_logger import ReasoningLogger
from codebus_agent.agent.types import (
    QAAction,
    QAAnswer,
    QAState,
    ToolCall,
)
from codebus_agent.kb.payload import KBHit, KBPayload


def _hit(score: float, text: str, file_path: str = "src/x.py") -> KBHit:
    payload = KBPayload(
        source_kind="code",
        file_path=file_path,
        line_start=1,
        line_end=10,
        text=text,
        text_hash="0" * 64,
        added_by="qa_agent",
        chunk_index=0,
        chunk_total=1,
        created_at=datetime.now(timezone.utc),
    )
    return KBHit(point_id=f"pt-{file_path}", score=score, payload=payload)


def _make_state() -> QAState:
    return QAState(
        question="how does storage work",
        originating_station_id="s02-storage",
        session_id="qa_sess_test",
        budget_steps_left=_QA_MAX_STEPS,
    )


@pytest.mark.anyio("asyncio")
async def test_confident_hits_skip_react_loop(tmp_path: Path) -> None:
    """Confident hits → cheap path; reasoning_log.jsonl line count == 0."""
    kb = AsyncMock()
    kb.query = AsyncMock(
        return_value=[
            _hit(0.85, "storage adapter implementation"),
            _hit(0.80, "storage adapter contract"),
            _hit(0.72, "storage api notes"),
        ]
    )
    provider = AsyncMock()
    provider.chat = AsyncMock(
        return_value=QAAnswer(answer="cheap path answer", citations=[])
    )
    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")
    state = _make_state()

    answer = await run_qa(
        question="how does storage work",
        state=state,
        kb=kb,
        tools=None,  # cheap path doesn't dispatch tools
        provider=provider,
        logger=logger,
    )
    assert isinstance(answer, QAAnswer)
    # Cheap path MUST NOT write Step entries.
    assert not log_path.exists() or log_path.read_text(encoding="utf-8") == ""


@pytest.mark.anyio("asyncio")
async def test_non_confident_hits_enter_react_loop(tmp_path: Path) -> None:
    """Weak hits → ReAct loop runs; at least one Step written."""
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.40, "weak hit text")])
    # Provider returns one tool_call iter, then empty (signals stop).
    chat_responses = [
        QAAction(
            thought="check more files",
            tool_calls=[ToolCall(id="tc_1", name="search", arguments={"keyword": "x"})],
        ),
        QAAction(thought="done thinking", tool_calls=[]),
        QAAnswer(answer="loop-driven answer", citations=[]),
    ]

    async def _chat(messages, *, response_model):
        return chat_responses.pop(0)

    provider = AsyncMock()
    provider.chat = _chat

    # Tools mock — search returns minimal observation.
    class _Tools:
        async def search(self, **kwargs):
            return ["found-it"]

    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")
    state = _make_state()
    answer = await run_qa(
        question="abstruse question",
        state=state,
        kb=kb,
        tools=_Tools(),
        provider=provider,
        logger=logger,
    )
    assert isinstance(answer, QAAnswer)
    assert log_path.exists()
    line_count = log_path.read_text(encoding="utf-8").count("\n")
    assert line_count >= 1


@pytest.mark.anyio("asyncio")
async def test_step_limit_via_should_stop(tmp_path: Path) -> None:
    """Infinite-loop scripted provider → step_count converges at ≤ _QA_MAX_STEPS."""
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.40, "weak")])

    # Provider always returns a tool_call; loop should stop at step limit.
    async def _chat(messages, *, response_model):
        if response_model is QAAction:
            return QAAction(
                thought="loop forever",
                tool_calls=[ToolCall(id="tc", name="search", arguments={"keyword": "x"})],
            )
        return QAAnswer(answer="forced answer", citations=[])

    provider = AsyncMock()
    provider.chat = _chat

    class _Tools:
        async def search(self, **kwargs):
            return ["x"]

    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")
    state = _make_state()
    await run_qa(
        question="endless",
        state=state,
        kb=kb,
        tools=_Tools(),
        provider=provider,
        logger=logger,
    )
    assert state.step_count <= _QA_MAX_STEPS


def test_qa_does_not_instantiate_judge_or_coverage() -> None:
    """Import-graph check: agent/qa.py MUST NOT import Judge/Coverage symbols."""
    qa_path = (
        Path(__file__).resolve().parents[2]
        / "src"
        / "codebus_agent"
        / "agent"
        / "qa.py"
    )
    text = qa_path.read_text(encoding="utf-8")
    tree = ast.parse(text)
    imported_names: list[str] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom):
            for alias in node.names:
                imported_names.append(alias.name)
        elif isinstance(node, ast.Import):
            for alias in node.names:
                imported_names.append(alias.name)
    forbidden = {"LLMJudge", "LLMCoverageChecker", "Judge", "CoverageChecker"}
    leaks = [n for n in imported_names if n in forbidden]
    assert leaks == [], f"qa.py imports forbidden symbols: {leaks}"


@pytest.mark.anyio("asyncio")
async def test_reasoning_log_has_qa_prompt_version_not_explorer(tmp_path: Path) -> None:
    """reasoning_log.jsonl lines from `run_qa` MUST have qa_prompt_version, not explorer/judge."""
    kb = AsyncMock()
    kb.query = AsyncMock(return_value=[_hit(0.40, "weak")])
    chat_responses = [
        QAAction(thought="step 1", tool_calls=[]),
        QAAnswer(answer="done", citations=[]),
    ]

    async def _chat(messages, *, response_model):
        return chat_responses.pop(0)

    provider = AsyncMock()
    provider.chat = _chat

    log_path = tmp_path / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path, mode="qa")
    state = _make_state()
    await run_qa(
        question="abstract",
        state=state,
        kb=kb,
        tools=None,
        provider=provider,
        logger=logger,
    )
    lines = [json.loads(line) for line in log_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert lines, "expected ≥1 Step line"
    for line in lines:
        assert line.get("qa_prompt_version") == QA_PROMPT_VERSION
        assert "explorer_prompt_version" not in line
        assert "judge_prompt_version" not in line
