"""Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: Config schema supports provider pool with role bindings
    Scenario: New schema accepted
    Scenario: Legacy schema converted with deprecation warning
    Scenario: Binding referencing unknown provider rejected
    Scenario: Embed binding to chat-typed provider rejected

Plus the related Requirement (Sidecar accepts provider config mutation
endpoints) clause around `pii.mode == "llm"` requiring `provider_id`.
"""
from __future__ import annotations

import warnings

import pytest

from codebus_agent.config.provider_pool import (
    INVALID_PII_PROVIDER,
    INVALID_PROVIDER_BINDING,
    INVALID_PROVIDER_TYPE,
    ProviderPoolConfigError,
    load_provider_pool,
)


def test_new_schema_accepted() -> None:
    """Scenario: New schema accepted.

    `[[llm.providers]]` array + `[llm.bindings]` table → in-memory
    snapshot matches the source dict.
    """
    raw = {
        "llm": {
            "providers": [
                {
                    "id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                },
                {
                    "id": "openai-embed-3-small",
                    "type": "openai_embedding",
                    "model": "text-embedding-3-small",
                    "base_url": "https://api.openai.com/v1",
                },
            ],
            "bindings": {
                "reasoning": "openai-default",
                "judge": "openai-default",
                "chat": "openai-default",
                "embed": "openai-embed-3-small",
            },
            "pii": {"mode": "rule"},
        }
    }
    snap = load_provider_pool(raw)

    assert {p.id for p in snap.providers} == {
        "openai-default",
        "openai-embed-3-small",
    }
    assert snap.bindings == {
        "reasoning": "openai-default",
        "judge": "openai-default",
        "chat": "openai-default",
        "embed": "openai-embed-3-small",
    }
    assert snap.pii_mode == "rule"
    assert snap.pii_provider_id is None


def test_legacy_schema_converted_with_one_deprecation_warning() -> None:
    """Scenario: Legacy schema converted with deprecation warning.

    `[llm.roles.<role>]` shape MUST be converted in-memory and emit
    exactly one deprecation warning per process start.
    """
    raw = {
        "llm": {
            "roles": {
                "reasoning": {
                    "provider_id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                },
                "embed": {
                    "provider_id": "openai-embed-3",
                    "type": "openai_embedding",
                    "model": "text-embedding-3-small",
                    "base_url": "https://api.openai.com/v1",
                },
            }
        }
    }
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        snap = load_provider_pool(raw)

    deprecation = [w for w in caught if issubclass(w.category, DeprecationWarning)]
    assert len(deprecation) == 1, (
        f"expected exactly one deprecation warning, got {len(deprecation)}"
    )
    assert {p.id for p in snap.providers} == {"openai-default", "openai-embed-3"}
    assert snap.bindings["reasoning"] == "openai-default"
    assert snap.bindings["embed"] == "openai-embed-3"


def test_binding_unknown_provider_rejected() -> None:
    """Scenario: Binding referencing unknown provider rejected."""
    raw = {
        "llm": {
            "providers": [
                {
                    "id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                }
            ],
            "bindings": {
                "reasoning": "does-not-exist",
            },
        }
    }
    with pytest.raises(ProviderPoolConfigError) as exc_info:
        load_provider_pool(raw)
    assert exc_info.value.code == INVALID_PROVIDER_BINDING
    # Offending role name must surface in the message.
    assert "reasoning" in str(exc_info.value)


def test_embed_binding_to_chat_typed_provider_rejected() -> None:
    """Scenario: Embed binding to chat-typed provider rejected."""
    raw = {
        "llm": {
            "providers": [
                {
                    "id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                }
            ],
            "bindings": {
                "embed": "openai-default",
            },
        }
    }
    with pytest.raises(ProviderPoolConfigError) as exc_info:
        load_provider_pool(raw)
    assert exc_info.value.code == INVALID_PROVIDER_TYPE


def test_pii_mode_llm_without_provider_id_rejected() -> None:
    """`pii.mode == "llm"` MUST require a non-empty `provider_id`."""
    raw = {
        "llm": {
            "providers": [
                {
                    "id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                },
            ],
            "bindings": {
                "reasoning": "openai-default",
            },
            "pii": {"mode": "llm"},
        }
    }
    with pytest.raises(ProviderPoolConfigError) as exc_info:
        load_provider_pool(raw)
    assert exc_info.value.code == INVALID_PII_PROVIDER


def test_pii_mode_llm_with_unknown_provider_rejected() -> None:
    """`pii.mode == "llm"` `provider_id` must reference a real provider."""
    raw = {
        "llm": {
            "providers": [
                {
                    "id": "openai-default",
                    "type": "openai_chat",
                    "model": "gpt-4o-mini",
                    "base_url": "https://api.openai.com/v1",
                },
            ],
            "bindings": {
                "reasoning": "openai-default",
            },
            "pii": {"mode": "llm", "provider_id": "no-such-pii"},
        }
    }
    with pytest.raises(ProviderPoolConfigError) as exc_info:
        load_provider_pool(raw)
    assert exc_info.value.code == INVALID_PII_PROVIDER
