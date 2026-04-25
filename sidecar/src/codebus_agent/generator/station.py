"""Per-station LLM call + retry + Sanitizer Pass 1 + write file.

Backs Requirements in
`openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`:
  - ``Generator entrypoint orchestrates per-station markdown pipeline`` (per-station path)
  - ``Degraded fallback writes per-station stub after retry exhaustion``
  - ``Generator output passes Sanitizer Pass 1 before disk write``

The orchestrator (``runner.py``) calls ``generate_station`` once per
``Station`` in the ``ExplorerState``. Per-station failure produces a
degraded stub but does NOT abort the whole run — D-029 §十六.2
multi-file isolation invariant.
"""
from __future__ import annotations

import logging
import uuid
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any, Literal

from codebus_agent.agent.types import Station
from codebus_agent.providers.protocol import Message
from codebus_agent.sanitizer import (
    FileSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)

from .frontmatter import render_frontmatter
from .log import GeneratorLogger
from .prompts import (
    STATION_PROMPT_VERSION,
    STATION_SYSTEM_INTERACTIVE,
    STATION_SYSTEM_PLAIN,
    render_station_prompt,
)
from .types import Frontmatter, StationMarkdown
from .validator import validate_station_markdown

__all__ = [
    "StationContext",
    "StationOutcome",
    "generate_station",
    "_make_degraded_stub",
]


logger = logging.getLogger(__name__)


@dataclass
class StationContext:
    """All non-station inputs ``generate_station`` needs to do its job.

    Bundled into a dataclass so the function signature stays short and
    so the orchestrator can build it once per station with a few
    diff'd fields (station_index / station_id / title / output_path).
    """

    workspace_root: Path
    output_path: Path

    provider: Any  # TrackedProvider | LLMProvider — duck-typed for testability
    sanitizer: SanitizerEngine
    sanitizer_audit: SanitizerAuditLogger
    rules_version: str
    log: GeneratorLogger

    station_index: int
    station_id: str
    station_title: str
    task: str
    repo_name: str
    workspace_type: Literal["folder", "topic"]
    generated_at: datetime
    duration_minutes: int = 15
    related_files: list[str] = field(default_factory=list)
    related_stations: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)

    mode: Literal["interactive", "plain"] = "interactive"
    target_persona: str = "experienced engineer"
    previous_stations_summary: str = ""
    related_files_excerpt: str = ""
    kb_hits_excerpt: str = ""

    max_retries: int = 3

    # SSE wiring (Section 14) — defaults preserve no-op behaviour.
    emitter: Any | None = None
    total_stations: int = 1


@dataclass
class StationOutcome:
    """Per-station terminal result the orchestrator consumes for route.json."""

    station_path: Path
    degraded: bool
    error: str | None = None
    required_checks: list[str] = field(default_factory=list)
    attempts: int = 1


async def generate_station(
    *,
    station: Station,
    ctx: StationContext,
) -> StationOutcome:
    """Drive one station through LLM call → validator → retry → sanitize → write.

    Per spec Requirement
    ``Generator entrypoint orchestrates per-station markdown pipeline``:

    1. (id assignment happens in the runner; ``ctx.station_id`` is pre-set)
    2. (context fields filled by caller — ``related_files_excerpt`` etc.)
    3. ``provider.chat(messages, response_model=StationMarkdown)`` per attempt
    4. Validator pipeline
    5. Up to ``ctx.max_retries`` retries; previous attempt's issues feed
       the next attempt's prompt as ``correction_hint``
    6. After exhaustion: degraded stub via ``_make_degraded_stub``
    7. Sanitizer Pass 1 over the rendered content before disk write
    8. Frontmatter prepended; whole content written
    9. Output path: ``<workspace>/codebus-tutorials/{task_id}/stations/s{NN}-{slug}.md``
    """
    body, required_checks, degraded, attempts = await _attempt_loop(
        station=station, ctx=ctx
    )

    fm = _build_frontmatter(ctx, required_checks=required_checks, degraded=degraded)
    rendered = render_frontmatter(fm) + body

    _emit(
        ctx,
        status="writing_file",
        file_path=str(ctx.output_path),
    )

    # Sanitizer Pass 1 over output (Decision 1 — defense in depth).
    session_id = str(uuid.uuid4())
    sanitized = ctx.sanitizer.sanitize(
        rendered,
        source=FileSource(
            pass_="generator",
            path=str(ctx.output_path),
        ),
    )
    for entry in sanitized.entries:
        ctx.sanitizer_audit.append(
            entry=entry,
            pass_num=1,
            rules_version=ctx.rules_version,
            session_id=session_id,
        )

    # Disk write — single attempt only. OSError → log + flagged degraded.
    try:
        ctx.output_path.parent.mkdir(parents=True, exist_ok=True)
        ctx.output_path.write_text(sanitized.text, encoding="utf-8")
    except OSError as exc:
        logger.exception(
            "generator: write failed for station %s (%s)",
            ctx.station_id,
            type(exc).__name__,
        )
        ctx.log.append(
            event="write_failed",
            station_id=ctx.station_id,
            station_index=ctx.station_index,
            error=f"{type(exc).__name__}: {exc}",
            prompt_version=STATION_PROMPT_VERSION,
        )
        return StationOutcome(
            station_path=ctx.output_path,
            degraded=True,
            error="write_failed",
            required_checks=required_checks,
            attempts=attempts,
        )

    return StationOutcome(
        station_path=ctx.output_path,
        degraded=degraded,
        required_checks=required_checks,
        attempts=attempts,
    )


