"""KBGrowthLogger unit tests.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/kb-growth/spec.md
  Requirement: KBGrowthLogger writes kb_growth.jsonl
  Requirement: Required fields on every kb_growth.jsonl line
  Requirement: Event type field defaults to "add" with rollback reserved for P1
"""
from __future__ import annotations

import inspect
import json
import re
from pathlib import Path

import pytest

from codebus_agent.kb.growth_logger import KBGrowthLogger


_REQUIRED_KEYS = {
    "ts",
    "session_id",
    "question",
    "originating_station_id",
    "entry_id",
    "source",
    "related_stations",
    "reason",
    "sanitize_stats",
    "chunk_size_chars",
    "dedup_skipped",
    "event_type",
}


def _kwargs(**overrides):
    """Default kwargs for `write(...)` so individual tests can override one field."""
    base = dict(
        point_id="abcd1234-pt-01",
        source="src/x.py:10-20",
        reason="reusable storage adapter contract",
        related_stations=["s02-storage"],
        originating_station_id="s02-storage",
        sanitize_stats={"secret": 0},
        chunk_size_chars=42,
        dedup_skipped=False,
        session_id="qa_sess_01",
        question="how does the storage adapter work",
    )
    base.update(overrides)
    return base


def test_constructor_auto_mkdirs(tmp_path: Path) -> None:
    """`.codebus/` is created automatically; file does not yet exist."""
    target = tmp_path / ".codebus" / "kb_growth.jsonl"
    assert not target.parent.exists()
    KBGrowthLogger(target)
    assert target.parent.is_dir()
    assert not target.exists()


def test_write_appends_one_line(tmp_path: Path) -> None:
    """Single `write` produces one JSONL line containing all required keys."""
    target = tmp_path / ".codebus" / "kb_growth.jsonl"
    logger = KBGrowthLogger(target)
    logger.write(**_kwargs())

    text = target.read_text(encoding="utf-8")
    assert text.count("\n") == 1
    parsed = json.loads(text.strip())
    assert set(parsed.keys()) >= _REQUIRED_KEYS


def test_event_type_always_add_in_p0(tmp_path: Path) -> None:
    """All P0 lines MUST have `event_type` literal `"add"`; signature MUST NOT accept event_type kwarg."""
    target = tmp_path / ".codebus" / "kb_growth.jsonl"
    logger = KBGrowthLogger(target)
    logger.write(**_kwargs())

    parsed = json.loads(target.read_text(encoding="utf-8").strip())
    assert parsed["event_type"] == "add"

    sig = inspect.signature(KBGrowthLogger.write)
    assert "event_type" not in sig.parameters, (
        "KBGrowthLogger.write MUST NOT accept event_type kwarg in P0 — "
        "rollback path is reserved for a future P1 change"
    )


def test_invalid_station_id_raises_pre_write(tmp_path: Path) -> None:
    """Invalid `related_stations` MUST raise ValueError before any disk write."""
    target = tmp_path / ".codebus" / "kb_growth.jsonl"
    logger = KBGrowthLogger(target)

    with pytest.raises(ValueError) as excinfo:
        logger.write(**_kwargs(related_stations=["s9-bad"]))
    assert "s9-bad" in str(excinfo.value)
    # File MUST NOT have been created during the failed write.
    assert not target.exists() or target.stat().st_size == 0


def test_ts_iso_8601_with_utc(tmp_path: Path) -> None:
    """`ts` MUST match the kb-growth spec regex (ISO 8601 with UTC suffix)."""
    target = tmp_path / ".codebus" / "kb_growth.jsonl"
    logger = KBGrowthLogger(target)
    logger.write(**_kwargs())

    parsed = json.loads(target.read_text(encoding="utf-8").strip())
    pattern = r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$"
    assert re.match(pattern, parsed["ts"]), parsed["ts"]


def test_kbgrowthlogger_reexported_from_kb_package() -> None:
    """`from codebus_agent.kb import KBGrowthLogger` MUST work."""
    from codebus_agent.kb import KBGrowthLogger as Reexported

    assert Reexported is KBGrowthLogger
