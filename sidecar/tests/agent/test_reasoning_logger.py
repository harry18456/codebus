"""RED tests for ReasoningLogger.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReasoningLogger appends one JSONL line per Step to workspace path
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

import pytest


def _make_step(step_num: int):
    from codebus_agent.agent.types import Step

    return Step(
        step=step_num,
        ts=datetime(2026, 4, 23, 12, 0, step_num, tzinfo=timezone.utc),
        thought=f"iteration {step_num}",
        tool_calls=[],
        tool_results=[],
        judge_verdict=None,
        tokens_used=100 + step_num,
    )


def test_each_write_appends_one_jsonl_line(workspace_dir: Path) -> None:
    """K writes → K lines, each `\\n`-terminated, each Step.model_validate_json-parseable."""
    from codebus_agent.agent.reasoning_logger import ReasoningLogger
    from codebus_agent.agent.types import Step

    log_path = workspace_dir / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path)

    for i in range(5):
        logger.write(_make_step(i))

    assert log_path.exists()
    raw = log_path.read_text(encoding="utf-8")
    # Each line terminated by `\n`.
    assert raw.endswith("\n")
    lines = raw.splitlines()
    assert len(lines) == 5

    parsed_steps = [Step.model_validate_json(line) for line in lines]
    for i, step in enumerate(parsed_steps):
        assert step.step == i
        assert step.thought == f"iteration {i}"
        assert step.tokens_used == 100 + i


def test_prompt_version_columns_present(workspace_dir: Path) -> None:
    """Every line MUST carry the module-level prompt version constants."""
    from codebus_agent.agent.prompts import (
        EXPLORER_PROMPT_VERSION,
        JUDGE_PROMPT_VERSION,
    )
    from codebus_agent.agent.reasoning_logger import ReasoningLogger

    log_path = workspace_dir / "reasoning_log.jsonl"
    logger = ReasoningLogger(log_path)
    logger.write(_make_step(0))

    parsed = json.loads(log_path.read_text(encoding="utf-8").splitlines()[0])
    assert "explorer_prompt_version" in parsed
    assert "judge_prompt_version" in parsed
    assert parsed["explorer_prompt_version"] == EXPLORER_PROMPT_VERSION
    assert parsed["judge_prompt_version"] == JUDGE_PROMPT_VERSION
    # Constants MUST be non-empty strings so golden-sample replays have a pin.
    assert isinstance(EXPLORER_PROMPT_VERSION, str) and EXPLORER_PROMPT_VERSION
    assert isinstance(JUDGE_PROMPT_VERSION, str) and JUDGE_PROMPT_VERSION


def test_write_failure_propagates(workspace_dir: Path) -> None:
    """Writer MUST raise on disk errors — silent drops are forbidden."""
    from codebus_agent.agent.reasoning_logger import ReasoningLogger

    # Target path whose parent directory does not exist → open('a') raises
    # FileNotFoundError. Bubble up MUST NOT be swallowed by the logger.
    bad_path = workspace_dir / "does_not_exist" / "nested" / "reasoning_log.jsonl"
    logger = ReasoningLogger(bad_path)

    with pytest.raises((FileNotFoundError, OSError)):
        logger.write(_make_step(0))
