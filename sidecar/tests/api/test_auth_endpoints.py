"""Tests for ``POST /auth/{grant,deny,revoke}`` and ``GET /auth/status``.

Covers ``authorization-audit`` capability scenarios:
- Bearer middleware enforced on all four endpoints
- POST /auth/grant rejects invalid workspace path
- POST /auth/grant on success returns session_id and writes audit
- POST /auth/deny writes audit and creates no session
- POST /auth/revoke without active session returns 404
- GET /auth/status reads audit log fresh on each call
- workspace_type=topic returns 501 (handler-level, schema accepts)

Plus ``sidecar-runtime`` Modified Requirement scenarios:
- Auth router included in app factory
- Auth endpoints return 503 when factory is None
- Auth endpoints reject missing bearer
"""
from __future__ import annotations

import json
import secrets
from pathlib import Path
from typing import Callable

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.auth.audit_logger import AuthorizationAuditLogger


def _bearer() -> str:
    return secrets.token_hex(32)


def _factory(audit_path: Path) -> Callable[[], AuthorizationAuditLogger]:
    return lambda: AuthorizationAuditLogger(audit_path)


def _grant_body(workspace: Path, scenario: str = "first_run") -> dict:
    return {
        "workspace_type": "folder",
        "workspace_source": {"path": str(workspace)},
        "scenario": scenario,
        "scope": {
            "llm_provider": "anthropic",
            "llm_model": "claude-haiku-4.5",
            "outbound_endpoint": "api.anthropic.com",
        },
        "sanitizer_rules_version": "2026-04-20-1",
        "user_ack": [
            "raw_stays_local",
            "no_kb_persist",
            "outbound_to_anthropic",
        ],
    }


def _deny_body(workspace: Path) -> dict:
    return {
        "workspace_type": "folder",
        "workspace_source": {"path": str(workspace)},
        "scenario": "first_run",
        "reason": "user_cancelled",
    }


@pytest.fixture
def app_and_audit(tmp_path: Path):
    bearer = _bearer()
    audit_path = tmp_path / ".codebus" / "authorization_audit.jsonl"
    app = create_app(bearer, auth_audit_logger_factory=_factory(audit_path))
    return app, bearer, audit_path


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    ws = tmp_path / "projects" / "timeline"
    ws.mkdir(parents=True)
    return ws


# --- bearer middleware -----------------------------------------------------


def test_endpoints_reject_missing_bearer(app_and_audit, workspace: Path) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    for method, path, payload in [
        ("post", "/auth/grant", _grant_body(workspace)),
        ("post", "/auth/deny", _deny_body(workspace)),
        (
            "post",
            "/auth/revoke",
            {"session_id": "x", "trigger": "settings_revoke"},
        ),
        ("get", "/auth/status?workspace_id=ws_x", None),
    ]:
        if method == "post":
            r = client.post(path, json=payload)
        else:
            r = client.get(path)
        assert r.status_code == 401, f"{method.upper()} {path} should require bearer"


# --- factory missing ------------------------------------------------------


def test_grant_factory_none_returns_503(workspace: Path) -> None:
    bearer = _bearer()
    app = create_app(bearer, auth_audit_logger_factory=None)
    client = TestClient(app)
    r = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert r.status_code == 503
    assert r.json()["detail"]["code"] == "AUTH_NOT_CONFIGURED"


# --- workspace validation -------------------------------------------------


def test_grant_workspace_invalid_path_returns_400(
    app_and_audit, tmp_path: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    not_a_dir = tmp_path / "this_is_a_file"
    not_a_dir.write_text("hello")
    client = TestClient(app)

    r = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(not_a_dir),
    )
    assert r.status_code == 400
    assert r.json()["detail"]["code"] == "AUTH_WORKSPACE_INVALID"
    assert not audit_path.exists() or audit_path.read_text() == ""


def test_grant_workspace_path_does_not_exist_returns_400(
    app_and_audit, tmp_path: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    missing = tmp_path / "nonexistent"
    client = TestClient(app)

    r = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(missing),
    )
    assert r.status_code == 400
    assert r.json()["detail"]["code"] == "AUTH_WORKSPACE_INVALID"


# --- happy path -----------------------------------------------------------


