"""TrackedProvider chat cost is sourced from the pricing table.

Backs spec MODIFIED Requirement
``UsageTracker writes token_usage.jsonl`` Scenarios
``Known chat model writes non-zero cost_usd`` and
``Unknown chat model logs warning and writes zero cost_usd``
(review-backlog-cleanup), per D-021 (token / cost ledger) and D-022
(wire payload). The Stage 4 review backlog Cat 3 #4 finding was that
chat ``cost_usd`` was hard-coded to ``0.0`` while embed already had a
working cost path. This file pins the post-fix invariants:

1. A known-model chat call writes ``cost_usd > 0`` to
   ``token_usage.jsonl`` AND the matching ``usage_delta`` SSE event
   carries the same value (audit and wire agree).
2. An unknown-model chat call records ``0.0`` and emits a WARNING log
   naming the unknown model id.
"""
from __future__ import annotations

import json
import logging
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers import pricing
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
    emitter: object | None = None,
) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    call_logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=call_logger,
        role=ProviderRole.CHAT,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
        rules_version="test-v1",
        default_module="chat",
        emitter=emitter,
    )


@pytest.mark.asyncio
async def test_chat_call_writes_non_zero_cost_usd_for_known_model(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Known-model chat call records pricing-derived cost in both audit and SSE."""
    # MockProvider's `_chat_model_id` returns `"mock-chat-v1"`. The production
    # entry is `(0.0, 0.0)` so Mock-driven test runs don't pollute cost data;
    # we monkeypatch a non-zero entry here to exercise the pricing-lookup path
    # without touching the real `gpt-4o-mini-chat-v1` row.
    monkeypatch.setitem(pricing._CHAT_PRICING, "mock-chat-v1", (0.15, 0.60))

    spy = _SpyEmitter()
    wrapped = _build_tracked(tmp_path, emitter=spy)

    await wrapped.chat(
        messages=[Message(role="user", content="hello world")],
        response_model=_Plan,
    )

    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    assert len(usage_lines) == 1
    line = usage_lines[0]

    expected = (
        line["input_tokens"] * 0.15 / 1_000_000
        + line["output_tokens"] * 0.60 / 1_000_000
    )
    assert line["cost_usd"] == pytest.approx(expected)
    assert line["cost_usd"] > 0.0
    assert line["model"] == "mock-chat-v1"

    deltas = [e for e in spy.events if e["type"] == "usage_delta"]
    assert len(deltas) == 1
    assert deltas[0]["cost_usd"] == pytest.approx(line["cost_usd"]), (
        "audit and wire MUST agree on cost_usd — same pricing-table value "
        "feeds both `token_usage.jsonl` and `usage_delta` SSE event"
    )


@pytest.mark.asyncio
async def test_chat_call_writes_zero_cost_usd_for_unknown_model_and_warns(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    caplog: pytest.LogCaptureFixture,
) -> None:
    """Unknown-model chat call records 0.0 and logs WARNING.

    We monkeypatch the pricing table empty so the resolved
    ``mock-chat-v1`` model id falls outside the known set, exercising
    the unknown-model branch without inventing a fake provider.
    """
    monkeypatch.setattr(pricing, "_CHAT_PRICING", {})

    wrapped = _build_tracked(tmp_path)

    with caplog.at_level(logging.WARNING, logger="codebus_agent.providers.pricing"):
        await wrapped.chat(
            messages=[Message(role="user", content="hello")],
            response_model=_Plan,
        )

    usage_lines = _read_lines(tmp_path / "token_usage.jsonl")
    assert len(usage_lines) == 1
    assert usage_lines[0]["cost_usd"] == 0.0

    warnings = [r for r in caplog.records if r.levelno >= logging.WARNING]
    assert any("mock-chat-v1" in r.getMessage() for r in warnings), (
        f"expected WARNING naming the unknown model id; got: "
        f"{[r.getMessage() for r in warnings]}"
    )
