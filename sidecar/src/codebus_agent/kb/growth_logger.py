"""KBGrowthLogger — append-only `kb_growth.jsonl` writer (seventh audit layer).

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/kb-growth/spec.md
  Requirement: KBGrowthLogger writes kb_growth.jsonl
  Requirement: Required fields on every kb_growth.jsonl line
  Requirement: Event type field defaults to "add" with rollback reserved for P1

Each ``write(...)`` call appends exactly one JSON line to
``<workspace>/.codebus/kb_growth.jsonl``. The logger is the single
source of truth for this path; no other production module SHALL open it
for writing (see capability spec scenario "Single source of truth for
the path").

Per Decision 7 (`module-8-qa-p0` design): the P0 schema already includes
`event_type` set to the literal string `"add"`. A future P1 change adds
keyword-only `event_type` parameter to expose the `"rollback"` value;
the schema remains backward-compatible because the field already exists.
"""
from __future__ import annotations

import json
import threading
from datetime import datetime, timezone
from pathlib import Path

from codebus_agent.agent.station_id import _STATION_ID_RE

__all__ = ["KBGrowthLogger"]


def _validate_station_ids(related_stations: list[str]) -> None:
    for sid in related_stations:
        if not isinstance(sid, str) or not _STATION_ID_RE.fullmatch(sid):
            raise ValueError(
                f"related_stations entry {sid!r} must match "
                f"{_STATION_ID_RE.pattern}"
            )


class KBGrowthLogger:
    """Append-only JSONL writer for the seventh workspace audit layer."""

    def __init__(self, path: Path) -> None:
        self._path = Path(path)
        # Auto-mkdir parent — mirror UsageTracker / LLMCallLogger convention so
        # callers don't need to pre-create `.codebus/`. Spec scenario
        # "Constructor auto-creates .codebus parent" requires this.
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._lock = threading.Lock()

    @property
    def path(self) -> Path:
        return self._path

    def write(
        self,
        *,
        point_id: str,
        source: str,
        reason: str,
        related_stations: list[str],
        originating_station_id: str | None,
        sanitize_stats: dict[str, int],
        chunk_size_chars: int,
        dedup_skipped: bool,
        session_id: str,
        question: str | None,
    ) -> None:
        """Append exactly one JSON line.

        Pre-validates `related_stations` regex before disk I/O so a
        malformed station id surfaces as ``ValueError`` and the audit
        chain never persists invalid station references (spec scenario
        "Invalid station id rejected pre-write").

        `event_type` is hardcoded to `"add"` per Decision 7; the
        absence of an `event_type` kwarg in this signature is the
        machine-checkable guarantee that P0 callers cannot drift the
        audit semantic.
        """
        _validate_station_ids(related_stations)

        line = {
            "ts": _iso_utc_now(),
            "session_id": session_id,
            "question": question,
            "originating_station_id": originating_station_id,
            "entry_id": point_id,
            "source": source,
            "related_stations": list(related_stations),
            "reason": reason,
            "sanitize_stats": dict(sanitize_stats),
            "chunk_size_chars": int(chunk_size_chars),
            "dedup_skipped": bool(dedup_skipped),
            "event_type": "add",
        }
        payload = json.dumps(line, ensure_ascii=False) + "\n"
        with self._lock:
            with self._path.open("a", encoding="utf-8") as fp:
                fp.write(payload)


def _iso_utc_now() -> str:
    """ISO 8601 timestamp with UTC offset suffix (matches kb-growth spec regex)."""
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")
