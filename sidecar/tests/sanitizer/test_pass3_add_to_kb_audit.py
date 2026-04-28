"""Tests for Pass 3 `add_to_kb` sanitize audit emission.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/sanitizer/spec.md
  Requirement: Pass 3 add_to_kb sanitize emits structured audit entry
"""
from __future__ import annotations

import json
import re
from pathlib import Path

import pytest

from codebus_agent.sanitizer import (
    FileSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)
from codebus_agent.sanitizer import RULES_VERSION


def _build_engine_and_audit(tmp_path: Path) -> tuple[SanitizerEngine, SanitizerAuditLogger, Path]:
    audit_path = tmp_path / ".codebus" / "sanitize_audit.jsonl"
    return SanitizerEngine(), SanitizerAuditLogger(audit_path), audit_path


async def _record_pass3(
    engine: SanitizerEngine,
    audit: SanitizerAuditLogger,
    *,
    text: str,
    chunk_source: str,
    session_id: str = "qa_sess_01",
) -> None:
    """Run Pass 3 sanitize and route hits into audit logger."""
    result = await engine.sanitize(
        text, source=FileSource(path=chunk_source, pass_="qa_add_to_kb")
    )
    for entry in result.entries:
        audit.append(
            entry=entry,
            pass_num=3,
            rules_version=RULES_VERSION,
            session_id=session_id,
        )


def _read_lines(audit_path: Path) -> list[dict]:
    if not audit_path.exists():
        return []
    return [json.loads(line) for line in audit_path.read_text(encoding="utf-8").splitlines()]


@pytest.mark.asyncio
async def test_pass_num_3_on_audit_line(tmp_path: Path) -> None:
    """Pass 3 audit lines MUST carry the pass discriminator value 3."""
    engine, audit, audit_path = _build_engine_and_audit(tmp_path)
    await _record_pass3(
        engine,
        audit,
        text="email is foo@example.com",
        chunk_source="src/x.py:10-20",
    )

    lines = _read_lines(audit_path)
    assert len(lines) >= 1
    # The audit logger writes the pass discriminator under the `"pass"` key
    # (existing SanitizerAuditLogger schema preserved across Pass 1 / 2 / 3).
    assert lines[0]["pass"] == 3


@pytest.mark.asyncio
async def test_source_field_structured_form(tmp_path: Path) -> None:
    """Pass 3 audit lines MUST use the structured `{"pass", "path"}` source form."""
    engine, audit, audit_path = _build_engine_and_audit(tmp_path)
    await _record_pass3(
        engine,
        audit,
        text="email is foo@example.com",
        chunk_source="src/x.py:10-20",
    )

    lines = _read_lines(audit_path)
    assert len(lines) >= 1
    src = lines[0]["source"]
    assert isinstance(src, dict)
    assert src == {"pass": "qa_add_to_kb", "path": "src/x.py:10-20"}


def test_sanitize_source_union_not_extended() -> None:
    """`SanitizeSource` MUST remain `FileSource | MessageSource` exactly."""
    engine_path = (
        Path(__file__).resolve().parents[2]
        / "src"
        / "codebus_agent"
        / "sanitizer"
        / "engine.py"
    )
    text = engine_path.read_text(encoding="utf-8")
    match = re.search(r"^\s*SanitizeSource\s*=\s*(.+)$", text, flags=re.MULTILINE)
    assert match is not None, "SanitizeSource assignment not found"
    rhs = match.group(1).strip()
    assert rhs == "FileSource | MessageSource", rhs
    assert "Pass3Source" not in text
    assert "QASource" not in text


@pytest.mark.asyncio
async def test_empty_post_sanitize_still_records_hits(tmp_path: Path) -> None:
    """Even when caller-side decides to skip KB write, Pass 3 hits MUST be audited."""
    engine, audit, audit_path = _build_engine_and_audit(tmp_path)
    # Construct text so sanitizer rules redact (email rule is built-in).
    # Multiple emails so multiple audit entries land per call.
    text = "user1@example.com user2@example.com user3@example.com"
    await _record_pass3(engine, audit, text=text, chunk_source="src/secrets.py:1-3")

    lines = _read_lines(audit_path)
    assert any(line["pass"] == 3 for line in lines)
    # Caller-side empty-chunk decision is OUT OF SCOPE for the engine —
    # the engine's audit lines MUST land regardless of what the caller
    # does with `result.text` afterward.
