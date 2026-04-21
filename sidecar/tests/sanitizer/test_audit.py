"""Tests for `SanitizerAuditLogger` — covers Requirements
"SanitizerAuditLogger appends each replacement to JSONL" and
"Rules version is recorded on every audit line".
"""
from __future__ import annotations

import json
import threading
import uuid
from pathlib import Path

from codebus_agent.sanitizer import AuditEntry, SanitizerAuditLogger


_REQUIRED_FIELDS = {
    "ts",
    "schema_version",
    "rules_version",
    "pass",
    "session_id",
    "source",
    "rule_id",
    "kind",
    "placeholder_index",
    "extra",
}


def _make_entry(kind: str = "email") -> AuditEntry:
    return AuditEntry(
        rule_id=f"pii_{kind}_v1",
        kind=kind,
        placeholder_index=1,
        source="file:src/app.py",
    )


def test_audit_line_contains_required_fields(tmp_path):
    audit_path = tmp_path / "sanitize_audit.jsonl"
    logger = SanitizerAuditLogger(audit_path)

    session_id = str(uuid.uuid4())
    logger.append(
        entry=_make_entry(),
        pass_num=1,
        rules_version="2026-04-20-1",
        session_id=session_id,
    )

    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    parsed = json.loads(lines[0])
    assert set(parsed.keys()) == _REQUIRED_FIELDS
    assert parsed["extra"] == {}
    assert parsed["pass"] == 1
    assert parsed["kind"] == "email"
    assert parsed["placeholder_index"] == 1
    assert parsed["rule_id"] == "pii_email_v1"
    assert parsed["source"] == "file:src/app.py"
    assert parsed["session_id"] == session_id


def test_audit_rules_version_propagates_from_config(tmp_path):
    logger = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")
    logger.append(
        entry=_make_entry(),
        pass_num=1,
        rules_version="2026-04-20-1",
        session_id="abc",
    )

    parsed = json.loads((tmp_path / "sanitize_audit.jsonl").read_text().splitlines()[0])
    assert parsed["rules_version"] == "2026-04-20-1"


def test_audit_schema_version_equals_1(tmp_path):
    logger = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")
    logger.append(
        entry=_make_entry(),
        pass_num=2,
        rules_version="2026-04-20-1",
        session_id="abc",
    )

    parsed = json.loads((tmp_path / "sanitize_audit.jsonl").read_text().splitlines()[0])
    assert parsed["schema_version"] == 1


def test_audit_extra_field_round_trips(tmp_path):
    entry = AuditEntry(
        rule_id="pii_tw_mobile_v1",
        kind="phone",
        placeholder_index=2,
        source="message:chat_req_xyz",
        extra={"allowlisted": True, "nested": {"k": 1}},
    )
    logger = SanitizerAuditLogger(tmp_path / "sanitize_audit.jsonl")
    logger.append(
        entry=entry,
        pass_num=2,
        rules_version="2026-04-20-1",
        session_id="s1",
    )

    parsed = json.loads((tmp_path / "sanitize_audit.jsonl").read_text().splitlines()[0])
    assert parsed["extra"] == {"allowlisted": True, "nested": {"k": 1}}


def test_audit_concurrent_writes_atomic(tmp_path):
    """Two threads appending in parallel MUST NOT interleave bytes."""
    audit_path = tmp_path / "sanitize_audit.jsonl"
    logger = SanitizerAuditLogger(audit_path)

    N = 100

    def worker(thread_id: int) -> None:
        for i in range(N):
            entry = AuditEntry(
                rule_id="pii_email_v1",
                kind="email",
                placeholder_index=i + 1,
                source=f"file:t{thread_id}_{i}.py",
            )
            logger.append(
                entry=entry,
                pass_num=1,
                rules_version="2026-04-20-1",
                session_id=f"thread-{thread_id}",
            )

    threads = [threading.Thread(target=worker, args=(tid,)) for tid in range(2)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()

    # Every written line must be a full JSON object terminated by \n.
    raw = audit_path.read_bytes()
    assert raw.endswith(b"\n"), "file must end with newline"
    lines = audit_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == N * 2
    for line in lines:
        parsed = json.loads(line)  # would raise if interleaved
        assert set(parsed.keys()) == _REQUIRED_FIELDS


def test_audit_creates_parent_dir(tmp_path):
    nested = tmp_path / "some" / "deep" / "dir" / "sanitize_audit.jsonl"
    logger = SanitizerAuditLogger(nested)
    logger.append(
        entry=_make_entry(),
        pass_num=1,
        rules_version="v1",
        session_id="s1",
    )
    assert nested.exists()
