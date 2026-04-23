"""TDD red tests for `wire_kb_dependencies` — Section 4 of
openspec/changes/kb-build-production-wiring/tasks.md.

Backs openspec/changes/kb-build-production-wiring/specs/sidecar-runtime/spec.md
  Requirement: KB dependency injection hook

Strategy:
  * Exercise `create_app` (which internally calls `wire_kb_dependencies`
    per Section 5 GREEN) with combinations of env vars / urls to verify
    the four `app.state.kb_*` slots end up in the spec-dictated states.
  * `respx` mocks OpenAI calls so the healthz smoke probe doesn't leak
    outbound traffic during tests.
  * The factory-returns-TrackedProvider scenario (from design decision 3
    Option A) dispatches through the slot to confirm the wrapper chain.
"""
from __future__ import annotations

import secrets
from pathlib import Path

import httpx
import pytest
import respx

from codebus_agent.api import create_app
from codebus_agent.providers import (
    OpenAIEmbeddingProvider,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)

_OPENAI_EMBED_URL = "https://api.openai.com/v1/embeddings"


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _fake_embed_response(texts: list[str]) -> dict:
    return {
        "object": "list",
        "data": [
            {"object": "embedding", "index": i, "embedding": [0.0] * 1536}
            for i, _ in enumerate(texts)
        ],
        "model": "text-embedding-3-small",
        "usage": {"prompt_tokens": 1, "total_tokens": 1},
    }


@pytest.fixture(autouse=True)
def _clean_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("CODEBUS_OPENAI_API_KEY", raising=False)
    monkeypatch.delenv("OPENAI_API_KEY", raising=False)


def test_wires_all_four_slots_when_env_present(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Both env vars present wire all four slots"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        app = create_app(
            bearer_token=_bearer(),
            qdrant_url="http://127.0.0.1:6333",
            openai_api_key="sk-test",
        )

    assert app.state.kb_backend is not None
    assert app.state.kb_provider is not None
    assert app.state.kb_usage_tracker is not None
    assert app.state.kb_embedding_dim is not None


def test_missing_openai_key_leaves_provider_none_but_qdrant_wired(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Missing OpenAI API key leaves provider slot as None"."""
    # env fixture already unset CODEBUS_OPENAI_API_KEY.
    app = create_app(
        bearer_token=_bearer(),
        qdrant_url="http://127.0.0.1:6333",
        openai_api_key=None,
    )

    # Degraded mode: provider + dim are None, backend/tracker slots also
    # None because factories construct TrackedProvider (which needs the
    # provider). Spec guarantees only `kb_provider` + `kb_embedding_dim`
    # explicitly, but the practical contract is "either everything or
    # nothing for the openai-dependent slots".
    assert app.state.kb_provider is None
    assert app.state.kb_embedding_dim is None
    # Qdrant still wired independently.
    assert app.state.qdrant_client is not None


def test_usage_tracker_slot_is_factory_not_instance(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "UsageTracker slot is a factory, not a prebuilt instance"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        app = create_app(
            bearer_token=_bearer(),
            qdrant_url="http://127.0.0.1:6333",
            openai_api_key="sk-test",
        )

    factory = app.state.kb_usage_tracker
    assert callable(factory), "kb_usage_tracker MUST be a factory, got instance"
    ws = tmp_path / "ws-a"
    ws.mkdir()
    tracker = factory(ws)
    assert isinstance(tracker, UsageTracker)
    # Path MUST land under the workspace.
    assert str(ws) in str(tracker.path)


def test_kb_provider_slot_is_factory_returning_tracked_provider(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Provider slot is also a factory returning a TrackedProvider"."""
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        app = create_app(
            bearer_token=_bearer(),
            qdrant_url="http://127.0.0.1:6333",
            openai_api_key="sk-test",
        )

    factory = app.state.kb_provider
    assert callable(factory), "kb_provider MUST be a factory, got instance"
    ws = tmp_path / "ws-b"
    ws.mkdir()
    provider = factory(ws)
    assert isinstance(provider, TrackedProvider)
    assert provider.role == ProviderRole.EMBED
    # Inner provider is OpenAI, not Mock.
    assert isinstance(provider._inner, OpenAIEmbeddingProvider)


def test_healthz_reports_openai_embedding_dependency_states(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec scenario "Healthz reflects OpenAI embedding configuration state".

    Verifies three discrete states:
      * not-configured — no env var set
      * ok — env var set + smoke embed returns 200
      * degraded — env var set + smoke embed fails
    """
    from fastapi.testclient import TestClient

    # not-configured
    app = create_app(bearer_token=_bearer(), openai_api_key=None)
    client = TestClient(app)
    resp = client.get(
        "/healthz", headers={"Authorization": f"Bearer {app.state.bearer_token}"}
    )
    body = resp.json()
    assert body["dependencies"]["openai_embedding"]["status"] == "not-configured", (
        f"not-configured state missing: {body}"
    )

    # ok
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        app_ok = create_app(bearer_token=_bearer(), openai_api_key="sk-test")
    client_ok = TestClient(app_ok)
    resp_ok = client_ok.get(
        "/healthz",
        headers={"Authorization": f"Bearer {app_ok.state.bearer_token}"},
    )
    body_ok = resp_ok.json()
    assert body_ok["dependencies"]["openai_embedding"]["status"] == "ok", (
        f"ok state missing: {body_ok}"
    )

    # degraded
    with respx.mock(assert_all_called=False) as mock2:
        mock2.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(401, json={"error": {"message": "bad key"}})
        )
        app_deg = create_app(bearer_token=_bearer(), openai_api_key="sk-bad")
    client_deg = TestClient(app_deg)
    resp_deg = client_deg.get(
        "/healthz",
        headers={"Authorization": f"Bearer {app_deg.state.bearer_token}"},
    )
    body_deg = resp_deg.json()
    assert body_deg["dependencies"]["openai_embedding"]["status"] == "degraded", (
        f"degraded state missing: {body_deg}"
    )
