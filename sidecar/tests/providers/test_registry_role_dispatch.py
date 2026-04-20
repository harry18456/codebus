"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: Registry dispatches provider by role
    Scenario: Registry returns role-specific provider
    Scenario: Registry raises on missing role
"""
from __future__ import annotations

from pathlib import Path

import pytest

from codebus_agent.providers import (
    LLMCallLogger,
    MockProvider,
    ProviderRegistry,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)


def _wrap(
    tmp_path: Path, *, name: str, role: ProviderRole = ProviderRole.CHAT
) -> TrackedProvider:
    tracker = UsageTracker(tmp_path / f"{name}_token_usage.jsonl")
    logger = LLMCallLogger(tmp_path / f"{name}_llm_calls.jsonl")
    return TrackedProvider(
        MockProvider(), tracker=tracker, logger=logger, role=role
    )


def test_registry_returns_role_specific_provider(tmp_path: Path) -> None:
    """Scenario: Registry returns role-specific provider."""
    reasoning_provider = _wrap(
        tmp_path, name="reasoning", role=ProviderRole.REASONING
    )
    judge_provider = _wrap(tmp_path, name="judge", role=ProviderRole.JUDGE)

    registry = ProviderRegistry(
        {
            ProviderRole.REASONING: reasoning_provider,
            ProviderRole.JUDGE: judge_provider,
        }
    )

    assert registry.get(ProviderRole.JUDGE) is judge_provider
    assert registry.get(ProviderRole.REASONING) is reasoning_provider


def test_registry_raises_key_error_on_missing_role(tmp_path: Path) -> None:
    """Scenario: Registry raises on missing role."""
    registry = ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap(
                tmp_path, name="reasoning", role=ProviderRole.REASONING
            )
        }
    )

    with pytest.raises(KeyError, match="embed"):
        registry.get(ProviderRole.EMBED)


def test_registry_exposes_registered_roles(tmp_path: Path) -> None:
    registry = ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap(
                tmp_path, name="reasoning", role=ProviderRole.REASONING
            ),
            ProviderRole.JUDGE: _wrap(
                tmp_path, name="judge", role=ProviderRole.JUDGE
            ),
        }
    )
    assert set(registry.roles) == {ProviderRole.REASONING, ProviderRole.JUDGE}
