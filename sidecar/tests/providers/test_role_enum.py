"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: ProviderRole enumerates call-site categories
    Scenario: ProviderRole exposes four members
    Scenario: ProviderRole is a StrEnum

Design D-003 + llm-role-routing change design §1:
`ProviderRole` is a four-valued `StrEnum` (REASONING / JUDGE / CHAT /
EMBED) — no vision / multimodal dimension (D-028).
"""
from __future__ import annotations

from enum import StrEnum

from codebus_agent.providers import ProviderRole


def test_provider_role_has_exactly_four_members() -> None:
    """Scenario: ProviderRole exposes four members."""
    members = {member.name for member in ProviderRole}
    assert members == {"REASONING", "JUDGE", "CHAT", "EMBED"}


def test_provider_role_values_are_lowercase_of_names() -> None:
    """Scenario: ProviderRole exposes four members — value convention."""
    assert ProviderRole.REASONING.value == "reasoning"
    assert ProviderRole.JUDGE.value == "judge"
    assert ProviderRole.CHAT.value == "chat"
    assert ProviderRole.EMBED.value == "embed"


def test_provider_role_is_str_enum_subclass() -> None:
    """Scenario: ProviderRole is a StrEnum — subclass of StrEnum."""
    assert issubclass(ProviderRole, StrEnum)


def test_provider_role_member_compares_equal_to_its_string_value() -> None:
    """Scenario: ProviderRole is a StrEnum — equality with plain string."""
    assert ProviderRole.JUDGE == "judge"
    assert ProviderRole.REASONING == "reasoning"
    assert ProviderRole.CHAT == "chat"
    assert ProviderRole.EMBED == "embed"
