"""Integration tests for settings mutation → disk persistence.

Backs SHALL clauses in
``openspec/changes/phase7-onboarding-polish/specs/keyring-integration/spec.md``
  Requirement: Provider pool persists to disk across sidecar restarts

Each mutation endpoint (POST / DELETE /settings/providers, PUT
/settings/bindings, PUT /settings/pii-mode) MUST mirror the in-memory
snapshot to ``~/.codebus/llm-config.json`` after applying the
in-memory change. ``create_app`` MUST rehydrate the snapshot from
that file at boot.
"""
from __future__ import annotations

import json
import secrets
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.auth import paths as _paths


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


@pytest.fixture
def redirect_llm_config(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> Path:
    """Point ``llm_config_path()`` at a tmp file so the test never
    touches the developer's real ``~/.codebus/llm-config.json``."""
    fake = tmp_path / "llm-config.json"
    monkeypatch.setattr(_paths, "llm_config_path", lambda: fake)
    return fake


def test_post_provider_writes_to_disk(redirect_llm_config: Path) -> None:
    token = _bearer()
    app = create_app(bearer_token=token)
    client = TestClient(app)

    resp = client.post(
        "/settings/providers",
        json={
            "id": "openai-default",
            "type": "openai_chat",
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
        },
        headers=_auth(token),
    )
    assert resp.status_code == 204

    assert redirect_llm_config.exists()
    payload = json.loads(redirect_llm_config.read_text(encoding="utf-8"))
    assert payload["version"] == 1
    assert payload["providers"] == [
        {
            "id": "openai-default",
            "type": "openai_chat",
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
        }
    ]


def test_put_bindings_writes_to_disk(redirect_llm_config: Path) -> None:
    token = _bearer()
    app = create_app(bearer_token=token)
    client = TestClient(app)

    # Seed pool first so the binding has something to point at.
    client.post(
        "/settings/providers",
        json={
            "id": "openai-default",
            "type": "openai_chat",
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
        },
        headers=_auth(token),
    )

    resp = client.put(
        "/settings/bindings",
        json={"chat": "openai-default"},
        headers=_auth(token),
    )
    assert resp.status_code == 204

    payload = json.loads(redirect_llm_config.read_text(encoding="utf-8"))
    assert payload["bindings"]["chat"] == "openai-default"


def test_create_app_rehydrates_from_existing_file(
    redirect_llm_config: Path,
) -> None:
    """Pre-write a config file then build the app — the in-memory
    snapshot MUST reflect the file's contents (mirrors a real
    sidecar restart for an already-onboarded user)."""
    redirect_llm_config.parent.mkdir(parents=True, exist_ok=True)
    redirect_llm_config.write_text(
        json.dumps(
            {
                "version": 1,
                "providers": [
                    {
                        "id": "openai-default",
                        "type": "openai_chat",
                        "model": "gpt-4o-mini",
                        "base_url": "https://api.openai.com/v1",
                    }
                ],
                "bindings": {"chat": "openai-default"},
                "pii_mode": "rule",
                "pii_provider_id": None,
            }
        ),
        encoding="utf-8",
    )

    token = _bearer()
    app = create_app(bearer_token=token)
    client = TestClient(app)

    resp = client.get("/settings/providers", headers=_auth(token))
    assert resp.status_code == 200
    body = resp.json()
    assert len(body["providers"]) == 1
    assert body["providers"][0]["id"] == "openai-default"
    assert body["bindings"] == {"chat": "openai-default"}


def test_delete_provider_writes_to_disk(redirect_llm_config: Path) -> None:
    token = _bearer()
    app = create_app(bearer_token=token)
    client = TestClient(app)

    client.post(
        "/settings/providers",
        json={
            "id": "openai-default",
            "type": "openai_chat",
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
        },
        headers=_auth(token),
    )
    resp = client.delete(
        "/settings/providers/openai-default", headers=_auth(token)
    )
    assert resp.status_code == 204

    payload = json.loads(redirect_llm_config.read_text(encoding="utf-8"))
    assert payload["providers"] == []


def test_disk_payload_never_contains_api_key(
    redirect_llm_config: Path,
) -> None:
    """Trust boundary: the persisted file MUST NEVER carry an api_key
    field, even if the request body somehow tried to sneak one
    (Pydantic `extra='forbid'` already 422s such bodies before they
    reach the snapshot, but we keep the assertion as defense in
    depth)."""
    token = _bearer()
    app = create_app(bearer_token=token)
    client = TestClient(app)

    client.post(
        "/settings/providers",
        json={
            "id": "openai-default",
            "type": "openai_chat",
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
        },
        headers=_auth(token),
    )

    raw = redirect_llm_config.read_text(encoding="utf-8")
    assert "api_key" not in raw.lower()
    assert "apikey" not in raw.lower()
