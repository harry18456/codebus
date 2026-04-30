"""Tests for ``run_generator(target_stations=...)`` partial-regen path.

Backs spec ADDED Requirement
``Partial regen via target_stations preserves unrelated stations`` in
``openspec/changes/phase6-step29-intervention-points/specs/module-5-generator/spec.md``.

Invariants under test:
- Only stations whose stable id is in ``target_stations`` are
  regenerated; on-disk content for unrelated stations is byte-identical
  before and after.
- ``tutorial.md`` (MOC) and ``route.json`` MUST be byte-identical
  across a partial run (the assembler / route writer MUST NOT run).
- ``GeneratorResult.station_paths`` lists only the regenerated files,
  in the order requested.
- ``generator_log.jsonl`` gains a ``mode="partial"`` row per regen.
- Station id drift (LLM-generated stable id != requested id) MUST be
  rejected with ``GENERATE_STATION_ID_DRIFT``; the on-disk file stays
  byte-identical and the runner continues with remaining ids.
"""
from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path

import pytest

from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.generator.runner import run_generator
from codebus_agent.generator.types import GeneratorOptions, StationMarkdown
from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


_TASK_ID = "generate_partial1"


def _good_body(idx: int, title: str) -> str:
    return (
        f"# {title}\n\n"
        f"Body for {title}.\n\n"
        f"<Checkpoint id=\"station-{idx}-check\">\n- [ ] {title} 對齊\n</Checkpoint>\n"
    )


def _three_stations() -> list[Station]:
    """Stations with deterministic file_paths so stable_id resolves to
    s01-overview / s02-mqtt-client / s03-storage."""
    return [
        Station(path=f"src/{name}.ts", role="interface", relevance=0.5, why=".")
        for name in ["overview", "mqtt-client", "storage"]
    ]


def _three_stable_titles() -> list[str]:
    # _derive_station_title turns "src/overview.ts" stem → "Overview" etc.
    return ["Overview", "Mqtt Client", "Storage"]


async def _do_full_run(
    tmp_path: Path,
    script: MockScript,
    factory: Callable[[Path], TrackedProvider],
):
    for idx, title in enumerate(_three_stable_titles(), start=1):
        script.push(StationMarkdown(thought="ok", body=_good_body(idx, title)))
    state = ExplorerState(
        task="walk through the storage adapter",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    return await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=factory,
        options=GeneratorOptions(mode="interactive"),
    )


