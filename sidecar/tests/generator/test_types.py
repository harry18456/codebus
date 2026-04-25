"""Round-trip tests for Generator Pydantic schemas.

Backs Requirements that produce / consume these schemas in
`openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Each schema must serialize via ``model_dump_json`` and deserialize via
``model_validate_json`` without data loss — the same contract
``reasoning_log.jsonl`` replay relies on for the agent-core schemas.
"""
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from codebus_agent.generator.types import (
    Frontmatter,
    GeneratorResult,
    RouteStation,
    StationMarkdown,
    StationSummary,
    ValidationResult,
)


def test_station_markdown_round_trip() -> None:
    original = StationMarkdown(
        thought="explain storage interface contract",
        body="# Storage\n\nThis station covers ...",
        notes="related: s03-adapter",
    )
    restored = StationMarkdown.model_validate_json(original.model_dump_json())
    assert restored == original


def test_frontmatter_round_trip() -> None:
    original = Frontmatter(
        schema_version=1,
        station_id="s02-storage",
        station_index=2,
        title="Storage",
        duration_minutes=15,
        workspace_type="folder",
        repo_name="timeline",
        task="add gdrive adapter",
        generated_at=datetime(2026, 4, 25, 10, 30, 0, tzinfo=timezone.utc),
        required_checks=["station-2-check"],
        degraded=False,
        tags=["architecture"],
        related_stations=["s01-overview"],
        related_files=["src/storage.ts"],
    )
    restored = Frontmatter.model_validate_json(original.model_dump_json())
    assert restored == original


def test_validation_result_round_trip() -> None:
    original = ValidationResult(
        issues=["too_long", "missing_checkpoint"],
        parsed={"required_checks": ["station-2-check"]},
    )
    restored = ValidationResult.model_validate_json(original.model_dump_json())
    assert restored == original


def test_route_station_round_trip() -> None:
    original = RouteStation(
        station_id="s02-storage",
        index=2,
        title="Storage",
        duration=15,
        file_path="stations/s02-storage.md",
        prerequisites=[],
        related_files=["src/storage.ts"],
        related_stations=["s01-overview"],
        required_checks=["station-2-check"],
        degraded=False,
        error=None,
    )
    restored = RouteStation.model_validate_json(original.model_dump_json())
    assert restored == original


def test_station_summary_round_trip() -> None:
    original = StationSummary(station_id="s01-overview", title="Repo Overview", duration=10)
    restored = StationSummary.model_validate_json(original.model_dump_json())
    assert restored == original


def test_generator_result_round_trip() -> None:
    original = GeneratorResult(
        tutorial_path=Path("/tmp/ws/codebus-tutorials/generate_abcd1234/tutorial.md"),
        station_paths=[
            Path("/tmp/ws/codebus-tutorials/generate_abcd1234/stations/s01-overview.md"),
            Path("/tmp/ws/codebus-tutorials/generate_abcd1234/stations/s02-storage.md"),
        ],
        route_path=Path("/tmp/ws/codebus-tutorials/generate_abcd1234/route.json"),
        log_path=Path("/tmp/ws/.codebus/generator_log.jsonl"),
        degraded_count=0,
    )
    restored = GeneratorResult.model_validate_json(original.model_dump_json())
    # Path comparisons normalize separators on Windows; compare via string.
    assert restored.degraded_count == original.degraded_count
    assert str(restored.tutorial_path) == str(original.tutorial_path)
    assert [str(p) for p in restored.station_paths] == [
        str(p) for p in original.station_paths
    ]
    assert str(restored.route_path) == str(original.route_path)
    assert str(restored.log_path) == str(original.log_path)
