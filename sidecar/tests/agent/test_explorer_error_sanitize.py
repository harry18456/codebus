"""TDD red tests for D2.19 — Explorer tool error path MUST go through Pass 2 sanitize.

Backs Requirement `ReAct loop executes think-act-observe-judge-log-update
each iteration` (agent-core capability), Scenarios:
  * `Tool errors do not crash the loop` (modified — adds Pass 2 constraint)
  * `Tool error string sanitized through Pass 2` (new)
And `SanitizerAuditLogger appends each replacement to JSONL` (sanitizer):
  * `Explorer tool error path runs Pass 2 sanitize` (new)

Pre-fix behavior: `_execute_one` raises an exception → `output=f"ERROR:
{exc}"` puts raw exception text (which may include user input / secrets)
straight into `ToolResult.output`. This bypasses Pass 2.

Post-fix behavior: when `run_explorer` is given a sanitizer + audit
logger, the error string is sanitized via Pass 2 with
`MessageSource(message_id=f"explorer_step_{step_count}_tool_error")`.
"""
from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_RULES_VERSION = "2026-04-20-1"


# --- Spies -----------------------------------------------------------------


class _CountingJudge:
    async def evaluate(self, state: Any, results: list[Any]) -> Any:
        from codebus_agent.agent.types import JudgeVerdict

        return JudgeVerdict(
            relevance=0.5,
            should_follow_imports=False,
            should_add_station=False,
            reason="ok",
        )


class _CountingCoverage:
    async def check(self, state: Any) -> list[Any]:
        return []


class _RecordingLogger:
    def __init__(self, path: Path) -> None:
        from codebus_agent.agent.reasoning_logger import ReasoningLogger

        self._inner = ReasoningLogger(path)
        self.writes: list[Any] = []

    @property
    def path(self) -> Path:
        return self._inner.path

    def write(self, step: Any) -> None:
        self.writes.append(step)
        self._inner.write(step)


class _ExplodingTool:
    """Tool whose echo method raises ValueError with the configured message."""

    def __init__(self, error_text: str) -> None:
        self._error_text = error_text

    async def echo(self, **kwargs: Any) -> str:
        raise ValueError(self._error_text)


# --- Helpers ---------------------------------------------------------------


def _build_tracked_reasoning_provider(
    workspace: Path, script: MockScript
) -> TrackedProvider:
    audit_dir = workspace / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    return TrackedProvider(
        MockProvider(script=script, role=ProviderRole.REASONING),
        tracker=UsageTracker(audit_dir / "token_usage.jsonl"),
        logger=LLMCallLogger(audit_dir / "llm_calls.jsonl"),
        role=ProviderRole.REASONING,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
        rules_version=_RULES_VERSION,
        default_module="reasoning",
    )


def _read_audit_lines(workspace: Path) -> list[dict]:
    path = workspace / ".codebus" / "sanitize_audit.jsonl"
    if not path.exists():
        return []
    return [
        json.loads(line)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]


def _push_action_with_echo(script: MockScript, msg: str = "x") -> None:
    from codebus_agent.agent.types import ExplorerAction, ToolCall

    script.push(
        ExplorerAction(
            thought="call echo",
            tool_calls=[ToolCall(id="tc_1", name="echo", arguments={"msg": msg})],
            stop=False,
        )
    )


# --- D2.19 tests -----------------------------------------------------------


