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
    OpenAIChatProvider,
    OpenAIEmbeddingProvider,
    ProviderRole,
    TrackedProvider,
    UsageTracker,
)

_OPENAI_EMBED_URL = "https://api.openai.com/v1/embeddings"
_OPENAI_CHAT_URL = "https://api.openai.com/v1/chat/completions"


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


def _fake_chat_response(
    payload: dict | None = None, *, tool_name: str = "_ChatProbeModel"
) -> dict:
    """OpenAI chat completions in Instructor TOOLS mode.

    Used by the chat smoke probe — `chat-provider-wiring` wires a startup
    probe that calls `gpt-4o-mini` with `response_model=_ChatProbeModel`.
    The default `tool_name` matches the probe's Pydantic model class name
    because Instructor TOOLS mode routes tool_call responses to the
    model whose name matches the `tool_calls[].function.name` field.
    """
    import json as _json

    return {
        "id": "chatcmpl-test",
        "object": "chat.completion",
        "created": 1_700_000_000,
        "model": "gpt-4o-mini",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": None,
                    "tool_calls": [
                        {
                            "id": "call_abc",
                            "type": "function",
                            "function": {
                                "name": tool_name,
                                "arguments": _json.dumps(payload or {"ok": True}),
                            },
                        }
                    ],
                },
                "finish_reason": "tool_calls",
            }
        ],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 1,
            "total_tokens": 2,
        },
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


