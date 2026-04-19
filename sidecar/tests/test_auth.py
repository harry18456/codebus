"""Bearer token authentication tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/sidecar-runtime/spec.md
  Requirement: Bearer token authentication
"""
from __future__ import annotations

import secrets

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from codebus_agent.api import create_app


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


@pytest.fixture
def app_with_probe(bearer: str) -> FastAPI:
    app = create_app(bearer_token=bearer)

    @app.get("/__probe__")
    def probe() -> dict[str, str]:
        return {"status": "reached"}

    return app


@pytest.fixture
def client(app_with_probe: FastAPI) -> TestClient:
    return TestClient(app_with_probe)


def test_missing_bearer_rejected(client: TestClient) -> None:
    """Scenario: Missing bearer rejected."""
    response = client.get("/__probe__")
    assert response.status_code == 401


def test_wrong_bearer_rejected(client: TestClient) -> None:
    """Scenario: Wrong bearer rejected."""
    response = client.get(
        "/__probe__",
        headers={"Authorization": "Bearer not-the-real-token"},
    )
    assert response.status_code == 401


def test_correct_bearer_accepted(client: TestClient, bearer: str) -> None:
    """Scenario: Correct bearer accepted."""
    response = client.get(
        "/__probe__",
        headers={"Authorization": f"Bearer {bearer}"},
    )
    assert response.status_code == 200
    assert response.json() == {"status": "reached"}


def test_malformed_auth_header_rejected(client: TestClient, bearer: str) -> None:
    """Audit lens (sharp edges): a non-Bearer scheme must NOT be accepted
    even if it carries the right token — otherwise `Basic <token>` could
    sneak through a naive startswith check.  Not a SHALL-level scenario
    but a direct consequence of the 'Bearer' scheme requirement.
    """
    response = client.get(
        "/__probe__",
        headers={"Authorization": f"Basic {bearer}"},
    )
    assert response.status_code == 401


def test_timing_safe_comparison(bearer: str) -> None:
    """Token comparison must use a constant-time function.  This is a
    structural assertion — we check the middleware uses `secrets.compare_digest`
    by patching it and observing the call.  Prevents future refactors from
    regressing to `==` comparison (timing-oracle risk).
    """
    from unittest.mock import patch

    app = create_app(bearer_token=bearer)

    @app.get("/__probe__")
    def probe() -> dict[str, str]:
        return {"status": "reached"}

    with patch("codebus_agent.auth.secrets.compare_digest", wraps=secrets.compare_digest) as spy:
        with TestClient(app) as c:
            c.get("/__probe__", headers={"Authorization": f"Bearer {bearer}"})
        assert spy.called, "bearer comparison must go through secrets.compare_digest"
