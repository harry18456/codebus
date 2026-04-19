"""Health endpoint tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Health endpoint
"""
from __future__ import annotations

import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.health import DependencyStatus


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