def test_query_provider_factory_uses_kb_query_module(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `KB query endpoint registration` Scenario "Both KB build and KB
    query slots present after wiring":

    `app.state.kb_query_provider` is a separate factory from
    `app.state.kb_provider`. Both invoked with the same workspace return
    distinct TrackedProviders with `_default_module` "kb_build" vs "kb_query"
    respectively, so cost accounting can split build/query embedding spend.
    """
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

    build_factory = app.state.kb_provider
    query_factory = app.state.kb_query_provider
    assert callable(build_factory), "kb_provider must be a factory"
    assert callable(query_factory), "kb_query_provider must be a factory"

    ws = tmp_path / "ws-q"
    ws.mkdir()
    build_provider = build_factory(ws)
    query_provider = query_factory(ws)
    assert isinstance(build_provider, TrackedProvider)
    assert isinstance(query_provider, TrackedProvider)
    assert build_provider is not query_provider, (
        "build / query providers MUST be distinct instances so audit logs do not "
        "share state"
    )
    assert build_provider._default_module == "kb_build"
    assert query_provider._default_module == "kb_query"


def test_missing_openai_key_leaves_query_provider_none(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `KB query endpoint registration` Scenario "Missing OpenAI API key
    leaves both provider slots None"."""
    # env fixture already unset CODEBUS_OPENAI_API_KEY.
    app = create_app(
        bearer_token=_bearer(),
        qdrant_url="http://127.0.0.1:6333",
        openai_api_key=None,
    )
    assert app.state.kb_provider is None
    assert app.state.kb_query_provider is None


def test_wires_all_eight_slots_when_env_present(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `chat-provider-wiring / KB dependency injection hook` Scenario
    "Both env vars present wire all eight slots".

    Extends the original four-slot invariant to the three chat-ish slots
    (`llm_reasoning_provider` / `llm_judge_provider` / `llm_chat_provider`)
    plus `kb_query_provider` so cost-split audit and Module 4 Explorer
    wiring work out of the box.
    """
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(200, json=_fake_chat_response())
        )
        app = create_app(
            bearer_token=_bearer(),
            qdrant_url="http://127.0.0.1:6333",
            openai_api_key="sk-test",
        )

    assert app.state.kb_backend is not None
    assert app.state.kb_provider is not None
    assert app.state.kb_query_provider is not None
    assert app.state.kb_usage_tracker is not None
    assert app.state.kb_embedding_dim is not None
    assert app.state.llm_reasoning_provider is not None
    assert app.state.llm_judge_provider is not None
    assert app.state.llm_chat_provider is not None


def test_missing_openai_key_leaves_chat_slots_none(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `chat-provider-wiring / KB dependency injection hook` Scenario
    "Missing OpenAI API key leaves provider slot as None".

    All OpenAI-dependent slots (both embed-family and chat-family) MUST
    be `None` so the sidecar starts in graceful degraded mode without
    constructing a broken provider.
    """
    # env fixture already unset CODEBUS_OPENAI_API_KEY.
    app = create_app(
        bearer_token=_bearer(),
        qdrant_url="http://127.0.0.1:6333",
        openai_api_key=None,
    )

    assert app.state.kb_provider is None
    assert app.state.kb_query_provider is None
    assert app.state.kb_embedding_dim is None
    assert app.state.llm_reasoning_provider is None
    assert app.state.llm_judge_provider is None
    assert app.state.llm_chat_provider is None


def test_chat_slots_are_factories_returning_tracked_providers(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `chat-provider-wiring / KB dependency injection hook` Scenario
    "Chat-ish provider slots are factories returning TrackedProviders
    with role-appropriate default_module".

    Three slots must each produce a TrackedProvider with its own
    `default_module` tag and matching `role`, so `token_usage.jsonl`
    can split cost accounting between `reasoning` / `judge` / `chat`.
    """
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(200, json=_fake_chat_response())
        )
        app = create_app(
            bearer_token=_bearer(),
            qdrant_url="http://127.0.0.1:6333",
            openai_api_key="sk-test",
        )

    ws = tmp_path / "ws-chat"
    ws.mkdir()

    cases = [
        ("llm_reasoning_provider", "reasoning", ProviderRole.REASONING),
        ("llm_judge_provider", "judge", ProviderRole.JUDGE),
        ("llm_chat_provider", "chat", ProviderRole.CHAT),
    ]
    seen_instances: list[TrackedProvider] = []
    for slot_name, expected_module, expected_role in cases:
        factory = getattr(app.state, slot_name)
        assert callable(factory), f"{slot_name} MUST be a factory"
        provider = factory(ws)
        assert isinstance(provider, TrackedProvider), (
            f"{slot_name} MUST return a TrackedProvider"
        )
        assert isinstance(provider._inner, OpenAIChatProvider), (
            f"{slot_name} inner provider MUST be OpenAIChatProvider"
        )
        assert provider.role == expected_role, (
            f"{slot_name} role MUST be {expected_role}; got {provider.role}"
        )
        assert provider._default_module == expected_module, (
            f"{slot_name} default_module MUST be {expected_module!r}; "
            f"got {provider._default_module!r}"
        )
        # Fresh instance per call — no shared state across workspaces.
        provider_2 = factory(ws)
        assert provider is not provider_2, (
            f"{slot_name} factory MUST return a fresh TrackedProvider each call"
        )
        seen_instances.extend([provider, provider_2])

    # All six providers are distinct — no silent aliasing across slots.
    assert len({id(p) for p in seen_instances}) == len(seen_instances), (
        "factory invocations across slots MUST yield distinct instances"
    )


def test_healthz_reports_openai_chat_dependency_states(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Spec `chat-provider-wiring / KB dependency injection hook` Scenario
    "Healthz reflects OpenAI chat configuration state".

    Verifies three discrete states mirroring the embedding probe:
      * not-configured — no env var set
      * ok — env var set + smoke chat call returns 200
      * degraded — env var set + smoke chat call fails (e.g., 401)
    """
    from fastapi.testclient import TestClient

    # not-configured
    app = create_app(bearer_token=_bearer(), openai_api_key=None)
    client = TestClient(app)
    resp = client.get(
        "/healthz", headers={"Authorization": f"Bearer {app.state.bearer_token}"}
    )
    body = resp.json()
    assert body["dependencies"]["openai_chat"]["status"] == "not-configured", (
        f"not-configured state missing: {body}"
    )

    # ok
    monkeypatch.setenv("CODEBUS_OPENAI_API_KEY", "sk-test")
    with respx.mock(assert_all_called=False) as mock:
        mock.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        mock.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(200, json=_fake_chat_response())
        )
        app_ok = create_app(bearer_token=_bearer(), openai_api_key="sk-test")
    client_ok = TestClient(app_ok)
    resp_ok = client_ok.get(
        "/healthz",
        headers={"Authorization": f"Bearer {app_ok.state.bearer_token}"},
    )
    body_ok = resp_ok.json()
    assert body_ok["dependencies"]["openai_chat"]["status"] == "ok", (
        f"ok state missing: {body_ok}"
    )

    # degraded
    with respx.mock(assert_all_called=False) as mock2:
        mock2.post(_OPENAI_EMBED_URL).mock(
            return_value=httpx.Response(200, json=_fake_embed_response(["ping"]))
        )
        mock2.post(_OPENAI_CHAT_URL).mock(
            return_value=httpx.Response(
                401, json={"error": {"message": "bad key"}}
            )
        )
        app_deg = create_app(bearer_token=_bearer(), openai_api_key="sk-bad")
    client_deg = TestClient(app_deg)
    resp_deg = client_deg.get(
        "/healthz",
        headers={"Authorization": f"Bearer {app_deg.state.bearer_token}"},
    )
    body_deg = resp_deg.json()
    assert body_deg["dependencies"]["openai_chat"]["status"] == "degraded", (
        f"degraded state missing: {body_deg}"
    )


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
