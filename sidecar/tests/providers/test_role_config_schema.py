"""Backs SHALL clauses in
openspec/changes/llm-role-routing/specs/llm-provider/spec.md
  Requirement: Config schema declares llm.roles map
    Scenario: Config roles map parses into RoleConfig instances
    Scenario: Config rejects unknown role key

Design llm-role-routing §4: the `llm.roles` map keyed by lowercase
role name replaces the M1-era flat `llm.chat_provider` /
`llm.embed_provider` fields.  `llm.llm_disabled` is retained as a
kill switch for later changes.
"""
from __future__ import annotations

import pytest

from codebus_agent.providers import ProviderRole, RoleConfig
from codebus_agent.providers.config import parse_llm_roles


def test_config_roles_map_parses_single_role_with_defaults() -> None:
    """Scenario: Config roles map parses into RoleConfig instances."""
    config = {
        "llm": {
            "roles": {
                "judge": {"provider_id": "mock", "model": "mock-judge"}
            }
        }
    }
    parsed = parse_llm_roles(config)

    assert parsed == {
        ProviderRole.JUDGE: RoleConfig(
            provider_id="mock",
            model="mock-judge",
            temperature=0.2,
            max_tokens=None,
        )
    }


def test_config_roles_map_parses_all_four_roles() -> None:
    config = {
        "llm": {
            "roles": {
                "reasoning": {"provider_id": "mock", "model": "mock-reasoning"},
                "judge": {"provider_id": "mock", "model": "mock-judge"},
                "chat": {"provider_id": "mock", "model": "mock-chat"},
                "embed": {"provider_id": "mock", "model": "mock-embed"},
            }
        }
    }
    parsed = parse_llm_roles(config)

    assert set(parsed.keys()) == {
        ProviderRole.REASONING,
        ProviderRole.JUDGE,
        ProviderRole.CHAT,
        ProviderRole.EMBED,
    }
    assert parsed[ProviderRole.EMBED].model == "mock-embed"


def test_config_rejects_unknown_role_key_and_lists_valid_names() -> None:
    """Scenario: Config rejects unknown role key."""
    config = {
        "llm": {"roles": {"unknown_role": {"provider_id": "x", "model": "y"}}}
    }

    with pytest.raises(ValueError) as exc_info:
        parse_llm_roles(config)

    msg = str(exc_info.value)
    assert "unknown_role" in msg
    for name in ("reasoning", "judge", "chat", "embed"):
        assert name in msg


def test_config_accepts_temperature_and_max_tokens_overrides() -> None:
    config = {
        "llm": {
            "roles": {
                "chat": {
                    "provider_id": "mock",
                    "model": "mock-chat",
                    "temperature": 0.7,
                    "max_tokens": 4096,
                }
            }
        }
    }
    parsed = parse_llm_roles(config)

    assert parsed[ProviderRole.CHAT].temperature == 0.7
    assert parsed[ProviderRole.CHAT].max_tokens == 4096


def test_config_returns_empty_map_when_roles_absent() -> None:
    """No `llm.roles` key → empty map (caller decides how to treat it)."""
    assert parse_llm_roles({}) == {}
    assert parse_llm_roles({"llm": {}}) == {}


def test_config_rejects_uppercase_role_key() -> None:
    """Design §4 specifies lowercase-only role keys."""
    config = {"llm": {"roles": {"JUDGE": {"provider_id": "m", "model": "m"}}}}
    with pytest.raises(ValueError, match="JUDGE"):
        parse_llm_roles(config)


def test_config_rejects_payload_missing_required_fields() -> None:
    """provider_id + model are required; missing one should surface clearly."""
    config = {"llm": {"roles": {"judge": {"provider_id": "mock"}}}}
    with pytest.raises(TypeError):
        parse_llm_roles(config)
