"""Tests for `codebus_agent.providers.pricing`.

Backs spec MODIFIED Requirement
``UsageTracker writes token_usage.jsonl`` Scenarios
``Known chat model writes non-zero cost_usd`` and
``Unknown chat model logs warning and writes zero cost_usd``
(review-backlog-cleanup).

These tests pin the pure-function contract: known model returns the
table-derived cost, unknown model returns ``0.0`` and logs a warning,
zero-token call always returns ``0.0`` without warning so healthz-style
zero-token probes do not pollute logs.
"""
from __future__ import annotations

import logging

import pytest


def test_known_model_returns_non_zero_cost() -> None:
    from codebus_agent.providers.pricing import estimate_chat_cost_usd

    cost = estimate_chat_cost_usd(
        "gpt-4o-mini-chat-v1",
        prompt_tokens=1000,
        completion_tokens=500,
    )

    expected = 1000 * 0.15 / 1_000_000 + 500 * 0.60 / 1_000_000
    assert cost == pytest.approx(expected)
    assert cost > 0.0


def test_unknown_model_returns_zero_and_warns(caplog) -> None:
    from codebus_agent.providers.pricing import estimate_chat_cost_usd

    with caplog.at_level(logging.WARNING, logger="codebus_agent.providers.pricing"):
        cost = estimate_chat_cost_usd(
            "fake-model",
            prompt_tokens=100,
            completion_tokens=50,
        )

    assert cost == 0.0
    warnings = [r for r in caplog.records if r.levelno >= logging.WARNING]
    assert any("fake-model" in r.getMessage() for r in warnings), (
        "expected a WARNING log naming the unknown model id; "
        f"got: {[r.getMessage() for r in warnings]}"
    )


def test_zero_tokens_returns_zero(caplog) -> None:
    """Zero-token calls (e.g., healthz probes) MUST NOT spam warnings.

    Even when the model is unknown, returning ``0.0`` for ``0`` tokens is
    arithmetically correct; emitting a warning here would flood operator
    logs without conveying useful information.
    """
    from codebus_agent.providers.pricing import estimate_chat_cost_usd

    with caplog.at_level(logging.WARNING, logger="codebus_agent.providers.pricing"):
        cost = estimate_chat_cost_usd(
            "fake-model",
            prompt_tokens=0,
            completion_tokens=0,
        )

    assert cost == 0.0
    assert not [
        r for r in caplog.records if r.levelno >= logging.WARNING
    ], "zero-token call should not emit any warning"
