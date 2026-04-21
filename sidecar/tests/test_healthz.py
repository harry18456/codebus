"""Health endpoint tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Health endpoint
and openspec/changes/qdrant-lifecycle-bootstrap/specs/qdrant-client/spec.md
  Requirement: Runtime health endpoint reflects Qdrant connectivity
"""
from __future__ import annotations

import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.health import DependencyStatus
from codebus_agent.kb import qdrant_client as _kb_qdrant


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def test_healthz_ok_when_all_dependencies_reachable(bearer: str) -> None:
    """Scenario: Healthy state."""
    async def qdrant_ok() -> DependencyStatus:
        return DependencyStatus(ok=True)

    app = create_app(bearer_token=bearer, dependency_checks={"qdrant": qdrant_ok})
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))

    assert response.status_code == 200
    body = response.json()
    assert body["status"] == "ok"


def test_healthz_degraded_when_qdrant_unreachable(bearer: str) -> None:
    """Scenario: Degraded state.

    When Qdrant is unreachable the response MUST still be HTTP 200
    (the sidecar itself is alive) but MUST name the failing dependency
    under ``dependencies``.
    """
    async def qdrant_down() -> DependencyStatus:
        return DependencyStatus(ok=False, detail="connection refused")

    app = create_app(bearer_token=bearer, dependency_checks={"qdrant": qdrant_down})
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))

    assert response.status_code == 200
    body = response.json()
    assert body["status"] == "degraded"
    assert "qdrant" in body["dependencies"]
    assert body["dependencies"]["qdrant"]["ok"] is False


def test_healthz_requires_bearer(bearer: str) -> None:
    """/healthz sits behind the same bearer wall as every other endpoint."""
    app = create_app(bearer_token=bearer)
    with TestClient(app) as client:
        response = client.get("/healthz")
    assert response.status_code == 401


def test_healthz_with_no_registered_checks_is_ok(bearer: str) -> None:
    """Audit lens: the 'empty dependency map' case must default to ok,
    not crash.  Under M1 there are zero mandatory external deps yet."""
    app = create_app(bearer_token=bearer)
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))

    assert response.status_code == 200
    assert response.json()["status"] == "ok"


# ---------------------------------------------------------------------------
# Runtime /healthz reflects live Qdrant connectivity
# (qdrant-lifecycle-bootstrap spec, Requirement: Runtime health endpoint ...)
# ---------------------------------------------------------------------------


def test_runtime_healthz_ok_when_qdrant_reachable(
    bearer: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Scenario: Qdrant reachable, healthz reports ok.

    We monkeypatch the shared probe so CI does not need a real Qdrant
    process — the scenario is about wire-up, not Qdrant itself.
    """
    monkeypatch.setattr(
        _kb_qdrant,
        "probe",
        lambda url, timeout_seconds=1.0: DependencyStatus(ok=True, detail=url),
    )
    app = create_app(bearer_token=bearer, qdrant_url="http://127.0.0.1:6333")
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))
    assert response.status_code == 200
    body = response.json()
    assert body["status"] == "ok"
    assert body["dependencies"]["qdrant"]["ok"] is True


def test_runtime_healthz_degraded_when_qdrant_unreachable(bearer: str) -> None:
    """Scenario: Qdrant unreachable, healthz reports degraded.

    Port 1 on loopback is an unprivileged-reserved port; the TCP
    connect fails fast and the real probe surfaces ``ok=false``.
    """
    app = create_app(bearer_token=bearer, qdrant_url="http://127.0.0.1:1")
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))
    assert response.status_code == 200
    body = response.json()
    assert body["status"] == "degraded"
    assert body["dependencies"]["qdrant"]["ok"] is False


def test_runtime_healthz_omits_qdrant_when_url_not_configured(bearer: str) -> None:
    """Scenario: No Qdrant URL configured, healthz omits dependency."""
    app = create_app(bearer_token=bearer)
    with TestClient(app) as client:
        response = client.get("/healthz", headers=_auth(bearer))
    assert response.status_code == 200
    body = response.json()
    assert "qdrant" not in body.get("dependencies", {})
