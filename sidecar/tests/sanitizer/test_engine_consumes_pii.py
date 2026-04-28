"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/sanitizer/spec.md
  Requirement: SanitizerEngine exposes pure `sanitize` interface (MODIFIED)
    Scenario: Pass 1 sanitize returns replaced text and audit entries
    Scenario: Same value replaced with same placeholder within single call
    Scenario: Placeholder index resets across sanitize calls
    Scenario: Fail-closed on engine error
    Scenario: Engine constructor accepts PIIProvider, not rules
    Scenario: Engine has no direct rule imports

Design context: D-033 Decision 3 — PIIProvider.detect() is always async,
so SanitizerEngine.sanitize follows. The Engine retains placeholder
rendering + audit emission ownership and delegates only span discovery
to the injected PIIProvider.
"""
from __future__ import annotations

import inspect

import pytest

from codebus_agent.providers.pii import (
    MockPIIProvider,
    PIISpan,
    RuleBasedPIIProvider,
)
from codebus_agent.sanitizer.engine import SanitizerEngine, SanitizerError
from codebus_agent.sanitizer.engine import FileSource


@pytest.mark.asyncio
async def test_pass1_async_sanitize_returns_replaced_text_and_audit() -> None:
    """Scenario: Pass 1 sanitize returns replaced text and audit entries."""
    engine = SanitizerEngine(pii_provider=RuleBasedPIIProvider())
    result = await engine.sanitize(
        "contact: alice@example.com",
        source=FileSource(path="src/app.py"),
    )
    assert "<REDACTED:email#1>" in result.text
    assert "alice@example.com" not in result.text
    assert len(result.entries) == 1
    entry = result.entries[0]
    assert entry.kind == "email"
    assert entry.placeholder_index == 1
    assert entry.rule_id == "pii_email_v1"


@pytest.mark.asyncio
async def test_same_value_same_placeholder_within_single_call() -> None:
    """Scenario: Same value replaced with same placeholder within single call."""
    engine = SanitizerEngine(pii_provider=RuleBasedPIIProvider())
    result = await engine.sanitize(
        "a: alice@example.com, b: alice@example.com",
        source=FileSource(path="src/a.py"),
    )
    # Both occurrences MUST be replaced with the same placeholder.
    assert result.text.count("<REDACTED:email#1>") == 2
    # Audit entries MUST contain exactly one entry for the (kind, value) pair.
    assert len(result.entries) == 1


@pytest.mark.asyncio
async def test_placeholder_index_resets_across_calls() -> None:
    """Scenario: Placeholder index resets across sanitize calls."""
    engine = SanitizerEngine(pii_provider=RuleBasedPIIProvider())
    a = await engine.sanitize(
        "a@b.com", source=FileSource(path="src/a.py")
    )
    b = await engine.sanitize(
        "c@d.com", source=FileSource(path="src/b.py")
    )
    assert a.entries[0].placeholder_index == 1
    assert b.entries[0].placeholder_index == 1


@pytest.mark.asyncio
async def test_fail_closed_when_pii_provider_raises() -> None:
    """Scenario: Fail-closed on engine error.

    When the injected PIIProvider raises during ``detect``, the Engine
    MUST raise ``SanitizerError`` chained via ``__cause__`` and MUST NOT
    return any partial text.
    """

    class _BoomProvider:
        async def detect(self, text: str) -> list[PIISpan]:
            raise RuntimeError("simulated detect failure")

    engine = SanitizerEngine(pii_provider=_BoomProvider())
    with pytest.raises(SanitizerError) as exc_info:
        await engine.sanitize(
            "contact: alice@example.com",
            source=FileSource(path="src/app.py"),
        )
    # Source identifier appears in the error message.
    assert "src/app.py" in str(exc_info.value)
    # Originating exception chained via __cause__.
    assert isinstance(exc_info.value.__cause__, RuntimeError)


def test_engine_rejects_legacy_rules_kwarg() -> None:
    """Scenario: Engine constructor accepts PIIProvider, not rules.

    Passing the legacy ``rules`` keyword MUST raise TypeError indicating
    the argument has been removed.
    """
    with pytest.raises(TypeError):
        SanitizerEngine(rules=[])  # type: ignore[call-arg]


def test_engine_accepts_pii_provider() -> None:
    """Scenario: Engine constructor accepts PIIProvider, not rules.

    `SanitizerEngine(pii_provider=RuleBasedPIIProvider())` MUST succeed.
    """
    engine = SanitizerEngine(pii_provider=RuleBasedPIIProvider())
    assert engine is not None


@pytest.mark.asyncio
async def test_engine_uses_injected_pii_provider() -> None:
    """Engine delegates span discovery to the injected PIIProvider.

    Inject a MockPIIProvider with a scripted span; verify that the
    Engine's output text contains the corresponding placeholder even
    though the input text would not match any default rule.
    """
    scripted = PIISpan(
        rule_id="custom_rule_v1",
        kind="email",
        start=0,
        end=4,
        value="abcd",
    )
    mock = MockPIIProvider(script=[[scripted]])
    engine = SanitizerEngine(pii_provider=mock)
    result = await engine.sanitize(
        "abcd does not match any default rule",
        source=FileSource(path="src/test.py"),
    )
    # The engine consumed the mock's span and applied a placeholder.
    assert "<REDACTED:email#1>" in result.text
    assert "abcd" not in result.text
    assert mock.calls == ["abcd does not match any default rule"]


@pytest.mark.asyncio
async def test_make_default_engine_preserves_rule_id_stability() -> None:
    """Scenario: rule_id stability across structural change.

    The ``rule_id`` values produced through the new
    ``make_default_engine()`` factory MUST equal the values the
    pre-D-033 ``SanitizerEngine(rules=default_rules())`` would have
    produced — i.e., the rename / move from ``SanitizerEngine``-owned
    rules to ``RuleBasedPIIProvider``-owned rules MUST NOT change any
    audit ``rule_id`` field.
    """
    from codebus_agent.sanitizer import make_default_engine

    engine = make_default_engine()

    # rule_id values captured from the sanitizer-safety-chain
    # implementation (pre-D-033). Drift between this fixture and the
    # actual rule_id emitted = a breaking schema change.
    cases = [
        ("alice@example.com", "pii_email_v1"),
        ("0912-345-678", "pii_tw_mobile_v1"),
        ("A123456789", "pii_tw_national_id_v1"),
        ("10.0.3.42", "net_rfc1918_a_v1"),
        ("db01.corp", "net_internal_tld_v1"),
    ]
    for text, expected_rule_id in cases:
        result = await engine.sanitize(
            text, source=FileSource(path="src/fixture.py")
        )
        assert (
            result.entries[0].rule_id == expected_rule_id
        ), (
            f"rule_id drift detected for {text!r}: "
            f"expected {expected_rule_id!r}, got {result.entries[0].rule_id!r}"
        )


def test_engine_has_no_direct_rule_imports() -> None:
    """Scenario: Engine has no direct rule imports.

    ``codebus_agent.sanitizer.engine`` MUST NOT import ``Rule`` /
    ``RegexRule`` / ``DetectSecretsRule`` / ``default_rules`` — all
    rule-related symbols are reachable only via the injected PIIProvider.
    """
    from codebus_agent.sanitizer import engine as engine_module

    src = inspect.getsource(engine_module)
    # Forbidden imports — these symbols MUST NOT appear in import lines.
    forbidden = ["RegexRule", "DetectSecretsRule", "default_rules"]
    for symbol in forbidden:
        # Look for `import <symbol>` or `<symbol>,` patterns. Simpler:
        # the symbol MUST NOT appear at all (engine no longer references rules).
        assert symbol not in src, (
            f"engine.py MUST NOT reference {symbol!r} — rule logic moved to "
            f"RuleBasedPIIProvider per D-033 Decision 4"
        )
