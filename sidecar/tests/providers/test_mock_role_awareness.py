"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: MockProvider records role for audit reachability
    Scenario: Mock provider exposes role
    Scenario: Mock without role remains backward compatible

Design llm-role-routing §5: single MockProvider class carries an
optional `role` attribute — avoids per-role subclass proliferation
while keeping the audit trail attributable.
"""
from __future__ import annotations

import pytest
from pydantic import BaseModel

from codebus_agent.providers import MockProvider, ProviderRole
from codebus_agent.providers.protocol import Message


class _Plan(BaseModel):
    title: str = ""
    steps: int = 0


def test_mock_provider_exposes_role_when_set() -> None:
    """Scenario: Mock provider exposes role."""
    provider = MockProvider(role=ProviderRole.REASONING)
    assert provider.role is ProviderRole.REASONING


def test_mock_provider_role_attribute_is_none_by_default() -> None:
    """Scenario: Mock without role remains backward compatible — attribute present, None."""
    provider = MockProvider()
    assert hasattr(provider, "role")
    assert provider.role is None


@pytest.mark.asyncio
async def test_mock_without_role_still_serves_chat_and_embed() -> None:
    """Scenario: Mock without role remains backward compatible — M1 chat + embed paths."""
    provider = MockProvider()

    chat_result = await provider.chat(
        messages=[Message(role="user", content="hi")],
        response_model=_Plan,
    )
    embed_result = await provider.embed(texts=["a"])

    assert isinstance(chat_result, _Plan)
    assert len(embed_result.vectors) == 1


def test_mock_role_can_be_any_provider_role_value() -> None:
    """All four roles are accepted — no implicit filtering."""
    for role in ProviderRole:
        provider = MockProvider(role=role)
        assert provider.role is role
