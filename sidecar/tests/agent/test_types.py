"""RED tests for agent-core Pydantic types.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: Agent-core types are Pydantic BaseModels with stable JSON serialization

Pins round-trip behaviour + validator guards BEFORE `agent/types.py`
defines the classes, so the GREEN work in Section 3 is measured
against a concrete contract rather than our memory of it.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone

import pytest
from pydantic import ValidationError


def test_explorer_action_round_trips() -> None:
    """Raw JSON → validate → dump → json.loads == original dict.

    Spec scenario `ExplorerAction round-trips through Instructor parse path`.
    """
    from codebus_agent.agent.types import ExplorerAction

    raw = {
        "thought": "check the scanner module for gitignore handling",
        "tool_calls": [],
        "stop": False,
    }
    action = ExplorerAction.model_validate_json(json.dumps(raw))
    dumped = json.loads(action.model_dump_json())

    assert dumped == raw


def test_step_round_trips_with_nested_verdict() -> None:
    """Step with embedded JudgeVerdict + ToolResult round-trips losslessly."""
    from codebus_agent.agent.types import (
        JudgeVerdict,
        Step,
        ToolCall,
        ToolResult,
    )

    step = Step(
        step=3,
        ts=datetime(2026, 4, 23, 12, 0, 0, tzinfo=timezone.utc),
        thought="follow the import from KnowledgeBase",
        tool_calls=[
            ToolCall(
                id="tc_abc",
                name="read_file",
                arguments={"path": "sidecar/src/codebus_agent/kb/knowledge_base.py"},
            )
        ],
        tool_results=[
            ToolResult(
                tool_call_id="tc_abc",
                tool_name="read_file",
                output="class KnowledgeBase: ...",
                raw=None,
                error=None,
            )
        ],
        judge_verdict=JudgeVerdict(
            relevance=0.85,
            should_follow_imports=True,
            should_add_station=True,
            reason="central abstraction",
        ),
        tokens_used=1240,
        explorer_prompt_version="v0-p0",
        judge_prompt_version="v0-p0",
    )

    payload = step.model_dump_json()
    restored = Step.model_validate_json(payload)
    assert restored.model_dump_json() == payload

    parsed = json.loads(payload)
    assert parsed["judge_verdict"]["relevance"] == 0.85
    assert parsed["tool_calls"][0]["arguments"]["path"].endswith("knowledge_base.py")
    assert parsed["tool_results"][0]["output"] == "class KnowledgeBase: ..."


def test_judge_verdict_rejects_out_of_range_relevance() -> None:
    """relevance > 1.0 MUST raise ValidationError at parse time."""
    from codebus_agent.agent.types import JudgeVerdict

    payload = json.dumps(
        {
            "relevance": 1.5,
            "should_follow_imports": False,
            "should_add_station": False,
            "reason": "out of bounds",
        }
    )
    with pytest.raises(ValidationError):
        JudgeVerdict.model_validate_json(payload)

    payload_negative = json.dumps(
        {
            "relevance": -0.1,
            "should_follow_imports": False,
            "should_add_station": False,
            "reason": "out of bounds",
        }
    )
    with pytest.raises(ValidationError):
        JudgeVerdict.model_validate_json(payload_negative)


def test_explorer_state_required_fields() -> None:
    """Constructing ExplorerState without required keys MUST raise ValidationError."""
    from codebus_agent.agent.types import ExplorerState

    with pytest.raises(ValidationError):
        ExplorerState()  # nothing supplied — all three required fields missing

    with pytest.raises(ValidationError):
        ExplorerState(task="explore KB")  # budget_* still missing

    with pytest.raises(ValidationError):
        ExplorerState(task="explore KB", budget_steps_left=10)  # tokens missing

    # All three supplied — MUST succeed (other fields have defaults).
    state = ExplorerState(
        task="explore KB",
        budget_steps_left=10,
        budget_tokens_left=10_000,
    )
    assert state.task == "explore KB"
    assert state.budget_steps_left == 10
    assert state.budget_tokens_left == 10_000
