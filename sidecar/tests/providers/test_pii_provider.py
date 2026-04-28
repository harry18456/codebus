"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/pii-provider/spec.md
  Requirement: RuleBasedPIIProvider wraps existing default_rules
    Scenario: Default construction uses default_rules
    Scenario: Multiple matches returned in order
    Scenario: Empty input returns empty list
    Scenario: detect is async-callable from sync regex
  Requirement: MockPIIProvider supports test scripting
    Scenario: Script controls return value
    Scenario: No script returns empty
    Scenario: Script exhaustion returns empty
    Scenario: Mock records call inputs

Design context: D-033 Decision 1 / 4 / 7 — RuleBasedPIIProvider wraps
the existing ``default_rules()`` table without modifying any rule
patterns; MockPIIProvider mirrors MockProvider's script-driven pattern
for unit-test wiring.
"""
from __future__ import annotations

import asyncio

import pytest

from codebus_agent.providers.pii import (
    MockPIIProvider,
    PIISpan,
    RuleBasedPIIProvider,
)


# ---------------------------------------------------------------------------
# RuleBasedPIIProvider
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_rule_based_default_construction_detects_email() -> None:
    """Scenario: Default construction uses default_rules."""
    provider = RuleBasedPIIProvider()
    spans = await provider.detect("contact: alice@example.com")
    assert len(spans) == 1
    span = spans[0]
    assert span.kind == "email"
    assert span.value == "alice@example.com"
    # rule_id must match the identifier produced by the default rule table —
    # no rename, no synthetic id (D-033 Decision 4).
    assert span.rule_id == "pii_email_v1"


@pytest.mark.asyncio
async def test_rule_based_multiple_matches_ordered_by_start() -> None:
    """Scenario: Multiple matches returned in order."""
    provider = RuleBasedPIIProvider()
    spans = await provider.detect("a@b.com and c@d.com")
    assert len(spans) == 2
    # Spans MUST be ordered by ``start`` ascending.
    assert spans[0].start < spans[1].start
    assert spans[0].value == "a@b.com"
    assert spans[1].value == "c@d.com"


@pytest.mark.asyncio
async def test_rule_based_empty_input_returns_empty_list() -> None:
    """Scenario: Empty input returns empty list."""
    provider = RuleBasedPIIProvider()
    spans = await provider.detect("")
    assert spans == []


@pytest.mark.asyncio
async def test_rule_based_detect_resolves_in_single_loop_tick() -> None:
    """Scenario: detect is async-callable from sync regex.

    Pure-regex detection contains no real await suspension point — the
    coroutine MUST resolve synchronously when polled, so a single
    ``loop.run_until_complete`` call sees the result immediately rather
    than yielding control to other tasks.
    """
    provider = RuleBasedPIIProvider()
    coro = provider.detect("contact: alice@example.com")
    # ``send(None)`` advances the coroutine; for a non-suspending coroutine
    # this raises StopIteration with the return value attached.
    with pytest.raises(StopIteration) as exc_info:
        coro.send(None)
    spans = exc_info.value.value
    assert len(spans) == 1
    assert spans[0].kind == "email"


@pytest.mark.asyncio
async def test_rule_based_returns_pii_span_instances() -> None:
    """Provider returns ``PIISpan`` (not the legacy ``RuleMatch``)."""
    provider = RuleBasedPIIProvider()
    spans = await provider.detect("ip: 10.0.3.42")
    assert len(spans) == 1
    assert isinstance(spans[0], PIISpan)


# ---------------------------------------------------------------------------
# MockPIIProvider
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_mock_script_controls_return_value() -> None:
    """Scenario: Script controls return value."""
    scripted = PIISpan(
        rule_id="x", kind="email", start=0, end=4, value="abcd"
    )
    mock = MockPIIProvider(script=[[scripted]])
    spans = await mock.detect("anything")
    assert spans == [scripted]


@pytest.mark.asyncio
async def test_mock_no_script_returns_empty() -> None:
    """Scenario: No script returns empty."""
    mock = MockPIIProvider()
    spans = await mock.detect("contact: alice@example.com")
    assert spans == []


@pytest.mark.asyncio
async def test_mock_script_exhaustion_returns_empty() -> None:
    """Scenario: Script exhaustion returns empty."""
    mock = MockPIIProvider(script=[[]])
    first = await mock.detect("first")
    second = await mock.detect("second")
    assert first == []
    assert second == []


@pytest.mark.asyncio
async def test_mock_records_call_inputs() -> None:
    """Scenario: Mock records call inputs."""
    mock = MockPIIProvider()
    await mock.detect("foo")
    await mock.detect("bar")
    assert mock.calls == ["foo", "bar"]