@pytest.mark.asyncio
async def test_explorer_tool_error_path_sanitized(tmp_path: Path) -> None:
    """D2.19 Scenario `Tool error string sanitized through Pass 2`.

    A tool raises `ValueError("api_key=sk-AKIAIOSFODNN7EXAMPLE invalid")`.
    Post-fix: `Step.tool_results[0].output` MUST contain `<REDACTED:`
    placeholder and MUST NOT contain the raw secret literal.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    script = MockScript()
    _push_action_with_echo(script)

    provider = _build_tracked_reasoning_provider(tmp_path, script)

    audit_dir = tmp_path / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    sanitizer = SanitizerEngine()
    audit = SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl")

    logger = _RecordingLogger(audit_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)
    tools = _ExplodingTool("api_key=sk-AKIAIOSFODNN7EXAMPLE invalid")

    await run_explorer(
        state=state,
        provider=provider,
        tools=tools,
        judge=_CountingJudge(),
        coverage=_CountingCoverage(),
        logger=logger,
        sanitizer=sanitizer,
        sanitizer_audit=audit,
        session_id="sess-d219",
        rules_version=_RULES_VERSION,
    )

    assert logger.writes, "Step MUST be written even when tool errors"
    failed = logger.writes[0].tool_results[0]
    assert failed.output, f"ToolResult.output must not be empty; got {failed!r}"
    assert "<REDACTED:" in failed.output, (
        f"output MUST contain redaction placeholder; got {failed.output!r}"
    )
    assert "AKIAIOSFODNN7EXAMPLE" not in failed.output, (
        f"raw secret MUST NOT leak; got {failed.output!r}"
    )


@pytest.mark.asyncio
async def test_explorer_error_writes_pass2_audit_with_message_source(
    tmp_path: Path,
) -> None:
    """D2.19 Scenario continued: audit log gains pass=2 line with MessageSource."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    script = MockScript()
    _push_action_with_echo(script)

    provider = _build_tracked_reasoning_provider(tmp_path, script)

    audit_dir = tmp_path / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    sanitizer = SanitizerEngine()
    audit = SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl")

    logger = _RecordingLogger(audit_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)
    tools = _ExplodingTool("token=AKIAIOSFODNN7EXAMPLE bad")

    await run_explorer(
        state=state,
        provider=provider,
        tools=tools,
        judge=_CountingJudge(),
        coverage=_CountingCoverage(),
        logger=logger,
        sanitizer=sanitizer,
        sanitizer_audit=audit,
        session_id="sess-d219",
        rules_version=_RULES_VERSION,
    )

    lines = _read_audit_lines(tmp_path)
    pass2_lines = [line for line in lines if line.get("pass") == 2]
    assert pass2_lines, f"expected at least one pass=2 line; got {lines!r}"

    # Source MUST reflect MessageSource(message_id="explorer_step_..._tool_error")
    # — `_format_source` for MessageSource yields `"message:<id>"` string.
    found = False
    for line in pass2_lines:
        src = line["source"]
        if isinstance(src, str) and src.startswith("message:") and "explorer_step_" in src and "tool_error" in src:
            found = True
            break
    assert found, (
        f"expected pass=2 line with MessageSource(message_id='explorer_step_*_tool_error'); "
        f"got {pass2_lines!r}"
    )


@pytest.mark.asyncio
async def test_explorer_error_with_clean_message_no_audit(tmp_path: Path) -> None:
    """D2.19: clean error message (no secrets) → no new pass=2 audit lines.

    Only sanitize hits append audit entries (per `SanitizerAuditLogger`
    contract). A clean error like `"file not found"` has zero hits, so
    no Pass 2 line is written. The output still contains the original
    message text (sanitize is a no-op).
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    script = MockScript()
    _push_action_with_echo(script)

    provider = _build_tracked_reasoning_provider(tmp_path, script)

    audit_dir = tmp_path / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    sanitizer = SanitizerEngine()
    audit = SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl")

    pre_existing_lines = _read_audit_lines(tmp_path)
    pre_pass2 = [line for line in pre_existing_lines if line.get("pass") == 2]

    logger = _RecordingLogger(audit_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)
    tools = _ExplodingTool("file not found")

    await run_explorer(
        state=state,
        provider=provider,
        tools=tools,
        judge=_CountingJudge(),
        coverage=_CountingCoverage(),
        logger=logger,
        sanitizer=sanitizer,
        sanitizer_audit=audit,
        session_id="sess-d219",
        rules_version=_RULES_VERSION,
    )

    failed = logger.writes[0].tool_results[0]
    assert "file not found" in failed.output, (
        f"clean error text MUST survive sanitize unchanged; got {failed.output!r}"
    )
    # Audit log MUST NOT gain new pass=2 entries when sanitize had zero hits.
    post_lines = _read_audit_lines(tmp_path)
    post_pass2 = [line for line in post_lines if line.get("pass") == 2]
    assert len(post_pass2) == len(pre_pass2), (
        f"clean error MUST NOT add pass=2 audit lines; "
        f"pre={len(pre_pass2)}, post={len(post_pass2)}"
    )
