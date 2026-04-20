"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: No outbound LLM traffic during M1 (MODIFIED)
    Scenario: Only MockProvider registered for every role
    Scenario: Integration test asserts no outbound calls across roles

Design llm-role-routing §3: the M1 zero-outbound invariant now fans
out across every `ProviderRole` — a registry constructed with a
non-Mock inner anywhere should already have died at `TrackedProvider`
wrap time; this test locks that invariant in at the per-role level.
"""
from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import BaseModel

from codebus_agent.providers import (
    LLMCallLogger,
    MockProvider,
    ProviderRegistry,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)
from codebus_agent.providers.protocol import Message


class _Plan(BaseModel):
    title: str = ""
    steps: int = 0


def _wrap(tmp_path: Path, *, role: ProviderRole) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / f"{role.value}_token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / f"{role.value}_llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(role=role), tracker=tracker, logger=logger, role=role
    )


def _all_roles_registry(tmp_path: Path) -> ProviderRegistry:
    return ProviderRegistry(
        {role: _wrap(tmp_path, role=role) for role in ProviderRole}
    )


def test_every_role_underlying_provider_is_mock(tmp_path: Path) -> None:
    """Scenario: Only MockProvider registered for every role."""
    registry = _all_roles_registry(tmp_path)

    for role in ProviderRole:
        tracked = registry.get(role)
        assert isinstance(tracked, TrackedProvider)
        assert type(tracked._inner) is MockProvider


@pytest.mark.asyncio
async def test_no_outbound_http_across_all_roles(
    tmp_path: Path, block_outbound_sockets: list
) -> None:
    """Scenario: Integration test asserts no outbound calls across roles."""
    registry = _all_roles_registry(tmp_path)

    for role in (ProviderRole.REASONING, ProviderRole.JUDGE, ProviderRole.CHAT):
        provider = registry.get(role)
        await provider.chat(
            messages=[Message(role="user", content="hi")],
            response_model=_Plan,
        )

    embed_provider = registry.get(ProviderRole.EMBED)
    await embed_provider.embed(texts=["a", "b"])

    assert block_outbound_sockets == [], (
        f"unexpected outbound connections across roles: {block_outbound_sockets!r}"
    )
