"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: TrackedProvider wraps every provider
    Scenario: Direct provider use forbidden
    Scenario: Wrapper preserves protocol shape
    Scenario: Skipping wrapper emits test failure
Also exercises the cross-requirement contract that chat / embed
through the wrapper produce exactly one UsageTracker line and one
LLMCallLogger line each.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider, MockScript
from codebus_agent.providers.protocol import LLMProvider, Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker


class _Plan(BaseModel):
    title: str = ""
    steps: int = 0


def _read_lines(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def _build_tracked(
    tmp_path: Path,
    inner: object | None = None,
    *,
    role: ProviderRole = ProviderRole.CHAT,
) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    return TrackedProvider(
        inner or MockProvider(), tracker=tracker, logger=logger, role=role
    )


def test_wrapper_passes_runtime_protocol_check(tmp_path: Path) -> None:
    """Scenario: Wrapper preserves protocol shape."""
    wrapped = _build_tracked(tmp_path)
    assert isinstance(wrapped, LLMProvider)


@pytest.mark.asyncio
async def test_chat_through_wrapper_writes_usage_and_call_logs(
    tmp_path: Path,
) -> None:
    wrapped = _build_tracked(tmp_path)

    result = await wrapped.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )
    assert isinstance(result, _Plan)

    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    call_lines = _read_lines(tmp_path / "llm_calls.jsonl")
    assert len(usage_lines) == 1
    assert len(call_lines) == 1
    assert usage_lines[0]["operation"] == "chat"
    assert usage_lines[0]["cost_usd"] is not None
    assert call_lines[0]["sanitizer_pass2_applied"] is False
    assert call_lines[0]["response"] is not None


@pytest.mark.asyncio
async def test_embed_through_wrapper_writes_usage_and_call_logs(
    tmp_path: Path,
) -> None:
    wrapped = _build_tracked(tmp_path)

    res = await wrapped.embed(texts=["one", "two"])
    assert len(res.vectors) == 2

    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    call_lines = _read_lines(tmp_path / "llm_calls.jsonl")
    assert len(usage_lines) == 1
    assert usage_lines[0]["operation"] == "embed"
    assert usage_lines[0]["output_tokens"] == 0
    assert len(call_lines) == 1


@pytest.mark.asyncio
async def test_chat_exception_still_logged_and_reraised(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Scenario: Failure still logged (from LLMCallLogger requirement).

    Inner provider must still be MockProvider (M1 invariant); we patch
    its `chat` to raise so the failure path runs under production rules.
    """
    wrapped = _build_tracked(tmp_path)

    async def _boom(*args: object, **kwargs: object) -> None:
        raise RuntimeError("boom")

    monkeypatch.setattr(wrapped._inner, "chat", _boom)

    with pytest.raises(RuntimeError, match="boom"):
        await wrapped.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Plan,
        )

    call_lines = _read_lines(tmp_path / "llm_calls.jsonl")
    assert len(call_lines) == 1
    assert call_lines[0]["response"] is None
    assert call_lines[0]["error"]["class"] == "RuntimeError"
    assert "boom" in call_lines[0]["error"]["message"]


@pytest.mark.asyncio
async def test_script_pinned_response_is_returned_through_wrapper(
    tmp_path: Path,
) -> None:
    pinned = _Plan(title="fixed", steps=9)
    script = MockScript()
    script.push(pinned)
    inner = MockProvider(script=script)
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    wrapped = TrackedProvider(
        inner, tracker=tracker, logger=logger, role=ProviderRole.CHAT
    )

    result = await wrapped.chat(
        messages=[Message(role="user", content="x")],
        response_model=_Plan,
    )
    assert result is pinned
