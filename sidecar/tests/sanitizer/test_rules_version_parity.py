"""Cross-check: ``GET /sanitizer/rules`` rules_version vs sanitize_audit writer.

Backs SHALL clauses in
``openspec/changes/sanitizer-audit-inspector-p0/specs/sanitizer-audit-inspector/spec.md``
  Requirement: `GET /sanitizer/rules` sidecar endpoint exposes rules registry snapshot
    Scenario: rules_version matches sanitize_audit.jsonl writes

Strategy: spin up a real SanitizerEngine in the same process, run a
sanitize call that emits at least one audit row, then call the
endpoint and compare ``rules_version`` strings character-for-character.
"""
from __future__ import annotations

import json
import secrets
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from codebus_agent.api import create_app
from codebus_agent.sanitizer import (
    FileSource,
    RULES_VERSION,
    SanitizerAuditLogger,
    make_default_engine,
)


@pytest.mark.asyncio
async def test_endpoint_rules_version_equals_sanitize_audit_rules_version(
    tmp_path: Path,
) -> None:
    """Engine writes rules_version=X to JSONL; endpoint MUST also return X."""
    audit_path = tmp_path / ".codebus" / "sanitize_audit.jsonl"
    audit_logger = SanitizerAuditLogger(audit_path)
    engine = make_default_engine()

    text_with_email = "contact me at alice@example.com please"
    result = await engine.sanitize(text_with_email, FileSource(path="src/app.py"))
    assert result.entries, "engine MUST produce at least one entry to test parity"

    for entry in result.entries:
        audit_logger.append(
            entry=entry,
            pass_num=1,
            rules_version=RULES_VERSION,
            session_id="parity_session",
        )

    raw_lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert raw_lines, "audit log MUST be non-empty after sanitize"
    audit_rules_version = json.loads(raw_lines[0])["rules_version"]

    bearer = secrets.token_urlsafe(32)
    app = create_app(bearer)
    client = TestClient(app)
    r = client.get(
        "/sanitizer/rules",
        headers={"Authorization": f"Bearer {bearer}"},
    )
    assert r.status_code == 200
    endpoint_rules_version = r.json()["rules_version"]

    assert endpoint_rules_version == audit_rules_version, (
        "endpoint rules_version MUST match the value sanitize_audit.jsonl writers emit"
    )
    # Also pin to the constant — defensive double-check that neither side
    # silently drifted from the documented constant.
    assert endpoint_rules_version == RULES_VERSION
