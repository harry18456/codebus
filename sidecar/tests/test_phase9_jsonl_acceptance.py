"""Phase 9 / task 9.4 acceptance — backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: UsageTracker writes token_usage.jsonl
    Scenario: Required fields present
    Scenario: Embed calls tracked
  Requirement: LLMCallLogger writes llm_calls.jsonl
    Scenario: Request and response captured
    Scenario: Sanitizer-ready field reserved
    Scenario: Failure still logged

9.4 checks the *first write* contract end-to-end: simulate a workspace
directory, run chat + embed + failing-chat through TrackedProvider,
parse every resulting JSONL line, and assert every required field is
present with the correct type / M1 invariant value.
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


class _Plan(BaseModel):
    title: str = ""
    steps: int = 0


_REQUIRED_USAGE_KEYS = {
    "timestamp",
    "provider",
    "model",
    "operation",
    "input_tokens",
    "output_tokens",
    "cost_usd",
}


def _read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


@pytest.mark.asyncio
async def test_first_workspace_write_has_every_required_field(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Simulate a brand-new workspace and assert the very first JSONL
    write satisfies every field-level SHALL clause for both tracker
    and call-logger."""
    workspace = tmp_path / "ws"
    workspace.mkdir()

    tracker = UsageTracker(workspace / "token_usage.jsonl")
    logger = LLMCallLogger(workspace / "llm_calls.jsonl")
    wrapped = TrackedProvider(
        MockProvider(role=ProviderRole.CHAT),
        tracker=tracker,
        logger=logger,
        role=ProviderRole.CHAT,
    )

    await wrapped.chat(
        messages=[Message(role="user", content="hello")],
        response_model=_Plan,
    )
    await wrapped.embed(texts=["one", "two", "three"])

    async def _boom(*args: object, **kwargs: object) -> None:
        raise RuntimeError("forced-failure")

    monkeypatch.setattr(wrapped._inner, "chat", _boom)
    with pytest.raises(RuntimeError, match="forced-failure"):
        await wrapped.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Plan,
        )

    usage_lines = _read_jsonl(workspace / "token_usage.jsonl")
    call_lines = _read_jsonl(workspace / "llm_calls.jsonl")

    # -- token_usage.jsonl --
    # Scenario: One line per chat / embed call — 2 successful + 1 failed chat.
    # The failed call was patched at the inner layer, so TrackedProvider
    # raises before UsageTracker.record runs (documented behaviour —
    # usage metrics only count completed provider work).
    assert len(usage_lines) == 2

    chat_row, embed_row = usage_lines
    assert chat_row["operation"] == "chat"
    assert embed_row["operation"] == "embed"
    # Required-fields scenario: every key present, values non-null.
    for row in usage_lines:
        missing = _REQUIRED_USAGE_KEYS - set(row)
        assert not missing, f"row {row!r} missing keys: {missing}"
        for key in _REQUIRED_USAGE_KEYS:
            assert row[key] is not None, f"row {row!r} has null for {key}"
    # Embed-specific invariant.
    assert embed_row["output_tokens"] == 0

    # -- llm_calls.jsonl --
    # Scenario: every chat call (success OR failure) produces a line.
    # Embeds also produce a call-log line in this implementation.
    assert len(call_lines) == 3

    success_chat = call_lines[0]
    embed_log = call_lines[1]
    failure_chat = call_lines[2]

    # Request/response captured scenario.
    assert success_chat["request"] is not None
    assert success_chat["response"] is not None
    # Sanitizer-ready field scenario — M1 invariant is false.
    for row in call_lines:
        assert row.get("sanitizer_pass2_applied") is False, (
            f"sanitizer_pass2_applied MUST be false during M1, got {row!r}"
        )

    # Failure-logged scenario.
    assert failure_chat["response"] is None
    err = failure_chat["error"]
    assert err["class"] == "RuntimeError"
    assert "forced-failure" in err["message"]

    # Sanity: embed log retains the embed request shape.
    assert embed_log["request"] is not None
