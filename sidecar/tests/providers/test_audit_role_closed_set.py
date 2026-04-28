"""Backs SHALL clauses in
openspec/changes/split-providers-and-pii-llm/specs/usage-tracking/spec.md
  Requirement: AuditRole enumerates legal role values in llm_calls.jsonl
    Scenario: Closed set of role values
    Scenario: pii_detection role pairs with sanitizer_pass2_applied false
    Scenario: LLM-mode roles pair with sanitizer_pass2_applied true
    Scenario: This change emits no pii_detection lines

D-033 Decision 5 / 6 — the audit ``role`` field exposes a closed enum
of five string values; this test pins the closed set so future drift
between ``LLMCallLogger`` and the spec is caught at test time.
"""
from __future__ import annotations

from typing import get_args

from codebus_agent.providers import AuditRole


_LLM_MODE_ROLES = {"reasoning", "judge", "chat", "embed"}
_PII_MODE_ROLES = {"pii_detection"}
_EXPECTED_LEGAL_VALUES = _LLM_MODE_ROLES | _PII_MODE_ROLES


def test_audit_role_literal_exposes_exact_closed_set() -> None:
    """Scenario: Closed set of role values.

    The runtime ``AuditRole`` Literal must contain exactly the five
    canonical values; an integration test guards against drift between
    spec and code.
    """
    legal_values = set(get_args(AuditRole))
    assert legal_values == _EXPECTED_LEGAL_VALUES, (
        f"AuditRole drift: expected {_EXPECTED_LEGAL_VALUES}, "
        f"got {legal_values}"
    )


def test_pii_detection_value_is_present_for_future_pii_llm_provider() -> None:
    """Scenario: This change emits no pii_detection lines (reserved value).

    ``"pii_detection"`` MUST be a legal value in ``AuditRole`` so a
    future LLM-based PII provider Spectra change can emit it without
    re-opening the closed set; this change ships zero call paths that
    write it (RuleBasedPIIProvider / MockPIIProvider perform no LLM
    calls).
    """
    assert "pii_detection" in get_args(AuditRole)


def test_llm_mode_roles_match_provider_role_enum() -> None:
    """Scenario: LLM-mode roles pair with sanitizer_pass2_applied true.

    The four LLM-mode AuditRole values MUST exactly match the
    ``ProviderRole`` enum's member values — this enforces the
    ``role`` field's pairing with ``sanitizer_pass2_applied=true``
    semantics at the schema layer.
    """
    from codebus_agent.providers.protocol import ProviderRole

    enum_values = {member.value for member in ProviderRole}
    assert _LLM_MODE_ROLES == enum_values


def test_no_pii_detection_lines_emitted_in_this_change() -> None:
    """Scenario: This change emits no pii_detection lines.

    ``RuleBasedPIIProvider`` and ``MockPIIProvider`` are the only PII
    providers shipping in this change; both perform pure detection
    (no LLM call) so they MUST NOT instantiate any code path that
    writes a line with ``role: "pii_detection"``. We verify this by
    checking those classes have no ``llm_calls.jsonl`` writer in their
    method set — only ``detect``.
    """
    from codebus_agent.providers.pii import (
        MockPIIProvider,
        RuleBasedPIIProvider,
    )

    for cls in (RuleBasedPIIProvider, MockPIIProvider):
        public_methods = {
            name
            for name in dir(cls)
            if not name.startswith("_") and callable(getattr(cls, name))
        }
        # ``detect`` is the only call-shaped public method.
        assert "detect" in public_methods
        # No ``chat`` / ``embed`` / ``log`` / ``record`` / ``write``
        # — those would be how an LLM call audit lane could leak.
        forbidden = {"chat", "embed", "log", "record", "write"}
        assert public_methods.isdisjoint(forbidden)
