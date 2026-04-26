"""`kb_search` Q&A tool — KB query with optional station filter.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: kb_search invokes KnowledgeBase query with optional station filter

Per Decision 8 (audit_fields excludes free-text):
``audit_fields = ["query", "top_k", "station_filter"]`` — `query` is the
Agent's decision trace (not pre-sanitize user content); `station_filter`
and `top_k` are scalar control flags. The tool's return value is a
multi-line rendered string consumed by the Q&A ReAct loop's prompt.
"""
from __future__ import annotations

import re
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, field_validator


__all__ = ["KBSearchArgs", "kb_search"]


_STATION_ID_RE = re.compile(r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$")
_SNIPPET_TRUNCATE_LIMIT: int = 200


class KBSearchArgs(BaseModel):
    """Pydantic schema for the `kb_search` tool's arguments.

    `station_filter` entries MUST match the stable station id regex —
    Pydantic raises `ValidationError` before the tool body runs (spec
    scenario `Invalid station id rejected by Pydantic`).
    """

    model_config = ConfigDict(extra="forbid")

    query: str
    top_k: int = Field(default=5, ge=1, le=50)
    station_filter: list[str] | None = None

    @field_validator("station_filter", mode="after")
    @classmethod
    def _validate_station_filter(cls, v: list[str] | None) -> list[str] | None:
        if v is None:
            return None
        for sid in v:
            if not isinstance(sid, str) or not _STATION_ID_RE.fullmatch(sid):
                raise ValueError(
                    f"station_filter entry {sid!r} must match "
                    r"^s\d{2}-[a-z0-9-]{1,40}(-\d+)?$"
                )
        return list(v)


# `audit_fields` is read by `QATools.kb_search.audit_fields` callers;
# kept at module scope so the tool function and the QATools wrapper agree.
_AUDIT_FIELDS: list[str] = ["query", "top_k", "station_filter"]


def _render_hit(hit: Any) -> str:
    """Render one `KBHit` into the multi-line text format Q&A loop expects.

    Hit shape: `<file>:<line> | score=<X.XX>[ | stations=[<id>,...]]\n  <snippet>`.
    Empty `payload.related_stations` MUST omit the `stations=` segment
    entirely (spec scenario `Hit rendering omits empty station list`).
    """
    payload = hit.payload
    file_path = payload.file_path or "?"
    line_start = payload.line_start or 0
    head_parts = [f"{file_path}:{line_start}", f"score={hit.score:.2f}"]
    if payload.related_stations:
        ids = ",".join(payload.related_stations)
        head_parts.append(f"stations=[{ids}]")
    head = " | ".join(head_parts)

    snippet = (payload.text or "").replace("\n", " ")
    if len(snippet) > _SNIPPET_TRUNCATE_LIMIT:
        snippet = snippet[:_SNIPPET_TRUNCATE_LIMIT] + "…"
    return f"{head}\n  {snippet}"


async def kb_search(args: KBSearchArgs, ctx: Any) -> str:
    """Forward to `ctx.kb.query` with station-filter pass-through.

    `ctx.kb` is duck-typed against `KnowledgeBase` (sidecar runtime) so
    tests can inject mocks without importing the heavy KB module here.
    """
    if ctx.kb is None:
        return "kb_search unavailable: ctx.kb is None"
    hits = await ctx.kb.query(
        args.query,
        top_k=args.top_k,
        filter_stations=args.station_filter,
    )
    if not hits:
        return "（無命中）"
    return "\n".join(_render_hit(h) for h in hits)


# Attach `audit_fields` as an attribute on the function object so
# `QATools.kb_search.audit_fields` resolves to the same list regardless
# of whether the caller reads it off the class or the function.
kb_search.audit_fields = _AUDIT_FIELDS  # type: ignore[attr-defined]
