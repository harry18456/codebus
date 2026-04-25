"""Generator Pydantic schemas.

Backs Requirements in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`:
  - ``Generator entrypoint orchestrates per-station markdown pipeline`` (``GeneratorResult``)
  - ``Frontmatter renderer produces D-029 schema_version 1 YAML`` (``Frontmatter``)
  - ``Markdown validator enforces D-029 component rules`` (``ValidationResult``)
  - ``route.json output carries D-029 §八 schema with station_id and file_path`` (``RouteStation``)
"""
from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field


__all__ = [
    "Frontmatter",
    "GeneratorOptions",
    "GeneratorResult",
    "RouteStation",
    "StationMarkdown",
    "StationSummary",
    "ValidationResult",
]


class StationMarkdown(BaseModel):
    """Instructor ``response_model`` shape for per-station LLM call.

    ``body`` carries the markdown text (frontmatter is added by the
    post-processing step, NOT by the LLM). ``thought`` records the
    LLM's reasoning so the reasoning log captures intent. ``notes``
    holds any extra context the LLM volunteered (optional).
    """

    thought: str
    body: str
    notes: str | None = None


class Frontmatter(BaseModel):
    """D-029 §7.3 frontmatter schema — 11 required + 3 optional fields.

    Field order is fixed (matches the spec Requirement scenario
    ``Required fields rendered in order``). Optional list fields are
    rendered only when populated; ``None`` / ``[]`` is omitted.
    """

    schema_version: int = 1
    station_id: str
    station_index: int
    title: str
    duration_minutes: int
    workspace_type: Literal["folder", "topic"]
    repo_name: str
    task: str
    generated_at: datetime
    required_checks: list[str] = Field(default_factory=list)
    degraded: bool = False
    tags: list[str] | None = None
    related_stations: list[str] | None = None
    related_files: list[str] | None = None


class ValidationResult(BaseModel):
    """Markdown validator output.

    ``issues`` empty means the markdown passes; ``parsed`` carries
    structured fields downstream consumers (e.g., frontmatter
    ``required_checks``) lift from the markdown.
    """

    issues: list[str] = Field(default_factory=list)
    parsed: dict[str, Any] = Field(default_factory=dict)


class RouteStation(BaseModel):
    """One entry inside ``route.json`` ``stations`` array.

    Field order matches D-029 §八 schema. ``prerequisites`` is empty
    in P0 (Decision 2: ``Station.depends_on`` backfill deferred).
    ``error`` populated only when disk-write failed (degraded path).
    """

    station_id: str
    index: int
    title: str
    duration: int
    file_path: str
    prerequisites: list[str] = Field(default_factory=list)
    related_files: list[str] = Field(default_factory=list)
    related_stations: list[str] = Field(default_factory=list)
    required_checks: list[str] = Field(default_factory=list)
    degraded: bool = False
    error: str | None = None


class StationSummary(BaseModel):
    """Lightweight station descriptor consumed by the MOC assembler.

    The MOC needs only the stable id + display title + duration to
    render its index list; full station bodies stay in their own files.
    """

    station_id: str
    title: str
    duration: int


class GeneratorOptions(BaseModel):
    """Caller-supplied generation options.

    ``mode`` selects between interactive (custom components) and plain
    (pure GitHub-renderable markdown) flavors per spec §六.
    ``target_persona`` is fed into the system prompt as a tone hint.
    """

    mode: Literal["interactive", "plain"] = "interactive"
    target_persona: str = "experienced engineer"


class GeneratorResult(BaseModel):
    """Terminal output of ``run_generator``.

    Paths are absolute. ``degraded_count`` lets callers (the SSE wire,
    front-end) decide whether to surface a "教材品質可能不佳" warning.
    """

    model_config = ConfigDict(arbitrary_types_allowed=True)

    tutorial_path: Path
    station_paths: list[Path] = Field(default_factory=list)
    route_path: Path
    log_path: Path
    degraded_count: int = 0
