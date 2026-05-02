"""Backs SHALL clauses in
openspec/changes/provider-settings-and-onboarding/specs/sidecar-runtime/spec.md
  Requirement: Health endpoint (MODIFIED)
    Scenario: Healthy state with all lanes ready
    Scenario: Degraded state with unreachable infrastructure dependency
    Scenario: not-configured lane after fresh install

The new `dependency` field surfaces per-lane readiness keyed by
semantic lane name (`llm_chat` / `llm_embed` / `pii` plus existing
infra dependencies like `qdrant`). Each lane reports one of
`ready` / `not-configured` / `unreachable`.
"""
from __future__ import annotations

import secrets

from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.config.provider_pool import (
    ProviderPoolSnapshot,
    ProviderSpec,
)
from codebus_agent.health import DependencyStatus


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


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


def test_cold_start_lanes_are_not_configured() -> None:
    """Scenario: not-configured lane after fresh install.

    No `POST /internal/startup-config` was made, so
    `app.state.provider_keys` is empty. Both `llm_chat` and `llm_embed`
    lanes MUST report `not-configured`.
    """
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _bound_snapshot()
    # provider_keys remains the empty dict installed by create_app.

    client = TestClient(app)
    resp = client.get("/healthz", headers=_auth(token))
    assert resp.status_code == 200
    body = resp.json()
    assert "dependency" in body
    assert body["dependency"]["llm_chat"] == "not-configured"
    assert body["dependency"]["llm_embed"] == "not-configured"


def test_keys_present_but_smoke_stale_not_configured_lanes_ready() -> None:
    """`phase7-onboarding-polish` regression fix: D-033 B switched API
    keys from env-var to keyring + startup-config, but the boot-time
    smoke probe registration in `create_app` still reads the old env
    var. When `openai_api_key` is None at boot (the new default in
    D-033 B), the registered probe always returns
    `status='not-configured'`. The lane resolver MUST treat that as a
    stale signal and trust the `app.state.provider_keys` presence
    check above it — otherwise no amount of `POST /internal/startup-config`
    can ever flip the lane to `ready` and the user is stuck in the
    onboarding redirect loop forever.
    """
    token = _bearer()
    app = create_app(bearer_token=token)  # no openai_api_key kwarg
    app.state.provider_pool_snapshot = _bound_snapshot()
    app.state.provider_keys = {
        "openai-default": "sk-test-A",
        "openai-embed-3": "sk-test-B",
    }
    # IMPORTANT: do NOT inject smoke probes here — the whole point of
    # this test is that the production boot-time `_probe_openai_chat_not_configured`
    # MUST NOT block the lane when keys are present.

    client = TestClient(app)
    body = client.get("/healthz", headers=_auth(token)).json()
    assert body["dependency"]["llm_chat"] == "ready"
    assert body["dependency"]["llm_embed"] == "ready"


def test_pii_rule_mode_is_ready() -> None:
    """`pii.mode == "rule"` MUST always report `ready` (no LLM key needed)."""
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _bound_snapshot()
    client = TestClient(app)
    body = client.get("/healthz", headers=_auth(token)).json()
    assert body["dependency"]["pii"] == "ready"


def test_keys_present_smoke_pass_lanes_ready() -> None:
    """Scenario: Healthy state with all lanes ready.

    Keys present + smoke probes pass → all three lanes are `ready` and
    the top-level status is `ok`.
    """
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _bound_snapshot()
    app.state.provider_keys = {
        "openai-default": "sk-test-A",
        "openai-embed-3": "sk-test-B",
    }
    # Inject smoke checks that pass — production probes hit OpenAI which
    # would either be flaky or leak the test API key. Tests inject
    # in-memory checks via app.state.dependency_checks (existing hook).
    app.state.dependency_checks["openai_chat"] = lambda: _ok_status()
    app.state.dependency_checks["openai_embedding"] = lambda: _ok_status()

    client = TestClient(app)
    body = client.get("/healthz", headers=_auth(token)).json()
    assert body["status"] == "ok"
    assert body["dependency"]["llm_chat"] == "ready"
    assert body["dependency"]["llm_embed"] == "ready"
    assert body["dependency"]["pii"] == "ready"


def test_keys_present_smoke_fail_lane_unreachable() -> None:
    """Smoke check failure surfaces lane status `unreachable`."""
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _bound_snapshot()
    app.state.provider_keys = {
        "openai-default": "sk-test-A",
        "openai-embed-3": "sk-test-B",
    }
    app.state.dependency_checks["openai_chat"] = lambda: _ok_status()
    app.state.dependency_checks["openai_embedding"] = lambda: _fail_status(
        "ConnectionRefused"
    )

    client = TestClient(app)
    body = client.get("/healthz", headers=_auth(token)).json()
    assert body["dependency"]["llm_embed"] == "unreachable"
    assert body["dependency"]["llm_chat"] == "ready"


def test_existing_dependency_field_for_qdrant_passthrough() -> None:
    """Scenario: Degraded state with unreachable infrastructure dependency.

    Qdrant lane is reported alongside the LLM lanes — existing infra
    keys keep flowing through `dependency`.
    """
    token = _bearer()
    app = create_app(bearer_token=token)
    app.state.provider_pool_snapshot = _bound_snapshot()
    app.state.dependency_checks["qdrant"] = lambda: _fail_status("timeout")

    client = TestClient(app)
    body = client.get("/healthz", headers=_auth(token)).json()
    assert body["status"] == "degraded"
    assert body["dependency"]["qdrant"] == "unreachable"


# --- helpers ---------------------------------------------------------


async def _ok_status() -> DependencyStatus:
    return DependencyStatus(ok=True, status="ok")


async def _fail_status(detail: str) -> DependencyStatus:
    return DependencyStatus(ok=False, status="degraded", detail=detail)
