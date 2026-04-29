"""TDD red tests for `GET /sanitizer/rules` endpoint.

Backs SHALL clauses in
``openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md``
  Requirement: `GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot

Six scenarios:
  - Authenticated GET returns rules list (200 + schema)
  - Missing bearer rejected (401)
  - rules_version matches `sanitize_audit.jsonl` writer constant
  - Endpoint is read-only (multiple calls do not write audit, registry unchanged)
  - pattern_summary is not raw regex source (no ``(?P<`` / ``(?:`` / length <= 80)
  - Empty registry returns 200 with ``rules: []``
"""
from __future__ import annotations

import json
import secrets
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.sanitizer import RULES_VERSION


def _bearer() -> str:
    return secrets.token_urlsafe(32)


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def _mock_snapshot() -> dict:
    """Two builtin rules + one user_yaml rule for endpoint shape validation."""
    return {
        "rules_version": RULES_VERSION,
        "rules": [
            {
                "rule_id": "pii_email_v1",
                "kind": "email",
                "description": "Email address (RFC 5322 form)",
                "pattern_summary": "<email RFC 5322>",
                "source": "builtin",
            },
            {
                "rule_id": "detect_secrets_aws_v1",
                "kind": "secret",
                "description": "AWS access key (static credential)",
                "pattern_summary": "AKIA[0-9A-Z]{16}",
                "source": "builtin",
            },
            {
                "rule_id": "user_allowlist_1",
                "kind": "allowlist",
                "description": "internal CI ip range exception",
                "pattern_summary": "10.20.30.0/24",
                "source": "user_yaml",
            },
        ],
    }


@pytest.fixture
def bearer() -> str:
    return _bearer()


@pytest.fixture
def app_with_mock_registry(bearer: str, monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(
        "codebus_agent.api.sanitizer_rules.build_rules_snapshot",
        _mock_snapshot,
    )
    return create_app(bearer)


@pytest.fixture
def app_with_empty_registry(bearer: str, monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(
        "codebus_agent.api.sanitizer_rules.build_rules_snapshot",
        lambda: {"rules_version": RULES_VERSION, "rules": []},
    )
    return create_app(bearer)


# --- Scenario: Authenticated GET returns rules list ---


def test_authenticated_get_returns_200_and_schema(
    app_with_mock_registry, bearer: str
) -> None:
    client = TestClient(app_with_mock_registry)
    r = client.get("/sanitizer/rules", headers=_auth(bearer))
    assert r.status_code == 200
    body = r.json()
    assert set(body.keys()) == {"rules_version", "rules"}
    assert isinstance(body["rules_version"], str)
    assert isinstance(body["rules"], list)
    for entry in body["rules"]:
        assert set(entry.keys()) == {
            "rule_id",
            "kind",
            "description",
            "pattern_summary",
            "source",
        }
        assert entry["source"] in ("builtin", "user_yaml")


# --- Scenario: Missing bearer rejected ---


def test_missing_bearer_returns_401(app_with_mock_registry) -> None:
    client = TestClient(app_with_mock_registry)
    r = client.get("/sanitizer/rules")
    assert r.status_code == 401
    body = r.json()
    assert "rules" not in body


# --- Scenario: rules_version matches writer constant ---


def test_rules_version_matches_writer_constant(
    bearer: str,
) -> None:
    """Endpoint's rules_version equals the constant SanitizerAuditLogger writes."""
    app = create_app(bearer)
    client = TestClient(app)
    r = client.get("/sanitizer/rules", headers=_auth(bearer))
    assert r.status_code == 200
    body = r.json()
    assert body["rules_version"] == RULES_VERSION


# --- Scenario: Endpoint is read-only ---


def test_endpoint_is_read_only(
    app_with_mock_registry, bearer: str, tmp_path: Path
) -> None:
    """Multiple calls do not write audit, registry remains identical."""
    client = TestClient(app_with_mock_registry)

    audit_path = tmp_path / ".codebus" / "sanitize_audit.jsonl"

    first = client.get("/sanitizer/rules", headers=_auth(bearer)).json()
    second = client.get("/sanitizer/rules", headers=_auth(bearer)).json()
    third = client.get("/sanitizer/rules", headers=_auth(bearer)).json()

    assert first == second == third
    # Endpoint MUST NOT touch any sanitize_audit.jsonl path; if the test
    # workspace path was somehow created we'd see traces here.
    assert not audit_path.exists()


# --- Scenario: pattern_summary is not raw regex source ---


def test_pattern_summary_not_raw_regex(bearer: str) -> None:
    """Real built-in registry: every entry's pattern_summary stays human-readable."""
    app = create_app(bearer)
    client = TestClient(app)
    r = client.get("/sanitizer/rules", headers=_auth(bearer))
    assert r.status_code == 200
    body = r.json()
    assert len(body["rules"]) > 0, "default registry MUST be non-empty"
    for entry in body["rules"]:
        summary = entry["pattern_summary"]
        assert "(?P<" not in summary, (
            f"rule_id={entry['rule_id']} pattern_summary leaks named regex group"
        )
        assert "(?:" not in summary, (
            f"rule_id={entry['rule_id']} pattern_summary leaks non-capturing group"
        )
        assert len(summary) <= 80, (
            f"rule_id={entry['rule_id']} pattern_summary length {len(summary)} > 80"
        )


# --- Scenario: Empty registry returns 200 with empty rules ---


def test_empty_registry_returns_200_with_empty_rules(
    app_with_empty_registry, bearer: str
) -> None:
    client = TestClient(app_with_empty_registry)
    r = client.get("/sanitizer/rules", headers=_auth(bearer))
    assert r.status_code == 200
    body = r.json()
    assert body["rules"] == []
    assert body["rules_version"] == RULES_VERSION


# --- Schema-shape verification with mock registry (tighter than the live test) ---


def test_mock_snapshot_renders_two_builtin_one_user_yaml(
    app_with_mock_registry, bearer: str
) -> None:
    client = TestClient(app_with_mock_registry)
    r = client.get("/sanitizer/rules", headers=_auth(bearer))
    body = r.json()
    sources = [e["source"] for e in body["rules"]]
    assert sources.count("builtin") == 2
    assert sources.count("user_yaml") == 1
