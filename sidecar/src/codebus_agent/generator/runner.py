"""Generator orchestrator — ``run_generator`` entrypoint.

Backs Requirement
``Generator entrypoint orchestrates per-station markdown pipeline``
(orchestrator path) and
``Output root directory is workspace/codebus-tutorials per task``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

The orchestrator iterates ``state.stations`` in order, allocating
stable ids, building per-station context, calling
``generate_station`` once per station, then assembling MOC + route.json
after the loop. Per-station failure produces a degraded stub but does
NOT abort the run (D-029 §十六.2 multi-file isolation invariant).
"""
from __future__ import annotations

import logging
from collections.abc import Callable
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from codebus_agent._audit_paths import (
    _GENERATOR_LOG_FILENAME,
    _SANITIZE_AUDIT_FILENAME,
    _WORKSPACE_AUDIT_SUBDIR,
)
from codebus_agent.agent.types import ExplorerState, Station
from codebus_agent.sanitizer import SanitizerAuditLogger, SanitizerEngine

from .log import GeneratorLogger
from .moc import assemble_moc
from .route import write_route_json
from .stable_id import generate_station_id
from .station import StationContext, StationOutcome, generate_station
from .types import (
    GeneratorOptions,
    GeneratorResult,
    RouteStation,
    StationSummary,
)

__all__ = ["run_generator"]


# Module-level constant per spec Requirement
# `Output root directory is workspace/codebus-tutorials per task`:
# all generator product output anchors here so callers cannot drift
# into `<ws>/.codebus/` (audit-only) or generic `<ws>/tutorials/`
# (collision risk with user-existing folders).
_TUTORIALS_DIRNAME: str = "codebus-tutorials"

_DEFAULT_RULES_VERSION: str = "2026-04-20-1"
_DEFAULT_DURATION_MINUTES: int = 15

logger = logging.getLogger(__name__)