def test_grant_first_run_success_writes_audit_returns_200(
    app_and_audit, workspace: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    r = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert r.status_code == 200
    body = r.json()
    assert set(body.keys()) >= {"session_id", "workspace_id", "granted_at"}

    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    entry = json.loads(lines[0])
    assert entry["event"] == "grant_issued"
    assert entry["session_id"] == body["session_id"]
    assert entry["workspace_id"] == body["workspace_id"]


# --- scenario invariants --------------------------------------------------


def test_grant_first_run_with_prior_returns_400(
    app_and_audit, workspace: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    r1 = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert r1.status_code == 200

    r2 = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert r2.status_code == 400
    assert r2.json()["detail"]["code"] == "AUTH_INVALID_REQUEST"


# --- deny -----------------------------------------------------------------


def test_deny_writes_audit_returns_204(app_and_audit, workspace: Path) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    r = client.post(
        "/auth/deny",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_deny_body(workspace),
    )
    assert r.status_code == 204

    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    entry = json.loads(lines[0])
    assert entry["event"] == "grant_denied"
    assert entry["reason"] == "user_cancelled"


# --- revoke ---------------------------------------------------------------


def test_revoke_unknown_session_returns_404(app_and_audit) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    r = client.post(
        "/auth/revoke",
        headers={"Authorization": f"Bearer {bearer}"},
        json={
            "session_id": "00000000-0000-4000-8000-000000000000",
            "trigger": "settings_revoke",
        },
    )
    assert r.status_code == 404
    assert r.json()["detail"]["code"] == "AUTH_NO_ACTIVE_GRANT"


def test_revoke_active_session_writes_grant_revoked_and_clears_session(
    app_and_audit, workspace: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    grant = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert grant.status_code == 200
    session_id = grant.json()["session_id"]

    revoke = client.post(
        "/auth/revoke",
        headers={"Authorization": f"Bearer {bearer}"},
        json={"session_id": session_id, "trigger": "settings_revoke"},
    )
    assert revoke.status_code == 204

    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 2
    assert json.loads(lines[0])["event"] == "grant_issued"
    assert json.loads(lines[1])["event"] == "grant_revoked"

    # Second revoke must now 404 (session cleared).
    revoke2 = client.post(
        "/auth/revoke",
        headers={"Authorization": f"Bearer {bearer}"},
        json={"session_id": session_id, "trigger": "settings_revoke"},
    )
    assert revoke2.status_code == 404


# --- status ---------------------------------------------------------------


def test_status_no_grants_returns_has_active_grant_false(
    app_and_audit,
) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    r = client.get(
        "/auth/status?workspace_id=ws_unknown_xx",
        headers={"Authorization": f"Bearer {bearer}"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["has_active_grant"] is False
    assert body["session_id"] is None
    assert body["last_grant"] is None
    assert body["current_rules_version"]


def test_status_after_grant_returns_last_grant_payload(
    app_and_audit, workspace: Path
) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    grant = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=_grant_body(workspace),
    )
    assert grant.status_code == 200
    workspace_id = grant.json()["workspace_id"]

    r = client.get(
        f"/auth/status?workspace_id={workspace_id}",
        headers={"Authorization": f"Bearer {bearer}"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["has_active_grant"] is True
    assert body["session_id"] == grant.json()["session_id"]
    assert body["last_grant"] is not None
    assert body["last_grant"]["event"] == "grant_issued"


# --- topic mode -----------------------------------------------------------


def test_topic_workspace_type_returns_501(app_and_audit, workspace: Path) -> None:
    app, bearer, audit_path = app_and_audit
    client = TestClient(app)

    body = _grant_body(workspace)
    body["workspace_type"] = "topic"
    body["workspace_source"] = {
        "query": "react hooks",
        "seed_urls": [],
        "domain_allowlist": [],
    }
    r = client.post(
        "/auth/grant",
        headers={"Authorization": f"Bearer {bearer}"},
        json=body,
    )
    assert r.status_code == 501
    assert r.json()["detail"]["code"] == "AUTH_INVALID_REQUEST"


# --- task_id regex unchanged ---------------------------------------------


def test_task_id_regex_pattern_does_not_include_auth() -> None:
    """``sidecar-runtime`` scenario "task_id regex unchanged" — no `auth`
    prefix added even though /auth/* exists."""
    from codebus_agent.api import tasks as tasks_module

    source = Path(tasks_module.__file__).read_text(encoding="utf-8")
    canonical = "^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$"
    assert canonical in source
    assert "auth|scan" not in source
    assert "scan|auth" not in source
    assert "qa|auth" not in source
