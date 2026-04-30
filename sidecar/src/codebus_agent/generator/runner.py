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
from codebus_agent.sanitizer import (
    RULES_VERSION as _DEFAULT_RULES_VERSION,
    SanitizerAuditLogger,
    SanitizerEngine,
)

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

__all__ = ["derive_station_ids", "run_generator"]


def derive_station_ids(stations: list[Station]) -> list[str]:
    """Re-implement the runner's stable_id allocation as a pure function.

    Used by ``POST /generate`` to pre-flight ``target_stations`` against
    the ids the runner would derive from ``state.stations`` (without
    spawning a background task). Mirrors the loop in ``run_generator``
    that calls ``generate_station_id(idx, title, existing_ids)``.
    """
    existing: set[str] = set()
    out: list[str] = []
    for idx, station in enumerate(stations, start=1):
        title = _derive_station_title(station)
        sid = generate_station_id(idx, title, existing)
        existing.add(sid)
        out.append(sid)
    return out


# Module-level constant per spec Requirement
# `Output root directory is workspace/codebus-tutorials per task`:
# all generator product output anchors here so callers cannot drift
# into `<ws>/.codebus/` (audit-only) or generic `<ws>/tutorials/`
# (collision risk with user-existing folders).
_TUTORIALS_DIRNAME: str = "codebus-tutorials"

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
    target_stations: list[str] | None = None,
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

    # Partial-regen branch (per spec ADDED Requirement
    # `Partial regen via target_stations preserves unrelated stations`).
    # Diverges from the full path before any MOC / route writes happen.
    if target_stations is not None and len(target_stations) > 0:
        return await _run_partial_regen(
            target_stations=target_stations,
            station_assignments=station_assignments,
            workspace_root=workspace_root,
            task_id=task_id,
            tutorial_dir=tutorial_dir,
            stations_dir=stations_dir,
            provider=provider,
            sanitizer=sanitizer,
            sanitizer_audit=sanitizer_audit,
            rules_version=rules_version,
            log=log,
            repo_name=repo_name,
            workspace_type=workspace_type,
            duration_minutes_per_station=duration_minutes_per_station,
            generated_at=generated_at,
            options=options,
            state=state,
            emitter=emitter,
        )

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


async def _run_partial_regen(
    *,
    target_stations: list[str],
    station_assignments: list[tuple[Station, int, str, str]],
    workspace_root: Path,
    task_id: str,
    tutorial_dir: Path,
    stations_dir: Path,
    provider: Any,
    sanitizer: SanitizerEngine,
    sanitizer_audit: SanitizerAuditLogger,
    rules_version: str,
    log: GeneratorLogger,
    repo_name: str,
    workspace_type: str,
    duration_minutes_per_station: int,
    generated_at: datetime,
    options: GeneratorOptions,
    state: ExplorerState,
    emitter: Any | None,
) -> GeneratorResult:
    """Execute the partial-regen path per spec ADDED Requirement.

    Iterates ``target_stations`` in request order. For each requested
    id:
      1. Locate matching ``Station`` via the pre-allocated assignments;
         on no match, treat as drift (LLM might want a new title), log
         and continue.
      2. Run ``_generate_station`` with normal context-building rules.
      3. Verify the resulting stable id matches the request; on drift,
         log + leave file untouched (continue with remaining ids).
      4. Sanitizer Pass 1 already runs inside ``generate_station``;
         no extra pass needed at runner level.
      5. ``generate_station`` writes the file directly to
         ``stations/{stable_id}.md`` so partial mode does not need a
         separate write step.

    MOC and ``route.json`` are NOT touched. ``GeneratorResult`` returns
    paths pointing at the existing on-disk MOC / route locations.
    """
    # Build a lookup from derived stable id → assignment so we can
    # resolve requested ids back to (station, idx, title). This is the
    # canonical pre-allocation order from `run_generator`.
    by_id: dict[str, tuple[Station, int, str, str]] = {
        sid: tup for tup, sid in zip(station_assignments, [sa[2] for sa in station_assignments])
    }

    log.append(
        event="run_started",
        task_id=task_id,
        task=state.task,
        station_count=len(target_stations),
        run_mode="partial",
    )

    regenerated_paths: list[Path] = []
    degraded_count = 0
    total_stations = len(target_stations)

    for partial_idx, requested_id in enumerate(target_stations, start=1):
        match = by_id.get(requested_id)
        if match is None:
            # Drift: requested id does not exist in the pre-allocated
            # set. We still produce an "observed" id by deriving from
            # the closest-matching station-by-prefix if any (best-effort
            # for the audit row); otherwise observed is the empty string.
            prefix = requested_id.split("-", 1)[0] if "-" in requested_id else ""
            observed = next(
                (sa[2] for sa in station_assignments if sa[2].startswith(prefix + "-")),
                "",
            )
            log.append(
                event="station_id_drift",
                task_id=task_id,
                requested_station_id=requested_id,
                observed_station_id=observed,
                run_mode="partial",
            )
            continue

        station, idx, station_id, station_title = match
        if emitter is not None:
            emitter.emit(
                {
                    "type": "progress",
                    "phase": "generating",
                    "current_station": partial_idx,
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
            previous_stations_summary="",
            related_files_excerpt="",
            kb_hits_excerpt="",
            max_retries=3,
            emitter=emitter,
            total_stations=total_stations,
        )
        outcome = await generate_station(station=station, ctx=ctx)
        regenerated_paths.append(outcome.station_path)
        if outcome.degraded:
            degraded_count += 1
        log.append(
            event="station_partial_regenerated",
            task_id=task_id,
            station_id=station_id,
            mode="partial",
            degraded=outcome.degraded,
        )

    log.append(
        event="run_completed",
        task_id=task_id,
        station_count=len(regenerated_paths),
        degraded_count=degraded_count,
        run_mode="partial",
    )

    return GeneratorResult(
        tutorial_path=tutorial_dir / "tutorial.md",
        station_paths=regenerated_paths,
        route_path=tutorial_dir / "route.json",
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
