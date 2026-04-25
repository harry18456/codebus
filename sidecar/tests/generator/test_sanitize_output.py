"""Tests for Generator output Pass 1 sanitization (Section 8).

Backs Requirement
``Generator output passes Sanitizer Pass 1 before disk write``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Decision 1: defense in depth — even though LLM input is already
Pass 1 + Pass 2 sanitized, the LLM output (creative entity) MUST go
through Pass 1 before hitting disk.
"""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

import pytest

from codebus_agent.agent.types import Station
from codebus_agent.generator.log import GeneratorLogger
from codebus_agent.generator.station import StationContext, generate_station
from codebus_agent.generator.types import StationMarkdown
from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine


_TS = datetime(2026, 4, 25, 10, 30, 0, tzinfo=timezone.utc)


def _ctx(workspace_dir: Path, provider: TrackedProvider) -> StationContext:
    audit_dir = workspace_dir / ".codebus"
    audit_dir.mkdir(parents=True, exist_ok=True)
    out_dir = (
        workspace_dir / "codebus-tutorials" / "generate_test1234" / "stations"
    )
    return StationContext(
        workspace_root=workspace_dir,
        output_path=out_dir / "s02-storage.md",
        provider=provider,
        sanitizer=SanitizerEngine(),
        sanitizer_audit=SanitizerAuditLogger(audit_dir / "sanitize_audit.jsonl"),
        rules_version="2026-04-20-1",
        log=GeneratorLogger(audit_dir / "generator_log.jsonl"),
        station_index=2,
        station_id="s02-storage",
        station_title="Storage",
        task="add gdrive adapter",
        repo_name=workspace_dir.name,
        workspace_type="folder",
        generated_at=_TS,
    )


def _read_audit(workspace_dir: Path) -> list[dict]:
    p = workspace_dir / ".codebus" / "sanitize_audit.jsonl"
    if not p.exists():
        return []
    return [json.loads(line) for line in p.read_text(encoding="utf-8").splitlines()]


@pytest.mark.asyncio
async def test_station_file_content_with_pii_pattern_triggers_pass_1_hit(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
) -> None:
    body = (
        "# Storage\n\n"
        "請寄信到 alice@example.com 索取存取權，再來看 Storage 介面。\n\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] 確認可拿到 access\n</Checkpoint>\n"
    )
    mock_script_generate.push(StationMarkdown(thought="ok", body=body))

    ctx = _ctx(tmp_path, mock_generate_provider)
    outcome = await generate_station(
        station=Station(
            path="src/storage.ts", role="interface", relevance=0.8, why="."
        ),
        ctx=ctx,
    )
    assert outcome.degraded is False

    text = outcome.station_path.read_text(encoding="utf-8")
    assert "alice@example.com" not in text, (
        "raw email MUST be scrubbed by Pass 1 before disk write"
    )
    assert "<REDACTED:" in text, (
        f"sanitized placeholder MUST appear; got body:\n{text!r}"
    )

    # Pass 1 audit entry MUST land in <ws>/.codebus/sanitize_audit.jsonl
    # with pass_num=1 and source.path tied to the station file path.
    audit_lines = _read_audit(tmp_path)
    pass1_hits = [
        line
        for line in audit_lines
        if line.get("pass") == 1
        and isinstance(line.get("source"), dict)
        and line["source"].get("pass") == "generator"
    ]
    assert pass1_hits, (
        f"expected at least one Pass 1 generator audit entry; got {audit_lines!r}"
    )
    matching = [
        h
        for h in pass1_hits
        if str(outcome.station_path) in h["source"]["path"]
        or h["source"]["path"].endswith("s02-storage.md")
    ]
    assert matching, (
        f"audit source.path must reference the station's output path; "
        f"got entries {pass1_hits!r}"
    )


@pytest.mark.asyncio
async def test_clean_llm_output_writes_verbatim_with_no_audit_entries(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider: TrackedProvider,
) -> None:
    body = (
        "# Storage\n\n"
        "Storage 介面定義在 src/storage.ts，提供 get/put 兩個動作。\n\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] 對齊核心 API\n</Checkpoint>\n"
    )
    mock_script_generate.push(StationMarkdown(thought="ok", body=body))

    ctx = _ctx(tmp_path, mock_generate_provider)
    outcome = await generate_station(
        station=Station(
            path="src/storage.ts", role="interface", relevance=0.5, why="."
        ),
        ctx=ctx,
    )
    text = outcome.station_path.read_text(encoding="utf-8")
    assert "<REDACTED:" not in text, "clean output MUST NOT carry placeholder"
    # No new generator-side Pass 1 audit lines for this station.
    audit_lines = _read_audit(tmp_path)
    pass1_generator_hits = [
        line
        for line in audit_lines
        if line.get("pass") == 1
        and isinstance(line.get("source"), dict)
        and line["source"].get("pass") == "generator"
    ]
    assert pass1_generator_hits == [], (
        f"clean output MUST NOT add audit entries; got {pass1_generator_hits!r}"
    )
