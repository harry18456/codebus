"""KB payload, hit, stats, progress, and chunk-draft Pydantic models.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: KBPayload schema
  Requirement: KBStats returned by build
  Requirement: Progress callback protocol
"""
from __future__ import annotations

import re
from collections.abc import Awaitable, Callable
from datetime import datetime
from typing import Annotated, Literal

from pydantic import AfterValidator, BaseModel, ConfigDict, Field, model_validator

SourceKind = Literal["code", "doc", "git_commit", "git_blame", "skeleton"]
AddedBy = Literal["scanner", "qa_agent"]
ProgressPhase = Literal["chunking", "embedding", "upserting", "done"]

_TEXT_HASH_RE = re.compile(r"^[0-9a-f]{64}$")
_STATION_ID_RE = re.compile(r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$")


def _validate_text_hash(value: str) -> str:
    if not isinstance(value, str) or not _TEXT_HASH_RE.fullmatch(value):
        raise ValueError(
            "text_hash must be a 64-character lowercase hex SHA-256 digest"
        )
    return value


def _validate_station_id(value: str) -> str:
    if not isinstance(value, str) or not _STATION_ID_RE.fullmatch(value):
        raise ValueError(
            f"related_stations entry {value!r} must match "
            r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$"
        )
    return value


TextHash = Annotated[str, AfterValidator(_validate_text_hash)]
StationId = Annotated[str, AfterValidator(_validate_station_id)]


class KBPayload(BaseModel):
    """Single point payload upserted to a workspace's Qdrant collection."""

    model_config = ConfigDict(extra="forbid")

    source_kind: SourceKind
    file_path: str | None = None
    line_start: int | None = Field(default=None, ge=0)
    line_end: int | None = Field(default=None, ge=0)
    commit_oid: str | None = None
    text: str
    text_hash: TextHash
    language: str | None = None
    added_by: AddedBy
    session_id: str | None = None
    chunk_index: int = Field(ge=0)
    chunk_total: int = Field(gt=0)
    created_at: datetime
    source_mtime: datetime | None = None
    sanitize_stats: dict[str, Annotated[int, Field(ge=0)]] = Field(
        default_factory=dict
    )
    related_stations: list[StationId] = Field(default_factory=list)

    @model_validator(mode="after")
    def _check_chunk_window(self) -> "KBPayload":
        if self.chunk_total < self.chunk_index + 1:
            raise ValueError(
                f"chunk_total ({self.chunk_total}) must be >= chunk_index + 1 "
                f"({self.chunk_index + 1})"
            )
        return self


class KBHit(BaseModel):
    """One Qdrant search hit with deserialized payload."""

    point_id: str
    score: float
    payload: KBPayload


class KBStats(BaseModel):
    """Aggregated counters returned by ``KnowledgeBase.build``.

    Per spec invariant: ``points_upserted + skipped_hash_count == chunks_emitted``
    when ``warnings`` is empty; oversize-skipped chunks add to ``warnings``
    instead of inflating ``skipped_hash_count``.
    """

    chunks_emitted: int = Field(ge=0)
    points_upserted: int = Field(ge=0)
    skipped_hash_count: int = Field(ge=0)
    batches_embedded: int = Field(ge=0)
    prompt_tokens_total: int = Field(ge=0)
    warnings: list[str] = Field(default_factory=list)
    duration_seconds: float = Field(ge=0.0)
    workspace_id: str
    collection_name: str


class KBProgressEvent(BaseModel):
    """One progress event emitted by ``KnowledgeBase.build``."""

    phase: ProgressPhase
    current: int = Field(ge=0)
    total: int = Field(ge=0)
    workspace_id: str
    message: str | None = None


ProgressCallback = Callable[[KBProgressEvent], Awaitable[None]]


class ChunkDraft(BaseModel):
    """In-flight chunk before embed + upsert.

    Spec ``Token-window chunker respects line boundaries`` requires
    ``text``, ``line_start`` (1-based inclusive), ``line_end`` (1-based
    inclusive), ``token_count``; ``chunk_index`` / ``chunk_total`` are
    populated by the builder before persistence so a chunker that emits
    one slice cannot know N upfront.
    """

    text: str
    line_start: int = Field(ge=1)
    line_end: int = Field(ge=1)
    token_count: int = Field(ge=0)
    chunk_index: int = Field(default=0, ge=0)
    chunk_total: int = Field(default=1, gt=0)
    flags: list[str] = Field(default_factory=list)


__all__ = [
    "AddedBy",
    "ChunkDraft",
    "KBHit",
    "KBPayload",
    "KBProgressEvent",
    "KBStats",
    "ProgressCallback",
    "ProgressPhase",
    "SourceKind",
    "StationId",
    "TextHash",
]
