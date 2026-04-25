"""RED tests for TrackedProvider session token counters.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/specs/usage-tracking/spec.md
  ADDED Requirement: TrackedProvider exposes session token counters

Section 2 pins the five scenarios of the counter contract:
  - counters start at zero
  - successful chat advances both prompt + completion counters
  - failed chat leaves counters unchanged (mirrors cost semantic)
  - embed path contributes to prompt counter only
  - counters are per-instance, not shared across TrackedProvider instances
"""
from __future__ import annotations

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


def test_session_token_counters_start_at_zero(tmp_path: Path) -> None:
    """Spec scenario `Counters start at zero`."""
    wrapped = _build_tracked(tmp_path)
    assert wrapped.session_prompt_tokens == 0
    assert wrapped.session_completion_tokens == 0
    assert wrapped.session_total_tokens == 0


@pytest.mark.asyncio
async def test_successful_chat_advances_both_counters(tmp_path: Path) -> None:
    """Spec scenario `Successful chat advances both counters`."""
    wrapped = _build_tracked(tmp_path)

    await wrapped.chat(
        messages=[Message(role="user", content="hello world")],
        response_model=_Plan,
    )

    # Counters are per-call deltas from TrackedProvider's token estimator
    # (4 chars ≈ 1 token). Values are non-zero after a successful call
    # and session_total_tokens = prompt + completion.
    prompt = wrapped.session_prompt_tokens
    completion = wrapped.session_completion_tokens
    assert prompt > 0, "prompt counter must advance on successful chat"
    assert completion > 0, "completion counter must advance on successful chat"
    assert wrapped.session_total_tokens == prompt + completion


@pytest.mark.asyncio
async def test_failed_chat_leaves_counters_unchanged(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Spec scenario `Failed chat leaves counters unchanged`."""
    wrapped = _build_tracked(tmp_path)

    async def _boom(*args: object, **kwargs: object) -> None:
        raise RuntimeError("upstream-500")

    monkeypatch.setattr(wrapped._inner, "chat", _boom)

    with pytest.raises(RuntimeError, match="upstream-500"):
        await wrapped.chat(
            messages=[Message(role="user", content="x")],
            response_model=_Plan,
        )

    assert wrapped.session_prompt_tokens == 0
    assert wrapped.session_completion_tokens == 0
    assert wrapped.session_total_tokens == 0
    # Failure record still lands per pre-existing D-022 contract.
    lines = (tmp_path / "llm_calls.jsonl").read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1


@pytest.mark.asyncio
async def test_embed_advances_prompt_counter_only(tmp_path: Path) -> None:
    """Spec scenario `Embed path contributes to prompt counter only`."""
    wrapped = _build_tracked(tmp_path)

    await wrapped.embed(["hello", "world"])

    # `embed` reports `embed_tokens` (treated as prompt-side input) and
    # zero completion tokens — the embed-response has no completion
    # content, so completion counter must stay at 0.
    assert wrapped.session_prompt_tokens > 0
    assert wrapped.session_completion_tokens == 0
    assert wrapped.session_total_tokens == wrapped.session_prompt_tokens


@pytest.mark.asyncio
async def test_counters_are_per_instance_not_shared(tmp_path: Path) -> None:
    """Spec scenario `Counters are per-instance not shared`."""
    ws_a = tmp_path / "a"
    ws_b = tmp_path / "b"
    ws_a.mkdir()
    ws_b.mkdir()
    wrapped_a = _build_tracked(ws_a)
    wrapped_b = _build_tracked(ws_b)

    await wrapped_a.chat(
        messages=[Message(role="user", content="driving instance a")],
        response_model=_Plan,
    )

    # A advanced, B is untouched.
    assert wrapped_a.session_total_tokens > 0
    assert wrapped_b.session_prompt_tokens == 0
    assert wrapped_b.session_completion_tokens == 0
    assert wrapped_b.session_total_tokens == 0