@pytest.mark.asyncio
async def test_partial_regen_single_station_overwrites_only_target(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    """Single target station overwrites only its own file; everything else byte-identical."""
    full = await _do_full_run(
        tmp_path, mock_script_generate, mock_generate_provider_factory
    )
    s01_before = full.station_paths[0].read_bytes()
    s02_before = full.station_paths[1].read_bytes()
    s03_before = full.station_paths[2].read_bytes()
    moc_before = full.tutorial_path.read_bytes()
    route_before = full.route_path.read_bytes()

    # Push a new body for the partial regen — title intentionally same
    # so stable_id resolves to s02-mqtt-client (matches the request).
    mock_script_generate.push(
        StationMarkdown(thought="ok-partial", body=_good_body(2, "Mqtt Client"))
    )
    state = ExplorerState(
        task="walk through the storage adapter",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    result = await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
        target_stations=["s02-mqtt-client"],
    )

    # Only s02 file changed; others byte-identical
    assert full.station_paths[0].read_bytes() == s01_before
    assert full.station_paths[1].read_bytes() != s02_before
    assert full.station_paths[2].read_bytes() == s03_before
    # MOC + route untouched
    assert full.tutorial_path.read_bytes() == moc_before
    assert full.route_path.read_bytes() == route_before
    # Result reflects partial scope
    assert len(result.station_paths) == 1
    assert result.station_paths[0].name == "s02-mqtt-client.md"
    # Log gained a partial row
    log_path = tmp_path / ".codebus" / "generator_log.jsonl"
    assert log_path.exists()
    rows = [
        json.loads(line)
        for line in log_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    partial_rows = [r for r in rows if r.get("mode") == "partial"]
    assert len(partial_rows) == 1
    assert partial_rows[0].get("station_id") == "s02-mqtt-client"


@pytest.mark.asyncio
async def test_partial_regen_multiple_stations_preserves_moc_and_route(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    """Two target stations both overwritten; MOC + route + unrelated station byte-identical."""
    full = await _do_full_run(
        tmp_path, mock_script_generate, mock_generate_provider_factory
    )
    s02_before = full.station_paths[1].read_bytes()
    moc_before = full.tutorial_path.read_bytes()
    route_before = full.route_path.read_bytes()

    mock_script_generate.push(
        StationMarkdown(thought="ok-p", body=_good_body(1, "Overview"))
    )
    mock_script_generate.push(
        StationMarkdown(thought="ok-p", body=_good_body(3, "Storage"))
    )
    state = ExplorerState(
        task="walk through the storage adapter",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    result = await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
        target_stations=["s01-overview", "s03-storage"],
    )

    # s02 unchanged
    assert full.station_paths[1].read_bytes() == s02_before
    # MOC + route untouched
    assert full.tutorial_path.read_bytes() == moc_before
    assert full.route_path.read_bytes() == route_before
    # Result lists in request order
    assert len(result.station_paths) == 2
    assert result.station_paths[0].name == "s01-overview.md"
    assert result.station_paths[1].name == "s03-storage.md"


@pytest.mark.asyncio
async def test_partial_regen_station_id_drift_rejected_and_continues(
    tmp_path: Path,
    mock_script_generate: MockScript,
    mock_generate_provider_factory: Callable[[Path], TrackedProvider],
) -> None:
    """If LLM-derived stable_id drifts, runner rejects and continues with remaining ids."""
    full = await _do_full_run(
        tmp_path, mock_script_generate, mock_generate_provider_factory
    )
    s02_before = full.station_paths[1].read_bytes()
    s03_before = full.station_paths[2].read_bytes()

    # The drift case is harder to trigger directly because stable_id is
    # computed from station title in the runner — but the spec invariant
    # is that runner verifies generated stable_id == requested id and
    # rejects mismatches. Simulate by passing target_stations with an
    # id that does NOT match what the runner derives from the station
    # title (the title is "Mqtt Client" → s02-mqtt-client; we request
    # "s02-something-else" — the runner SHOULD reject this id as a drift
    # since the derived id doesn't match the requested id).

    # Arrange: ask for two ids — s02-something-else (drift), s03-storage (ok)
    mock_script_generate.push(
        StationMarkdown(thought="ok-p", body=_good_body(3, "Storage"))
    )
    state = ExplorerState(
        task="walk through the storage adapter",
        budget_steps_left=0,
        budget_tokens_left=0,
        stations=_three_stations(),
    )
    result = await run_generator(
        state=state,
        workspace_root=tmp_path,
        task_id=_TASK_ID,
        llm_chat_provider=mock_generate_provider_factory,
        options=GeneratorOptions(mode="interactive"),
        target_stations=["s02-something-else", "s03-storage"],
    )

    # Drifted target left s02 untouched
    assert full.station_paths[1].read_bytes() == s02_before
    # s03 was regenerated (different bytes)
    assert full.station_paths[2].read_bytes() != s03_before
    # Result lists only the successful regen (drift skipped)
    assert len(result.station_paths) == 1
    assert result.station_paths[0].name == "s03-storage.md"

    # generator_log.jsonl recorded the drift with both requested + observed
    log_path = tmp_path / ".codebus" / "generator_log.jsonl"
    rows = [
        json.loads(line)
        for line in log_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    drift_rows = [r for r in rows if r.get("event") == "station_id_drift"]
    assert len(drift_rows) == 1
    assert drift_rows[0].get("requested_station_id") == "s02-something-else"
    # Observed id should be the one the runner WOULD have produced
    assert drift_rows[0].get("observed_station_id", "").startswith("s02-")
