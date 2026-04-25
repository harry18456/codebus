"""Stable station id (``s{NN}-{slug}``) generation.

Backs Requirement
``Stable station id generation produces s{NN}-{slug} with collision handling``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

The id is permanent for the life of a generated tutorial: once written,
re-runs against the same workspace MUST preserve it (D-029 §十六.2
invariant). Callers wanting that behavior populate ``existing_ids``
from the prior ``route.json``.
"""
from __future__ import annotations

import re

__all__ = ["generate_station_id"]


_SLUG_MAX_LEN: int = 40
_FALLBACK_SLUG: str = "station"
_NON_ALNUM_RE = re.compile(r"[^a-z0-9]+")
_DASH_RUN_RE = re.compile(r"-+")


def generate_station_id(
    station_index: int,
    station_title: str,
    existing_ids: set[str],
) -> str:
    """Produce ``s{NN}-{slug}`` with collision handling.

    Slug pipeline (in order):
        1. Lowercase the title
        2. Replace any character not in ``[a-z0-9]`` with a single ``-``
        3. Collapse consecutive ``-`` into one
        4. Strip leading and trailing ``-``
        5. Truncate to at most 40 characters at a ``-`` boundary
           (or hard truncate if no boundary exists below the limit)
        6. If the resulting slug is empty (e.g., title was all CJK),
           fall back to literal ``"station"``

    Collision handling: ``s{NN}-{slug}`` collides → ``-2`` suffix; that
    collides too → ``-3``, ``-4``, ... until a free slot is found.
    """
    prefix = f"s{station_index:02d}"

    lowered = station_title.lower()
    spaced = _NON_ALNUM_RE.sub("-", lowered)
    collapsed = _DASH_RUN_RE.sub("-", spaced)
    stripped = collapsed.strip("-")

    truncated = _truncate_at_boundary(stripped, _SLUG_MAX_LEN)
    slug = truncated or _FALLBACK_SLUG

    candidate = f"{prefix}-{slug}"
    if candidate not in existing_ids:
        return candidate

    suffix = 2
    while True:
        bumped = f"{candidate}-{suffix}"
        if bumped not in existing_ids:
            return bumped
        suffix += 1


def _truncate_at_boundary(slug: str, limit: int) -> str:
    """Truncate ``slug`` to ``limit`` characters at the last ``-`` boundary.

    If the slug already fits, return it unchanged. If no ``-`` exists
    inside the first ``limit`` characters, hard-truncate to ``limit``.
    """
    if len(slug) <= limit:
        return slug
    head = slug[:limit]
    last_dash = head.rfind("-")
    if last_dash <= 0:
        return head
    return head[:last_dash]
