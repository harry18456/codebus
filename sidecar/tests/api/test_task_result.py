"""TDD red tests for `GET /tasks/{id}/result` — Section 6 of
openspec/changes/sse-progress-skeleton/tasks.md.

Backs openspec/changes/sse-progress-skeleton/specs/sidecar-runtime/spec.md
  Requirement: Task result lookup endpoint
"""
from __future__ import annotations

import secrets

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.api.tasks import TaskRegistry


@pytest.fixture
def bearer() -> str:
    return secrets.token_urlsafe(32)


@pytest.fixture
def app_with_registry(bearer: str):
    """Build an app and expose its registry so tests can pre-seed handles."""
    app = create_app(bearer_token=bearer)
    return app, app.state.tasks


def _auth(bearer: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {bearer}"}


def test_result_returns_200_when_done(app_with_registry, bearer) -> None:
    app, registry = app_with_registry
    handle = registry.create("scan")
    assert handle is not None
    handle.status = "done"
    handle.result = {"workspace_root": "/tmp/x", "files": []}

    client = TestClient(app)
    resp = client.get(f"/tasks/{handle.id}/result", headers=_auth(bearer))
    assert resp.status_code == 200
    assert resp.json() == {"workspace_root": "/tmp/x", "files": []}


def test_result_returns_409_when_running(app_with_registry, bearer) -> None:
    app, registry = app_with_registry
    handle = registry.create("scan")
    assert handle is not None
    # status stays "running" by default

    client = TestClient(app)
    resp = client.get(f"/tasks/{handle.id}/result", headers=_auth(bearer))
    assert resp.status_code == 409
    body = resp.json()
    detail = body.get("detail", body)
    assert detail.get("code") == "TASK_NOT_DONE"
    assert detail.get("status") == "running"


def test_result_returns_404_when_unknown(app_with_registry, bearer) -> None:
    app, _registry = app_with_registry
    client = TestClient(app)
    resp = client.get("/tasks/scan_deadbeef/result", headers=_auth(bearer))
    assert resp.status_code == 404


def test_result_requires_bearer(app_with_registry) -> None:
    app, _registry = app_with_registry
    client = TestClient(app)
    resp = client.get("/tasks/scan_deadbeef/result")
    assert resp.status_code == 401
    assert resp.json() == {"detail": "unauthorized"}
