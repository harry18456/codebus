"""RED tests for TrackedProvider SSE emit (agent-sse-wiring §6).

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: TrackedProvider emits usage_delta on every completed call
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.protocol import Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


class _Plan(BaseModel):
    title: str = ""
    steps: int = 0


class _SpyEmitter:
    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


def _read_lines(path: Path) -> list[dict]:
    return [
        json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()
    ]


def _build_tracked(
    tmp_path: Path,
    *,
    inner: object | None = None,
    emitter: object | None = None,
    default_module: str = "judge",
) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    return TrackedProvider(
        inner or MockProvider(),
        tracker=tracker,
        logger=logger,
        role=ProviderRole.JUDGE,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
        rules_version="test-v1",
        default_module=default_module,
        emitter=emitter,
    )


@pytest.mark.asyncio
async def test_emitter_fires_after_token_usage_jsonl_write(tmp_path: Path) -> None:
    """Spec scenario `Emitter fires after token_usage.jsonl write`."""
    spy = _SpyEmitter()
    wrapped = _build_tracked(tmp_path, emitter=spy, default_module="judge")

    result = await wrapped.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )
    assert isinstance(result, _Plan)

    usage_deltas = [e for e in spy.events if e["type"] == "usage_delta"]
    assert len(usage_deltas) == 1, (
        f"exactly one usage_delta per successful chat; got {spy.events}"
    )
    evt = usage_deltas[0]
    assert evt["module"] == "judge"
    assert evt["prompt_tokens"] is not None
    assert evt["completion_tokens"] is not None
    # `session_total_cost_usd` MUST accompany every delta (spec requires running total).
    assert "session_total_cost_usd" in evt

    # token_usage.jsonl MUST have been written before the emit; we assert
    # one line is present since the emit fired (spec order is enforced by
    # TrackedProvider.chat body).
    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    assert len(usage_lines) == 1


@pytest.mark.asyncio
async def test_failed_call_suppresses_usage_delta(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Spec scenario `Failed call suppresses usage_delta`."""
    spy = _SpyEmitter()
    wrapped = _build_tracked(tmp_path, emitter=spy)

    async def _boom(*args: object, **kwargs: object) -> None:
        raise RuntimeError("upstream-500")

    monkeypatch.setattr(wrapped._inner, "chat", _boom)

    with pytest.raises(RuntimeError, match="upstream-500"):
        await wrapped.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Plan,
        )

    assert not any(e["type"] == "usage_delta" for e in spy.events), (
        "failed chat MUST NOT emit usage_delta"
    )
    # The llm_calls.jsonl failure line MUST still be present (pre-existing contract).
    call_lines = _read_lines(tmp_path / "llm_calls.jsonl")
    assert len(call_lines) == 1
    assert call_lines[0]["response"] is None
    assert call_lines[0]["error"]["class"] == "RuntimeError"


@pytest.mark.asyncio
async def test_omitting_emitter_preserves_existing_behavior(tmp_path: Path) -> None:
    """Spec scenario `Omitting emitter preserves existing behavior`.

    Constructing TrackedProvider without `emitter` keeps the existing
    `token_usage.jsonl` / `llm_calls.jsonl` single-line behaviour.
    """
    wrapped = _build_tracked(tmp_path)  # no emitter
    await wrapped.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )

    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    call_lines = _read_lines(tmp_path / "llm_calls.jsonl")
    assert len(usage_lines) == 1
    assert len(call_lines) == 1
    assert usage_lines[0]["module"] == "judge"


@pytest.mark.asyncio
async def test_context_var_scopes_phase_and_step(tmp_path: Path) -> None:
    """Spec: `phase` / `step` read from ContextVar; None when unset."""
    from codebus_agent.agent import context_vars

    spy = _SpyEmitter()
    wrapped = _build_tracked(tmp_path, emitter=spy)

    # With ContextVars set, the usage_delta MUST carry those values.
    token_phase = context_vars.current_phase_var.set("explore")
    token_step = context_vars.current_step_var.set(3)
    try:
        await wrapped.chat(
            messages=[Message(role="user", content="hi")],
            response_model=_Plan,
        )
    finally:
        context_vars.current_phase_var.reset(token_phase)
        context_vars.current_step_var.reset(token_step)

    evt = next(e for e in spy.events if e["type"] == "usage_delta")
    assert evt["phase"] == "explore"
    assert evt["step"] == 3

    # With ContextVars unset, both fields MUST appear as None (JSON null).
    spy.events.clear()
    await wrapped.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )
    evt2 = next(e for e in spy.events if e["type"] == "usage_delta")
    assert evt2["phase"] is None
    assert evt2["step"] is None