async def run_generator(
    *,
    state: ExplorerState,
    workspace_root: Path,
    task_id: str,
    llm_chat_provider: Callable[[Path], Any],
    kb: Any | None = None,
    options: GeneratorOptions | None = None,
    sanitizer: SanitizerEngine | None = None,
    sanitizer_audit: SanitizerAuditLogger | None = None,
    rules_version: str = _DEFAULT_RULES_VERSION,
    log: GeneratorLogger | None = None,
    repo_name: str | None = None,
    workspace_type: str = "folder",
    duration_minutes_per_station: int = _DEFAULT_DURATION_MINUTES,
    title: str | None = None,
    emitter: Any | None = None,
) -> GeneratorResult:
    """Orchestrate the full per-station pipeline + MOC + route.json.

    Per spec Requirement: iterate ``state.stations`` in order,
    invoke ``generate_station`` for each, then assemble MOC and write
    ``route.json`` after the loop. ``run_generator`` MUST NOT
    short-circuit on per-station failure.

    Wire defaults:
      * ``sanitizer`` → fresh ``SanitizerEngine()``
      * ``sanitizer_audit`` → ``<ws>/.codebus/sanitize_audit.jsonl``
      * ``log`` → ``<ws>/.codebus/generator_log.jsonl``
      * ``repo_name`` → ``workspace_root.name``
      * ``title`` → ``state.task``
    """
    options = options or GeneratorOptions()
    sanitizer = sanitizer or SanitizerEngine()
    audit_dir = workspace_root / _WORKSPACE_AUDIT_SUBDIR
    audit_dir.mkdir(parents=True, exist_ok=True)
    sanitizer_audit = sanitizer_audit or SanitizerAuditLogger(
        audit_dir / _SANITIZE_AUDIT_FILENAME
    )
    log = log or GeneratorLogger(audit_dir / _GENERATOR_LOG_FILENAME)
    repo_name = repo_name or workspace_root.name
    title = title or state.task

    tutorial_dir = workspace_root / _TUTORIALS_DIRNAME / task_id
    stations_dir = tutorial_dir / "stations"
    stations_dir.mkdir(parents=True, exist_ok=True)

    # Emit a `run_started` line so `generator_log.jsonl` always exists
    # after a run, even when every station succeeds first attempt and
    # no degraded / write_failed events fire (per spec scenario
    # `Generator does not write to .codebus subdirectory except
    # generator_log.jsonl`).
    log.append(
        event="run_started",
        task_id=task_id,
        task=state.task,
        station_count=len(state.stations),
        mode=options.mode,
    )

    # Build provider once per workspace — TrackedProvider's running
    # token / cost counters scope to this single instance for the
    # duration of the run (mirrors `LLMJudge`-style wiring in Module 4).
    provider = llm_chat_provider(workspace_root)

    generated_at = datetime.now(timezone.utc)

    # Pre-allocate stable ids in a single pass so collision handling
    # sees every prior station's id deterministically.
    existing_ids: set[str] = set()
    station_assignments: list[tuple[Station, int, str, str]] = []
    for idx, station in enumerate(state.stations, start=1):
        station_title = _derive_station_title(station)
        station_id = generate_station_id(idx, station_title, existing_ids)
        existing_ids.add(station_id)
        station_assignments.append((station, idx, station_id, station_title))

    outcomes: list[StationOutcome] = []
    summaries: list[StationSummary] = []
    previous_summaries: list[str] = []

    total_stations = len(state.stations)
    for station, idx, station_id, station_title in station_assignments:
        if emitter is not None:
            emitter.emit(
                {
                    "type": "progress",
                    "phase": "generating",
                    "current_station": idx,
                    "total_stations": total_stations,
                    "status": "generating",
                    "station_id": station_id,
                }
            )
        ctx = StationContext(
            workspace_root=workspace_root,
            output_path=stations_dir / f"{station_id}.md",
            provider=provider,
            sanitizer=sanitizer,
            sanitizer_audit=sanitizer_audit,
            rules_version=rules_version,
            log=log,
            station_index=idx,
            station_id=station_id,
            station_title=station_title,
            task=state.task,
            repo_name=repo_name,
            workspace_type=workspace_type,  # type: ignore[arg-type]
            generated_at=generated_at,
            duration_minutes=duration_minutes_per_station,
            related_files=[station.path] if station.path else [],
            related_stations=[],
            tags=[],
            mode=options.mode,
            target_persona=options.target_persona,
            previous_stations_summary="\n".join(previous_summaries),
            related_files_excerpt="",
            kb_hits_excerpt="",
            max_retries=3,
            emitter=emitter,
            total_stations=total_stations,
        )
        outcome = await generate_station(station=station, ctx=ctx)
        outcomes.append(outcome)
        summaries.append(
            StationSummary(
                station_id=station_id,
                title=station_title,
                duration=duration_minutes_per_station,
            )
        )
        previous_summaries.append(f"- {station_id}: {station_title}")

    # Assemble MOC (tutorial.md) once after all stations are written.
    tutorial_path = tutorial_dir / "tutorial.md"
    if emitter is not None:
        emitter.emit(
            {
                "type": "progress",
                "phase": "assembling_moc",
                "current_station": 0,
                "total_stations": total_stations,
                "status": "writing_file",
                "station_id": "",
                "file_path": str(tutorial_path),
            }
        )
    assemble_moc(
        task=state.task,
        total_minutes=sum(s.duration for s in summaries),
        generated_at=generated_at,
        workspace_name=repo_name,
        station_summaries=summaries,
        mode=options.mode,
        output_path=tutorial_path,
    )

    # Write route.json once after MOC.
    route_path = tutorial_dir / "route.json"
    route_stations = [
        RouteStation(
            station_id=summary.station_id,
            index=idx + 1,
            title=summary.title,
            duration=summary.duration,
            file_path=f"stations/{summary.station_id}.md",
            prerequisites=[],
            related_files=[
                station.path
            ]
            if station.path
            else [],
            related_stations=[],
            required_checks=outcome.required_checks,
            degraded=outcome.degraded,
            error=outcome.error,
        )
        for idx, (summary, outcome, station) in enumerate(
            zip(
                summaries,
                outcomes,
                [s for s, *_ in station_assignments],
            )
        )
    ]
    if emitter is not None:
        emitter.emit(
            {
                "type": "progress",
                "phase": "assembling_moc",
                "current_station": 0,
                "total_stations": total_stations,
                "status": "writing_file",
                "station_id": "",
                "file_path": str(route_path),
            }
        )
    write_route_json(
        title=title,
        task=state.task,
        source_type="folder" if workspace_type == "folder" else "topic",
        source_path=str(workspace_root),
        generated_at=generated_at,
        stations=route_stations,
        output_path=route_path,
    )

    degraded_count = sum(1 for o in outcomes if o.degraded)

    log.append(
        event="run_completed",
        task_id=task_id,
        station_count=len(state.stations),
        degraded_count=degraded_count,
    )

    return GeneratorResult(
        tutorial_path=tutorial_path,
        station_paths=[o.station_path for o in outcomes],
        route_path=route_path,
        log_path=log.path,
        degraded_count=degraded_count,
    )


def _derive_station_title(station: Station) -> str:
    """Cheap deterministic station title from the file path.

    P0 keeps this minimal — the LLM elaborates the prose body. Picking
    a consistent shape from the path means the MOC's display name and
    the stable id slug stay aligned.
    """
    if not station.path:
        return "Station"
    stem = Path(station.path).stem or "Station"
    cleaned = stem.replace("_", " ").replace("-", " ").strip()
    return cleaned.title() if cleaned else "Station"
