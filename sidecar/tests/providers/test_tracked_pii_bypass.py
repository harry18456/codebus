"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/pii-provider/spec.md
  Requirement: TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES
    Scenario: PII_ALLOWED_INNER_TYPES exposes initial allowlist
    Scenario: PII and LLM allowlists are disjoint
    Scenario: Non-allowlisted PII inner rejected at construction
    Scenario: Source-grep test pins allowlist to spec
  Requirement: TrackedProvider auto-bypasses Pass 2 for PII inner
    Scenario: PII mode bypasses Pass 2
    Scenario: No external flag can trigger bypass in LLM mode
    Scenario: Mode is determined once at construction
    Scenario: Wrong-mode method calls raise

Design context: D-033 Decision 2 — TrackedProvider 用 marker dispatch,
不拆 TrackedPIIProvider. The mode (llm / pii) is determined at __init__
by ``type(inner)`` allowlist membership; PII mode skips Pass 2 entirely.
"""
from __future__ import annotations

import inspect
from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers import tracked as tracked_module
from codebus_agent.providers.llm_call_logger import LLMCallLogger
from codebus_agent.providers.mock import MockProvider
from codebus_agent.providers.pii import (
    MockPIIProvider,
    PIISpan,
    RuleBasedPIIProvider,
)
from codebus_agent.providers.protocol import Message, ProviderRole
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.providers.usage_tracker import UsageTracker
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


class _Plan(BaseModel):
    title: str = ""


def _build_llm_tracked(tmp_path: Path) -> TrackedProvider:
    """Build a TrackedProvider in LLM mode with full audit wiring."""
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(),
        tracker=tracker,
        logger=logger,
        role=ProviderRole.CHAT,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl"),
        rules_version="test-v1",
    )


# ---------------------------------------------------------------------------
# Allowlist gating
# ---------------------------------------------------------------------------


def test_pii_allowed_inner_types_initial_allowlist() -> None:
    """Scenario: PII_ALLOWED_INNER_TYPES exposes initial allowlist."""
    assert TrackedProvider.PII_ALLOWED_INNER_TYPES == frozenset(
        {RuleBasedPIIProvider, MockPIIProvider}
    )


def test_pii_and_llm_allowlists_are_disjoint() -> None:
    """Scenario: PII and LLM allowlists are disjoint."""
    intersection = (
        TrackedProvider.ALLOWED_INNER_TYPES
        & TrackedProvider.PII_ALLOWED_INNER_TYPES
    )
    assert intersection == frozenset()


def test_non_allowlisted_pii_inner_rejected_at_construction() -> None:
    """Scenario: Non-allowlisted PII inner rejected at construction."""

    class _UnregisteredPIIProvider:
        async def detect(self, text: str) -> list[PIISpan]:
            return []

    with pytest.raises(TypeError) as exc_info:
        TrackedProvider(_UnregisteredPIIProvider())
    msg = str(exc_info.value)
    assert "_UnregisteredPIIProvider" in msg


def test_source_grep_pins_pii_allowlist_to_spec() -> None:
    """Scenario: Source-grep test pins allowlist to spec.

    The PII_ALLOWED_INNER_TYPES literal in tracked.py must reference
    exactly RuleBasedPIIProvider + MockPIIProvider (post-this-change).
    """
    src = inspect.getsource(tracked_module)
    # Both class names must appear as literal references in the module.
    assert "RuleBasedPIIProvider" in src
    assert "MockPIIProvider" in src
    # The allowlist runtime value is the canonical assertion.
    assert TrackedProvider.PII_ALLOWED_INNER_TYPES == frozenset(
        {RuleBasedPIIProvider, MockPIIProvider}
    )


# ---------------------------------------------------------------------------
# PII mode bypass behaviour
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_pii_mode_bypasses_pass_2(tmp_path: Path) -> None:
    """Scenario: PII mode bypasses Pass 2.

    The wrapped RuleBasedPIIProvider receives the original (un-redacted)
    text, and SanitizerEngine.sanitize is NEVER invoked even when a
    sanitizer is supplied at construction time.
    """
    sanitize_audit = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")

    class _SpyEngine(SanitizerEngine):
        sanitize_calls: int = 0

        def sanitize(self, text, source):  # type: ignore[no-untyped-def, override]
            type(self).sanitize_calls += 1
            return super().sanitize(text, source)

    spy_engine = _SpyEngine()
    _SpyEngine.sanitize_calls = 0

    inner = RuleBasedPIIProvider()
    wrapper = TrackedProvider(
        inner,
        sanitizer=spy_engine,
        sanitizer_audit=sanitize_audit,
        rules_version="test-v1",
    )
    spans = await wrapper.detect("contact: alice@example.com")
    assert len(spans) == 1
    assert spans[0].kind == "email"
    # Sanitizer.sanitize MUST NOT have been invoked even though a sanitizer
    # was supplied at construction time.
    assert _SpyEngine.sanitize_calls == 0


def test_no_skip_sanitizer_flag_accepted(tmp_path: Path) -> None:
    """Scenario: No external flag can trigger bypass in LLM mode.

    The constructor MUST NOT accept any kwarg named ``skip_sanitizer``,
    ``bypass_pass2``, or equivalent. Python raises TypeError for unknown
    kwargs by default — this test pins that behaviour.
    """
    tracker = UsageTracker(tmp_path / "token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / "llm_calls.jsonl")
    sanitizer_audit = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")
    with pytest.raises(TypeError):
        TrackedProvider(
            MockProvider(),
            tracker=tracker,
            logger=logger,
            role=ProviderRole.CHAT,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=sanitizer_audit,
            rules_version="test-v1",
            skip_sanitizer=True,  # type: ignore[call-arg]
        )


def test_mode_determined_once_at_construction(tmp_path: Path) -> None:
    """Scenario: Mode is determined once at construction."""
    pii_wrapper = TrackedProvider(RuleBasedPIIProvider())
    assert pii_wrapper._mode == "pii"

    llm_wrapper = _build_llm_tracked(tmp_path)
    assert llm_wrapper._mode == "llm"


@pytest.mark.asyncio
async def test_chat_in_pii_mode_raises() -> None:
    """Scenario: Wrong-mode method calls raise (chat in pii mode)."""
    wrapper = TrackedProvider(RuleBasedPIIProvider())
    with pytest.raises(RuntimeError) as exc_info:
        await wrapper.chat(
            messages=[Message(role="user", content="hi")],
            response_model=_Plan,
        )
    msg = str(exc_info.value)
    assert "pii" in msg.lower() and "chat" in msg.lower()


@pytest.mark.asyncio
async def test_embed_in_pii_mode_raises() -> None:
    """Scenario: Wrong-mode method calls raise (embed in pii mode)."""
    wrapper = TrackedProvider(RuleBasedPIIProvider())
    with pytest.raises(RuntimeError) as exc_info:
        await wrapper.embed(["foo"])
    msg = str(exc_info.value)
    assert "pii" in msg.lower() and "embed" in msg.lower()


@pytest.mark.asyncio
async def test_detect_in_llm_mode_raises(tmp_path: Path) -> None:
    """Scenario: Wrong-mode method calls raise (detect in llm mode)."""
    wrapper = _build_llm_tracked(tmp_path)
    with pytest.raises(RuntimeError) as exc_info:
        await wrapper.detect("foo")
    msg = str(exc_info.value)
    assert "llm" in msg.lower() and "detect" in msg.lower()


@pytest.mark.asyncio
async def test_pii_mode_detect_returns_inner_spans() -> None:
    """PII mode wrapper forwards detect call to inner.

    Mock the inner with a script and verify the wrapper returns exactly
    those spans without modification.
    """
    scripted = PIISpan(
        rule_id="x", kind="email", start=0, end=4, value="abcd"
    )
    inner = MockPIIProvider(script=[[scripted]])
    wrapper = TrackedProvider(inner)
    spans = await wrapper.detect("anything")
    assert spans == [scripted]
