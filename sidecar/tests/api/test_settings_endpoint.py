"""Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: Sidecar accepts provider config mutation endpoints
    Scenario: Settings provider GET excludes API keys
    Scenario: Settings binding PUT triggers RegistryHolder swap
    Scenario: PII mode llm without provider_id rejected

  Requirement: provider_config_changed SSE event surface
    Scenario: Binding change emits event with role list
    Scenario: Embed change sets embed_changed flag
    Scenario: Event carries no secrets

These endpoints (`GET/POST/DELETE /settings/providers`,
`PUT /settings/bindings`, `PUT /settings/pii-mode`) plus the
`GET /events?channel=app` SSE channel are bearer-guarded but kept
out of `/openapi.json` (`include_in_schema=False`).
"""
from __future__ import annotations

import asyncio
import json
import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.config.provider_pool import (
    ProviderPoolSnapshot,
    ProviderSpec,
)


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def _seed_snapshot() -> ProviderPoolSnapshot:
    """Two-provider snapshot used by the endpoint tests below."""
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


def _make_client(seed: ProviderPoolSnapshot | None = None) -> tuple[TestClient, str]:
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = seed or _seed_snapshot()
    return TestClient(app), token


def test_get_providers_excludes_api_keys() -> None:
    """Scenario: Settings provider GET excludes API keys."""
    client, token = _make_client()
    resp = client.get("/settings/providers", headers=_auth(token))

    assert resp.status_code == 200
    body = resp.json()
    assert "providers" in body
    assert "bindings" in body
    assert "pii_mode" in body

    for entry in body["providers"]:
        assert "api_key" not in entry, f"api_key leaked into GET response: {entry}"

    assert body["pii_mode"] == "rule"


def test_post_provider_upserts_into_pool() -> None:
    """`POST /settings/providers` adds an entry to the in-memory pool."""
    client, token = _make_client()
    new_provider = {
        "id": "anthropic-claude",
        "type": "openai_chat",
        "model": "claude-haiku",
        "base_url": "https://api.anthropic.com/v1",
    }
    resp = client.post(
        "/settings/providers", json=new_provider, headers=_auth(token)
    )
    assert resp.status_code in (200, 201, 204)

    get_resp = client.get("/settings/providers", headers=_auth(token))
    ids = {p["id"] for p in get_resp.json()["providers"]}
    assert "anthropic-claude" in ids


def test_post_provider_rejects_api_key_field() -> None:
    """`POST /settings/providers` MUST reject an `api_key` field in body."""
    client, token = _make_client()
    body_with_key = {
        "id": "rogue",
        "type": "openai_chat",
        "model": "gpt-4o-mini",
        "base_url": "https://api.openai.com/v1",
        "api_key": "sk-leak-attempt",
    }
    resp = client.post(
        "/settings/providers", json=body_with_key, headers=_auth(token)
    )
    # FastAPI maps unknown fields per Pydantic config — the model must be
    # configured to forbid extra fields so this returns 422.
    assert resp.status_code == 422


def test_delete_provider_blocked_when_bound() -> None:
    """`DELETE /settings/providers/{id}` MUST 409 when id is in bindings."""
    client, token = _make_client()
    resp = client.delete(
        "/settings/providers/openai-default", headers=_auth(token)
    )
    assert resp.status_code == 409
    body = resp.json()
    assert body["detail"]["code"] == "PROVIDER_BOUND_TO_ROLE"
    # The bound role names must surface so UI can prompt user to unbind first.
    assert "roles" in body["detail"]
    assert set(body["detail"]["roles"]) >= {"reasoning", "judge", "chat"}


