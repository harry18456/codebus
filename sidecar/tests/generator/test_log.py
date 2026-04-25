"""Tests for ``generator_log.jsonl`` writer (Section 11).

Backs Requirement
``Degraded fallback writes per-station stub after retry exhaustion``
(log-write segment).
"""
from __future__ import annotations

import json
from pathlib import Path

from codebus_agent.generator.log import GeneratorLogger


def _read_lines(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_degraded_event_appended_with_required_keys(tmp_path: Path) -> None:
    log_path = tmp_path / ".codebus" / "generator_log.jsonl"
    log = GeneratorLogger(log_path)
    log.append(
        event="degraded",
        station_id="s02-storage",
        station_index=2,
        attempts=3,
        last_issues=["missing_checkpoint", "too_long"],
    )
    [entry] = _read_lines(log_path)
    for key in ("timestamp", "station_id", "station_index", "attempts", "last_issues"):
        assert key in entry, f"missing required key {key} in {entry!r}"
    assert entry["event"] == "degraded"
    assert entry["attempts"] == 3
    assert entry["last_issues"] == ["missing_checkpoint", "too_long"]


def test_write_failed_event_appended(tmp_path: Path) -> None:
    log_path = tmp_path / ".codebus" / "generator_log.jsonl"
    log = GeneratorLogger(log_path)
    log.append(
        event="write_failed",
        station_id="s03-adapter",
        station_index=3,
        error="OSError: disk full",
    )
    [entry] = _read_lines(log_path)
    assert entry["event"] == "write_failed"
    assert entry["station_id"] == "s03-adapter"
    assert entry["error"] == "OSError: disk full"


def test_log_path_under_codebus_subdir(tmp_path: Path) -> None:
    log_path = tmp_path / ".codebus" / "generator_log.jsonl"
    GeneratorLogger(log_path).append(
        event="degraded",
        station_id="s01-x",
        station_index=1,
        attempts=3,
        last_issues=[],
    )
    assert log_path.exists()
    assert log_path.parent.name == ".codebus"
