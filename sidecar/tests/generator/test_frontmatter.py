"""Tests for frontmatter renderer (Section 4).

Backs Requirement
``Frontmatter renderer produces D-029 schema_version 1 YAML``.
"""
from __future__ import annotations

from datetime import datetime, timezone

from codebus_agent.generator.frontmatter import render_frontmatter
from codebus_agent.generator.types import Frontmatter


_SAMPLE_TS = datetime(2026, 4, 25, 0, 0, 0, tzinfo=timezone.utc)


def _required_only(**overrides) -> Frontmatter:
    base: dict = {
        "schema_version": 1,
        "station_id": "s02-storage",
        "station_index": 2,
        "title": "Storage",
        "duration_minutes": 15,
        "workspace_type": "folder",
        "repo_name": "timeline",
        "task": "add gdrive",
        "generated_at": _SAMPLE_TS,
        "required_checks": ["station-2-check"],
        "degraded": False,
    }
    base.update(overrides)
    return Frontmatter(**base)


def test_required_fields_rendered_in_order() -> None:
    rendered = render_frontmatter(_required_only())

    assert rendered.startswith("---\n"), f"missing opening delimiter: {rendered!r}"
    assert rendered.endswith("\n---\n"), f"missing closing delimiter: {rendered!r}"

    body = rendered.split("---\n", 2)[1]
    keys: list[str] = []
    for line in body.splitlines():
        # Pick first-column key lines only ("schema_version: 1" but not "  - foo").
        if not line or line.startswith(" ") or line.startswith("-"):
            continue
        if line.startswith("---"):
            break
        if ":" not in line:
            continue
        keys.append(line.split(":", 1)[0])

    assert keys == [
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
    ]
    # Sanity — first key value is exactly 1.
    assert "schema_version: 1" in body


def test_optional_empty_lists_are_omitted() -> None:
    fm = _required_only(tags=[], related_stations=[], related_files=[])
    rendered = render_frontmatter(fm)
    assert "tags:" not in rendered
    assert "related_stations:" not in rendered
    assert "related_files:" not in rendered


def test_optional_populated_lists_are_rendered() -> None:
    fm = _required_only(tags=["architecture", "interfaces"])
    rendered = render_frontmatter(fm)
    assert "tags:" in rendered
    assert "architecture" in rendered
    assert "interfaces" in rendered
