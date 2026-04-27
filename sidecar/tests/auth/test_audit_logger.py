"""AuthorizationAuditLogger — append-only writer for the seventh audit layer.

Covers ``authorization-audit`` capability scenarios:
- "Logger constructor auto-creates parent directory"
- "Relative path raises at construction"
- "Each event method writes exactly one JSONL line"
- "Three-event audit schema with workspace_type discriminator" required fields
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from codebus_agent.auth.audit_logger import AuthorizationAuditLogger


@pytest.fixture
def audit_path(tmp_path: Path) -> Path:
    return tmp_path / "home" / ".codebus" / "authorization_audit.jsonl"


def test_constructor_rejects_relative_path(tmp_path: Path) -> None:
    rel = Path("relative_audit.jsonl")
    with pytest.raises(ValueError) as excinfo:
        AuthorizationAuditLogger(rel)
    assert "relative_audit.jsonl" in str(excinfo.value)


def test_constructor_rejects_relative_path_does_not_create_dir(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(tmp_path)
    rel = Path(".codebus/authorization_audit.jsonl")
    with pytest.raises(ValueError):
        AuthorizationAuditLogger(rel)
    assert not (tmp_path / ".codebus").exists()


def test_constructor_creates_parent_dir(audit_path: Path) -> None:
    assert not audit_path.parent.exists()
    AuthorizationAuditLogger(audit_path)
    assert audit_path.parent.is_dir()


def test_constructor_idempotent_when_parent_exists(audit_path: Path) -> None:
    audit_path.parent.mkdir(parents=True)
    AuthorizationAuditLogger(audit_path)  # MUST NOT raise


def _grant_issued_kwargs() -> dict:
    return {
        "session_id": "01928b1f-7c3a-4f9e-9c8b-a1b2c3d4e5f6",
        "workspace_id": "ws_a3f2b1c8d4e5",
        "workspace_type": "folder",
        "workspace_source": {"path": "C:/projects/timeline"},
        "scenario": "first_run",
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


def test_write_grant_issued_appends_one_jsonl_line(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_issued(**_grant_issued_kwargs())

    content = audit_path.read_text(encoding="utf-8")
    assert content.endswith("\n")
    assert content.count("\n") == 1
    line = json.loads(content)

    expected_keys = {
        "ts",
        "event",
        "session_id",
        "workspace_id",
        "workspace_type",
        "workspace_source",
        "scenario",
        "scope",
        "sanitizer_rules_version",
        "user_ack",
    }
    assert expected_keys.issubset(line.keys())
    assert line["event"] == "grant_issued"
    assert line["scenario"] == "first_run"
    assert line["workspace_type"] == "folder"


def test_write_grant_issued_two_calls_yield_two_lines(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_issued(**_grant_issued_kwargs())
    logger.write_grant_issued(**_grant_issued_kwargs())

    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 2
    for raw in lines:
        json.loads(raw)


def test_write_grant_denied_minimal_required_fields(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_denied(
        session_id="01928b1f-7c3a-4f9e-9c8b-a1b2c3d4e5f6",
        workspace_type="folder",
        workspace_source={"path": "C:/projects/timeline"},
        scenario="first_run",
        reason="user_cancelled",
    )

    line = json.loads(audit_path.read_text(encoding="utf-8"))
    expected_keys = {
        "ts",
        "event",
        "session_id",
        "workspace_type",
        "workspace_source",
        "scenario",
        "reason",
    }
    assert expected_keys.issubset(line.keys())
    assert line["event"] == "grant_denied"
    assert line["reason"] == "user_cancelled"


def test_write_grant_revoked_required_fields(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_revoked(
        session_id="01928b1f-7c3a-4f9e-9c8b-a1b2c3d4e5f6",
        workspace_id="ws_a3f2b1c8d4e5",
        grant_ts="2026-04-19T10:30:00.000+00:00",
        trigger="settings_revoke",
    )

    line = json.loads(audit_path.read_text(encoding="utf-8"))
    expected_keys = {
        "ts",
        "event",
        "session_id",
        "workspace_id",
        "grant_ts",
        "trigger",
    }
    assert expected_keys.issubset(line.keys())
    assert line["event"] == "grant_revoked"
    assert line["trigger"] == "settings_revoke"


def test_three_methods_write_distinct_event_kinds(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_issued(**_grant_issued_kwargs())
    logger.write_grant_denied(
        session_id="sess-2",
        workspace_type="folder",
        workspace_source={"path": "C:/projects/other"},
        scenario="first_run",
        reason="user_cancelled",
    )
    logger.write_grant_revoked(
        session_id="01928b1f-7c3a-4f9e-9c8b-a1b2c3d4e5f6",
        workspace_id="ws_a3f2b1c8d4e5",
        grant_ts="2026-04-19T10:30:00.000+00:00",
        trigger="settings_revoke",
    )

    lines = [
        json.loads(raw)
        for raw in audit_path.read_text(encoding="utf-8").splitlines()
    ]
    assert [entry["event"] for entry in lines] == [
        "grant_issued",
        "grant_denied",
        "grant_revoked",
    ]


def test_each_line_uses_lf_terminator_no_crlf(audit_path: Path) -> None:
    logger = AuthorizationAuditLogger(audit_path)
    logger.write_grant_issued(**_grant_issued_kwargs())
    raw_bytes = audit_path.read_bytes()
    assert b"\r\n" not in raw_bytes
    assert raw_bytes.endswith(b"\n")
