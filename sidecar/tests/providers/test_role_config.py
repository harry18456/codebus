"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: RoleConfig binds provider, model, and default parameters per role
    Scenario: RoleConfig exposes required fields
    Scenario: RoleConfig is frozen

Design llm-role-routing §2: `RoleConfig` is a frozen dataclass with
`provider_id`, `model`, `temperature=0.2`, `max_tokens=None` defaults.
"""
from __future__ import annotations

import dataclasses

import pytest

from codebus_agent.providers import RoleConfig


def test_role_config_exposes_required_fields_with_defaults() -> None:
    """Scenario: RoleConfig exposes required fields."""
    cfg = RoleConfig(provider_id="mock", model="mock-judge")

    assert cfg.provider_id == "mock"
    assert cfg.model == "mock-judge"
    assert cfg.temperature == 0.2
    assert cfg.max_tokens is None


def test_role_config_accepts_overrides() -> None:
    """Scenario: RoleConfig exposes required fields — callers may override."""
    cfg = RoleConfig(
        provider_id="mock",
        model="mock-chat",
        temperature=0.7,
        max_tokens=4096,
    )
    assert cfg.temperature == 0.7
    assert cfg.max_tokens == 4096


def test_role_config_field_types() -> None:
    """Scenario: RoleConfig exposes required fields — field annotation shape."""
    fields = {f.name: f for f in dataclasses.fields(RoleConfig)}
    assert set(fields) == {"provider_id", "model", "temperature", "max_tokens"}


def test_role_config_is_frozen() -> None:
    """Scenario: RoleConfig is frozen."""
    cfg = RoleConfig(provider_id="mock", model="mock-judge")
    with pytest.raises(dataclasses.FrozenInstanceError):
        cfg.temperature = 0.9  # type: ignore[misc]


def test_role_config_forbids_mutating_provider_id() -> None:
    """Scenario: RoleConfig is frozen — all fields immutable, not just one."""
    cfg = RoleConfig(provider_id="mock", model="mock-judge")
    with pytest.raises(dataclasses.FrozenInstanceError):
        cfg.provider_id = "other"  # type: ignore[misc]
