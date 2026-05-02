"""Tests for the LLM provider pool persistence layer.

Backs SHALL clauses in
``openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md``
  Requirement: Provider pool persists to disk across sidecar restarts
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.config.llm_config_store import (
    LLM_CONFIG_SCHEMA_VERSION,
    load_llm_config_or_default,
    save_llm_config,
)
from codebus_agent.config.provider_pool import (
    ProviderPoolSnapshot,
    ProviderSpec,
)


def _bound_snapshot() -> ProviderPoolSnapshot:
    return ProviderPoolSnapshot(
        providers=(
            ProviderSpec(
                id="openai-default",
                type="openai_chat",
                model="gpt-4o-mini",
                base_url="https://api.openai.com/v1",
            ),
            ProviderSpec(
                id="openai-embed-3",
                type="openai_embedding",
                model="text-embedding-3-small",
                base_url="https://api.openai.com/v1",
            ),
        ),
        bindings={
            "reasoning": "openai-default",
            "judge": "openai-default",
            "chat": "openai-default",
            "embed": "openai-embed-3",
        },
        pii_mode="rule",
        pii_provider_id=None,
    )


def test_save_then_load_round_trip(tmp_path: Path) -> None:
    """Save → load returns the same snapshot."""
    target = tmp_path / "llm-config.json"
    snapshot = _bound_snapshot()

    save_llm_config(snapshot, path=target)
    loaded = load_llm_config_or_default(path=target)

    assert loaded.providers == snapshot.providers
    assert loaded.bindings == snapshot.bindings
    assert loaded.pii_mode == snapshot.pii_mode
    assert loaded.pii_provider_id == snapshot.pii_provider_id


def test_load_returns_empty_when_file_missing(tmp_path: Path) -> None:
    """Fresh install → no file → empty default + no exception."""
    target = tmp_path / "does-not-exist.json"
    loaded = load_llm_config_or_default(path=target)
    assert loaded.providers == ()
    assert loaded.bindings == {}
    assert loaded.pii_mode == "rule"
    assert loaded.pii_provider_id is None


def test_load_returns_empty_on_corrupt_json(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """Corrupt JSON → empty default + warn log; MUST NOT raise."""
    target = tmp_path / "llm-config.json"
    target.write_text("{not valid json", encoding="utf-8")

    with caplog.at_level("WARNING"):
        loaded = load_llm_config_or_default(path=target)

    assert loaded.providers == ()
    assert any("read failed" in rec.message for rec in caplog.records)


def test_save_atomic_uses_tmp_then_replace(tmp_path: Path) -> None:
    """Save process MUST NOT leave a partial state. The implementation
    writes to ``<path>.tmp`` then ``os.replace()``s into place. After
    a successful save the .tmp sibling MUST NOT exist (replace consumes
    it) and the target file content MUST be valid JSON."""
    target = tmp_path / "llm-config.json"
    save_llm_config(_bound_snapshot(), path=target)

    assert target.exists()
    assert not target.with_suffix(target.suffix + ".tmp").exists()
    payload = json.loads(target.read_text(encoding="utf-8"))
    assert payload["version"] == LLM_CONFIG_SCHEMA_VERSION


def test_save_does_not_write_api_key(tmp_path: Path) -> None:
    """Trust boundary invariant: serialized payload MUST NOT contain
    any field named 'api_key' / 'apiKey' / 'key' (the snapshot itself
    has no such field, but a future regression that adds one MUST be
    caught here so it cannot land silently)."""
    target = tmp_path / "llm-config.json"
    save_llm_config(_bound_snapshot(), path=target)

    raw = target.read_text(encoding="utf-8")
    assert "api_key" not in raw.lower()
    assert "apikey" not in raw.lower()
    # The string "key" alone is too generic (e.g. JSON keys are called
    # "keys"); we assert specifically the field shapes a serializer
    # might emit.


def test_load_recovers_from_schema_violation(
    tmp_path: Path, caplog: pytest.LogCaptureFixture
) -> None:
    """File present but bindings reference an unknown provider id →
    empty default + warn (never raise)."""
    target = tmp_path / "llm-config.json"
    target.write_text(
        json.dumps(
            {
                "version": 1,
                "providers": [],
                "bindings": {"chat": "ghost-id"},
                "pii_mode": "rule",
                "pii_provider_id": None,
            }
        ),
        encoding="utf-8",
    )

    with caplog.at_level("WARNING"):
        loaded = load_llm_config_or_default(path=target)

    assert loaded.providers == ()
    assert loaded.bindings == {}
    assert any("schema validation failed" in rec.message for rec in caplog.records)