async def _attempt_loop(
    *,
    station: Station,
    ctx: StationContext,
) -> tuple[str, list[str], bool, int]:
    """Run the LLM call ↔ validator ↔ retry loop.

    Returns ``(body, required_checks, degraded, attempts)``.
    """
    correction_hint = ""
    last_issues: list[str] = []

    system_prompt = (
        STATION_SYSTEM_INTERACTIVE
        if ctx.mode == "interactive"
        else STATION_SYSTEM_PLAIN
    )

    for attempt in range(1, ctx.max_retries + 1):
        user_prompt = render_station_prompt(
            mode=ctx.mode,
            target_persona=ctx.target_persona,
            station_title=ctx.station_title,
            station_index=ctx.station_index,
            task=ctx.task,
            related_files_excerpt=ctx.related_files_excerpt,
            kb_hits_excerpt=ctx.kb_hits_excerpt,
            previous_stations_summary=ctx.previous_stations_summary,
            correction_hint=correction_hint,
        )
        messages = [
            Message(role="system", content=system_prompt),
            Message(role="user", content=user_prompt),
        ]
        try:
            result = await ctx.provider.chat(
                messages, response_model=StationMarkdown
            )
        except Exception as exc:
            # Treat transient errors like a validator rejection: feed
            # into the correction hint and retry. The wrapping endpoint
            # surface (POST /generate) maps unrecoverable errors via
            # `_run_background_task` `GENERATE_FAILED`.
            logger.warning(
                "generator: provider.chat raised on station %s attempt %d: %s",
                ctx.station_id,
                attempt,
                type(exc).__name__,
            )
            last_issues = [f"provider_error: {type(exc).__name__}"]
            correction_hint = ", ".join(last_issues)
            continue

        body = result.body
        _emit(ctx, status="validating")
        validation = validate_station_markdown(
            body,
            station_idx=ctx.station_index,
            mode=ctx.mode,
            workspace_root=ctx.workspace_root,
        )
        if not validation.issues:
            required_checks = list(validation.parsed.get("required_checks", []))
            return body, required_checks, False, attempt
        last_issues = validation.issues
        correction_hint = ", ".join(validation.issues)
        if attempt < ctx.max_retries:
            _emit(ctx, status="retry", attempt=attempt + 1)

    # Retry budget exhausted — degraded stub.
    ctx.log.append(
        event="degraded",
        station_id=ctx.station_id,
        station_index=ctx.station_index,
        attempts=ctx.max_retries,
        last_issues=last_issues,
        prompt_version=STATION_PROMPT_VERSION,
    )
    body = _make_degraded_stub(
        station_id=ctx.station_id,
        station_index=ctx.station_index,
        station_title=ctx.station_title,
    )
    required_checks = [f"station-{ctx.station_index}-check"]
    return body, required_checks, True, ctx.max_retries


def _make_degraded_stub(
    *,
    station_id: str,
    station_index: int,
    station_title: str,
) -> str:
    """Per spec Requirement
    ``Degraded fallback writes per-station stub after retry exhaustion``:

    1. H1 heading with the station title
    2. A single paragraph telling the user what happened
    3. Exactly one ``<Checkpoint id="station-{idx}-check">`` with one item
    4. NO Quiz / CodeRef / Reveal elements
    """
    return (
        f"# {station_title}\n\n"
        f"本站此次生成失敗，其他站不受影響；請從工具列重新跑一次。\n\n"
        f"<Checkpoint id=\"station-{station_index}-check\">\n"
        f"- [ ] 本站需要重新生成\n"
        f"</Checkpoint>\n"
    )


def _emit(
    ctx: StationContext,
    *,
    status: str,
    phase: str = "generating",
    **extra: Any,
) -> None:
    """Fan out a `progress` event when an emitter is wired.

    Per spec Requirement
    `SSE generating events stream per-station progress`: every event
    carries `current_station` (1-based; 0 for assembling_moc phase),
    `total_stations` (snapshot at run start), `status`, and
    `station_id`. ``file_path`` / ``attempt`` ride along for the
    statuses that include them.
    """
    if ctx.emitter is None:
        return
    payload: dict[str, Any] = {
        "type": "progress",
        "phase": phase,
        "current_station": ctx.station_index,
        "total_stations": ctx.total_stations,
        "status": status,
        "station_id": ctx.station_id,
    }
    payload.update(extra)
    ctx.emitter.emit(payload)


def _build_frontmatter(
    ctx: StationContext,
    *,
    required_checks: list[str],
    degraded: bool,
) -> Frontmatter:
    return Frontmatter(
        schema_version=1,
        station_id=ctx.station_id,
        station_index=ctx.station_index,
        title=ctx.station_title,
        duration_minutes=ctx.duration_minutes,
        workspace_type=ctx.workspace_type,
        repo_name=ctx.repo_name,
        task=ctx.task,
        generated_at=ctx.generated_at,
        required_checks=required_checks,
        degraded=degraded,
        tags=list(ctx.tags) if ctx.tags else None,
        related_stations=list(ctx.related_stations) if ctx.related_stations else None,
        related_files=list(ctx.related_files) if ctx.related_files else None,
    )
