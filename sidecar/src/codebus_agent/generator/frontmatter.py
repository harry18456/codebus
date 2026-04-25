"""YAML frontmatter renderer.

Backs Requirement
``Frontmatter renderer produces D-029 schema_version 1 YAML``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.

Renders D-029 §7.3 schema as a YAML block delimited by ``---`` lines,
suitable for prepending to a markdown station file. Field order is
fixed; optional list fields are omitted when ``None`` / empty rather
than rendered as ``key: []`` (avoids editor noise + the spec scenario
``Optional empty lists are omitted`` requires it).

Schema-bumping policy: future additive fields keep ``schema_version=1``;
removing a field or changing its type bumps the version and forces a
migration note in the next change's ``design.md``.
"""
from __future__ import annotations

from typing import Any

import yaml

from .types import Frontmatter

__all__ = ["render_frontmatter"]


_REQUIRED_FIELD_ORDER: tuple[str, ...] = (
    "schema_version",
    "station_id",
    "station_index",
    "title",
    "duration_minutes",
    "workspace_type",
    "repo_name",
    "task",
    "generated_at",
    "required_checks",
    "degraded",
)

_OPTIONAL_FIELD_ORDER: tuple[str, ...] = (
    "tags",
    "related_stations",
    "related_files",
)


def render_frontmatter(meta: Frontmatter) -> str:
    """Render ``meta`` as ``---``-delimited YAML.

    The output ends with a trailing newline after the closing ``---``
    so callers can directly concatenate the markdown body without
    worrying about delimiter spacing.
    """
    payload: dict[str, Any] = {}

    # Required fields — always emitted, in fixed order.
    payload["schema_version"] = int(meta.schema_version)
    payload["station_id"] = str(meta.station_id)
    payload["station_index"] = int(meta.station_index)
    payload["title"] = str(meta.title)
    payload["duration_minutes"] = int(meta.duration_minutes)
    payload["workspace_type"] = str(meta.workspace_type)
    payload["repo_name"] = str(meta.repo_name)
    payload["task"] = str(meta.task)
    payload["generated_at"] = meta.generated_at.isoformat()
    payload["required_checks"] = list(meta.required_checks)
    payload["degraded"] = bool(meta.degraded)

    # Optional list fields — omit when None / empty so YAML stays clean.
    for field_name in _OPTIONAL_FIELD_ORDER:
        value = getattr(meta, field_name, None)
        if value:
            payload[field_name] = list(value)

    body = yaml.safe_dump(
        payload,
        sort_keys=False,
        allow_unicode=True,
        default_flow_style=False,
    )
    return f"---\n{body}---\n"
