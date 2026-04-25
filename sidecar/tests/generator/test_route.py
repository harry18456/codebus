"""Tests for ``route.json`` writer (Section 10)."""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

from codebus_agent.generator.route import write_route_json
from codebus_agent.generator.types import RouteStation


_TS = datetime(2026, 4, 25, 10, 30, 0, tzinfo=timezone.utc)


def _stations(*, all_degraded: bool = False) -> list[RouteStation]:
    return [
        RouteStation(
            station_id="s01-overview",
            index=1,
            title="Repo Overview",
            duration=10,
            file_path="stations/s01-overview.md",
            related_files=["README.md"],
            related_stations=["s02-storage"],
            required_checks=["station-1-check"],
            degraded=all_degraded,
        ),
        RouteStation(
            station_id="s02-storage",
            index=2,
            title="Storage",
            duration=15,
            file_path="stations/s02-storage.md",
            related_files=["src/storage.ts"],
            related_stations=["s01-overview", "s03-adapter"],
            required_checks=["station-2-check"],
            degraded=all_degraded,
        ),
        RouteStation(
            station_id="s03-adapter",
            index=3,
            title="Adapter Pattern",
            duration=20,
            file_path="stations/s03-adapter.md",
            related_files=["src/adapter.ts"],
            related_stations=["s02-storage"],
            required_checks=["station-3-check"],
            degraded=all_degraded,
        ),
    ]


def test_clean_run_emits_route_json_with_all_stations_and_no_top_level_degraded(
    tmp_path: Path,
) -> None:
    out = tmp_path / "route.json"
    write_route_json(
        title="add gdrive",
        task="add gdrive adapter",
        source_type="folder",
        source_path="/tmp/ws",
        generated_at=_TS,
        stations=_stations(),
        output_path=out,
    )
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert set(payload.keys()) == {
        "title",
        "task",
        "source_type",
        "source_path",
        "estimated_minutes",
        "generated_at",
        "stations",
    }
    assert payload["estimated_minutes"] == 45
    assert len(payload["stations"]) == 3
    for entry in payload["stations"]:
        for key in (
            "station_id",
            "index",
            "title",
            "duration",
            "file_path",
            "prerequisites",
            "related_files",
            "related_stations",
            "required_checks",
            "degraded",
        ):
            assert key in entry, f"missing {key} in {entry!r}"


def test_all_degraded_run_sets_top_level_degraded_flag(tmp_path: Path) -> None:
    out = tmp_path / "route.json"
    write_route_json(
        title="t",
        task="t",
        source_type="folder",
        source_path="/tmp/ws",
        generated_at=_TS,
        stations=_stations(all_degraded=True),
        output_path=out,
    )
    payload = json.loads(out.read_text(encoding="utf-8"))
    assert payload.get("degraded") is True


def test_file_path_uses_stations_relative_path_with_stable_id(tmp_path: Path) -> None:
    out = tmp_path / "route.json"
    write_route_json(
        title="t",
        task="t",
        source_type="folder",
        source_path="/tmp/ws",
        generated_at=_TS,
        stations=_stations(),
        output_path=out,
    )
    payload = json.loads(out.read_text(encoding="utf-8"))
    for entry in payload["stations"]:
        sid = entry["station_id"]
        assert entry["file_path"] == f"stations/{sid}.md"
        assert not entry["file_path"].startswith("./"), (
            "file_path must not start with ./ per spec scenario"
        )
