"""Tests for `QAState` / `QAAction` / `QAAnswer` / `KBCitation` Pydantic models.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: QAState, QAAnswer, and QAAction are Pydantic models
"""
from __future__ import annotations

from codebus_agent.agent.types import (
    KBCitation,
    Message,
    QAAction,
    QAAnswer,
    QAState,
    ToolCall,
)


def test_qastate_round_trip() -> None:
    state = QAState(
        question="how does storage work",
        originating_station_id="s02-storage",
        session_id="qa_sess",
        messages=[Message(role="user", content="hi")],
        step_count=3,
        add_to_kb_session_count=2,
        add_to_kb_question_count=1,
    )
    raw = state.model_dump_json()
    revived = QAState.model_validate_json(raw)
    assert revived.model_dump_json() == raw


def test_qaaction_compatible_with_explorer_action_shape() -> None:
    """QAAction MUST mirror ExplorerAction shape (`thought`, `tool_calls`)."""
    action = QAAction(
        thought="check kb",
        tool_calls=[
            ToolCall(id="tc_1", name="kb_search", arguments={"query": "x"})
        ],
    )
    assert action.thought == "check kb"
    assert len(action.tool_calls) == 1
    assert action.tool_calls[0].name == "kb_search"


def test_qaanswer_citations_schema() -> None:
    cit = KBCitation(
        file_path="x.py",
        line_start=1,
        line_end=10,
        related_stations=["s01-x"],
    )
    answer = QAAnswer(answer="text", citations=[cit])
    dumped = answer.model_dump()
    assert dumped["answer"] == "text"
    assert len(dumped["citations"]) == 1
    citation = dumped["citations"][0]
    for field in ("file_path", "line_start", "line_end", "related_stations"):
        assert field in citation
