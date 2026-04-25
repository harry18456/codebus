"""``route.json`` writer — D-029 §八 schema.

Backs Requirement
``route.json output carries D-029 §八 schema with station_id and file_path``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

The file lives at
``<workspace_root>/codebus-tutorials/{task_id}/route.json``.

Top-level shape:
    {
        "title": ...,
        "task": ...,
        "source_type": "folder" | "topic",
        "source_path": ...,
        "estimated_minutes": <sum of station durations>,
        "generated_at": ISO-8601,
        "stations": [...]
        # OPTIONAL "degraded": true — appears only when every station is degraded
    }

Each station entry preserves the field order documented in the spec
Requirement (station_id / index / title / duration / file_path /
prerequisites / related_files / related_stations / required_checks /
degraded). ``prerequisites`` stays empty in P0 — Decision 2 defers
``Station.depends_on`` backfill to a follow-up change.
"""
from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Literal

from .types import RouteStation

__all__ = ["write_route_json"]


def write_route_json(
    *,
    title: str,
    task: str,
    source_type: Literal["folder", "topic"],
    source_path: str,
    generated_at: datetime,
    stations: list[RouteStation],
    output_path: Path,
) -> None:
    """Render and write the ``route.json`` file.

    ``output_path`` is created with ``parents=True, exist_ok=True``.
    The output JSON is pretty-printed with 2-space indent + UTF-8 to
    keep human-readable diffs friendly.
    """
    output_path.parent.mkdir(parents=True, exist_ok=True)

    estimated_minutes = sum(int(s.duration) for s in stations)
    payload: dict = {
        "title": title,
        "task": task,
        "source_type": source_type,
        "source_path": source_path,
        "estimated_minutes": estimated_minutes,
        "generated_at": generated_at.isoformat(),
        "stations": [_serialize_station(s) for s in stations],
    }

    if stations and all(s.degraded for s in stations):
        payload["degraded"] = True

    output_path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


def _serialize_station(s: RouteStation) -> dict:
    """Serialize one ``RouteStation`` keeping the spec's field order.

    ``error`` is included only when populated; per-spec the field is
    optional and only meaningful for the ``write_failed`` degraded path.
    """
    out: dict = {
        "station_id": s.station_id,
        "index": int(s.index),
        "title": s.title,
        "duration": int(s.duration),
        "file_path": s.file_path,
        "prerequisites": list(s.prerequisites),
        "related_files": list(s.related_files),
        "related_stations": list(s.related_stations),
        "required_checks": list(s.required_checks),
        "degraded": bool(s.degraded),
    }
    if s.error:
        out["error"] = s.error
    return out