def test_put_bindings_swaps_registry_holder() -> None:
    """Scenario: Settings binding PUT triggers RegistryHolder swap.

    The endpoint must call `RegistryHolder.swap(new_registry)`. We
    detect the swap by installing a recording holder on app.state
    before the request and asserting `swap` was invoked.
    """
    from codebus_agent.providers import (
        LLMCallLogger,
        MockProvider,
        ProviderRegistry,
        ProviderRole,
        RegistryHolder,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

    client, token = _make_client()

    def _wrap(name: str, role: ProviderRole) -> TrackedProvider:
        return TrackedProvider(
            MockProvider(),
            tracker=UsageTracker(client.app.state.bearer_token + f".{name}.tok"),
            logger=LLMCallLogger(client.app.state.bearer_token + f".{name}.log"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(
                client.app.state.bearer_token + f".{name}.san"
            ),
            rules_version="test-v1",
        )

    initial = ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap("r", ProviderRole.REASONING),
            ProviderRole.JUDGE: _wrap("j", ProviderRole.JUDGE),
            ProviderRole.CHAT: _wrap("c", ProviderRole.CHAT),
            ProviderRole.EMBED: _wrap("e", ProviderRole.EMBED),
        }
    )

    holder = RegistryHolder(initial)
    swap_calls: list[ProviderRegistry] = []
    original_swap = holder.swap

    async def _recording_swap(new_registry: ProviderRegistry) -> None:
        swap_calls.append(new_registry)
        await original_swap(new_registry)

    holder.swap = _recording_swap  # type: ignore[method-assign]
    client.app.state.providers = holder
    # A registry factory the endpoint uses to build a fresh registry from
    # the updated snapshot. Tests pass a stub that returns a fresh
    # MockProvider-backed registry so swap() can be invoked safely.
    client.app.state.registry_factory = lambda snapshot, keys: ProviderRegistry(
        {
            ProviderRole.REASONING: _wrap("r2", ProviderRole.REASONING),
            ProviderRole.JUDGE: _wrap("j2", ProviderRole.JUDGE),
            ProviderRole.CHAT: _wrap("c2", ProviderRole.CHAT),
            ProviderRole.EMBED: _wrap("e2", ProviderRole.EMBED),
        }
    )

    new_bindings = {
        "reasoning": "openai-default",
        "judge": "openai-default",
        "chat": "openai-default",
        "embed": "openai-embed-3",
    }
    resp = client.put(
        "/settings/bindings", json=new_bindings, headers=_auth(token)
    )

    assert resp.status_code in (200, 204)
    assert len(swap_calls) == 1
    assert isinstance(swap_calls[0], ProviderRegistry)


def test_put_pii_mode_rejects_llm_without_provider_id() -> None:
    """Scenario: PII mode llm without provider_id rejected."""
    client, token = _make_client()
    resp = client.put(
        "/settings/pii-mode", json={"mode": "llm"}, headers=_auth(token)
    )
    assert resp.status_code == 400
    body = resp.json()
    assert body["detail"]["code"] == "INVALID_PII_PROVIDER"


def test_settings_endpoints_excluded_from_openapi() -> None:
    """All `/settings/*` and `/events` endpoints MUST set `include_in_schema=False`."""
    client, _ = _make_client()
    schema = client.get("/openapi.json").json()
    paths = set(schema.get("paths", {}).keys())
    for hidden in (
        "/settings/providers",
        "/settings/bindings",
        "/settings/pii-mode",
        "/events",
    ):
        assert hidden not in paths, (
            f"{hidden} leaked into the public OpenAPI surface"
        )


@pytest.mark.asyncio
async def test_provider_config_changed_event_emitted_with_changed_roles() -> None:
    """Scenario: Binding change emits event with role list.

    `PUT /settings/bindings` changes two roles → app channel observes
    exactly one `provider_config_changed` event whose `data.changed_roles`
    is the union (order-insensitive).
    """
    from httpx import ASGITransport, AsyncClient

    from codebus_agent.providers import (
        LLMCallLogger,
        MockProvider,
        ProviderRegistry,
        ProviderRole,
        RegistryHolder,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _seed_snapshot()

    def _wrap(name: str, role: ProviderRole) -> TrackedProvider:
        from pathlib import Path
        tmp = Path(".") / "_test_settings_sse_tmp"
        tmp.mkdir(exist_ok=True)
        return TrackedProvider(
            MockProvider(),
            tracker=UsageTracker(tmp / f"{name}.tok"),
            logger=LLMCallLogger(tmp / f"{name}.log"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(tmp / f"{name}.san"),
            rules_version="test-v1",
        )

    initial = ProviderRegistry(
        {role: _wrap(role.value, role) for role in ProviderRole}
    )
    app.state.providers = RegistryHolder(initial)
    app.state.registry_factory = lambda snapshot, keys: ProviderRegistry(
        {role: _wrap(f"new-{role.value}", role) for role in ProviderRole}
    )

    broker = app.state.app_event_broker
    queue = broker.subscribe()

    # Switch reasoning to a different provider id in addition to chat —
    # the snapshot already has chat + reasoning bound to openai-default,
    # so we need a third provider to flip them onto.
    app.state.provider_pool_snapshot = ProviderPoolSnapshot(
        providers=(
            *app.state.provider_pool_snapshot.providers,
            ProviderSpec(
                id="anthropic-claude",
                type="openai_chat",
                model="claude-haiku",
                base_url="https://api.anthropic.com/v1",
            ),
        ),
        bindings=dict(app.state.provider_pool_snapshot.bindings),
        pii_mode="rule",
        pii_provider_id=None,
    )

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://t") as client:
        resp = await client.put(
            "/settings/bindings",
            json={
                "reasoning": "anthropic-claude",
                "judge": "openai-default",
                "chat": "anthropic-claude",
                "embed": "openai-embed-3",
            },
            headers=_auth(token),
        )
        assert resp.status_code in (200, 204)

    # Coalescing window is 50ms — give the broker some leeway.
    event = await asyncio.wait_for(queue.get(), timeout=2.0)
    assert event["type"] == "provider_config_changed"
    assert set(event["data"]["changed_roles"]) == {"reasoning", "chat"}
    assert event["data"]["embed_changed"] is False


@pytest.mark.asyncio
async def test_provider_config_changed_event_marks_embed_change() -> None:
    """Scenario: Embed change sets embed_changed flag."""
    from codebus_agent.providers import (
        LLMCallLogger,
        MockProvider,
        ProviderRegistry,
        ProviderRole,
        RegistryHolder,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

    token = _bearer()
    app = create_app(bearer_token=token)

    def _wrap(name: str, role: ProviderRole) -> TrackedProvider:
        from pathlib import Path
        tmp = Path(".") / "_test_settings_sse_embed_tmp"
        tmp.mkdir(exist_ok=True)
        return TrackedProvider(
            MockProvider(),
            tracker=UsageTracker(tmp / f"{name}.tok"),
            logger=LLMCallLogger(tmp / f"{name}.log"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(tmp / f"{name}.san"),
            rules_version="test-v1",
        )

    snapshot = ProviderPoolSnapshot(
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
            ProviderSpec(
                id="openai-embed-large",
                type="openai_embedding",
                model="text-embedding-3-large",
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
    app.state.provider_pool_snapshot = snapshot

    initial = ProviderRegistry(
        {role: _wrap(role.value, role) for role in ProviderRole}
    )
    app.state.providers = RegistryHolder(initial)
    app.state.registry_factory = lambda s, k: ProviderRegistry(
        {role: _wrap(f"n-{role.value}", role) for role in ProviderRole}
    )

    broker = app.state.app_event_broker
    queue = broker.subscribe()

    from httpx import ASGITransport, AsyncClient

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://t") as client:
        resp = await client.put(
            "/settings/bindings",
            json={
                "reasoning": "openai-default",
                "judge": "openai-default",
                "chat": "openai-default",
                "embed": "openai-embed-large",
            },
            headers=_auth(token),
        )
        assert resp.status_code in (200, 204)

    event = await asyncio.wait_for(queue.get(), timeout=2.0)
    assert event["type"] == "provider_config_changed"
    assert event["data"]["embed_changed"] is True
    assert "embed" in event["data"]["changed_roles"]


@pytest.mark.asyncio
async def test_provider_config_changed_event_carries_no_secrets() -> None:
    """Scenario: Event carries no secrets.

    The emitted event payload MUST NOT contain any api_key value or any
    `~/.codebus/` filesystem path.
    """
    from codebus_agent.providers import (
        LLMCallLogger,
        MockProvider,
        ProviderRegistry,
        ProviderRole,
        RegistryHolder,
        TrackedProvider,
        UsageTracker,
    )
    from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _seed_snapshot()

    sentinel_key = "sk-sentinel-DO-NOT-LEAK-1234567890"
    app.state.provider_keys = {
        "openai-default": sentinel_key,
        "openai-embed-3": sentinel_key,
    }

    def _wrap(name: str, role: ProviderRole) -> TrackedProvider:
        from pathlib import Path
        tmp = Path(".") / "_test_settings_sse_secret_tmp"
        tmp.mkdir(exist_ok=True)
        return TrackedProvider(
            MockProvider(),
            tracker=UsageTracker(tmp / f"{name}.tok"),
            logger=LLMCallLogger(tmp / f"{name}.log"),
            role=role,
            sanitizer=SanitizerEngine(),
            sanitizer_audit=SanitizerAuditLogger(tmp / f"{name}.san"),
            rules_version="test-v1",
        )

    initial = ProviderRegistry(
        {role: _wrap(role.value, role) for role in ProviderRole}
    )
    app.state.providers = RegistryHolder(initial)
    app.state.registry_factory = lambda s, k: ProviderRegistry(
        {role: _wrap(f"n-{role.value}", role) for role in ProviderRole}
    )

    broker = app.state.app_event_broker
    queue = broker.subscribe()

    from httpx import ASGITransport, AsyncClient

    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://t") as client:
        await client.put(
            "/settings/bindings",
            json={
                "reasoning": "openai-default",
                "judge": "openai-default",
                "chat": "openai-default",
                "embed": "openai-embed-3",
            },
            headers=_auth(token),
        )

    # Drain whatever the broker emitted.
    events: list[dict] = []
    while True:
        try:
            ev = await asyncio.wait_for(queue.get(), timeout=0.5)
            events.append(ev)
        except asyncio.TimeoutError:
            break

    serialized = json.dumps(events)
    assert sentinel_key not in serialized, "api_key value leaked into SSE event"
    assert "/.codebus/" not in serialized, (
        "filesystem audit path leaked into SSE event"
    )
