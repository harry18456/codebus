"""Tests for generator prompt templates (Section 6)."""
from __future__ import annotations

import re

from codebus_agent.generator.prompts import (
    STATION_PROMPT_VERSION,
    STATION_SYSTEM_INTERACTIVE,
    STATION_SYSTEM_PLAIN,
    render_station_prompt,
)


def test_station_prompt_version_is_date_versioned() -> None:
    assert re.fullmatch(r"\d{4}-\d{2}-\d{2}-\d+", STATION_PROMPT_VERSION), (
        f"STATION_PROMPT_VERSION={STATION_PROMPT_VERSION!r} must match "
        f"^\\d{{4}}-\\d{{2}}-\\d{{2}}-\\d+$ (date-version aligned with "
        f"JUDGE_PROMPT_VERSION)"
    )


def test_interactive_system_prompt_mentions_component_constraints() -> None:
    assert "Checkpoint" in STATION_SYSTEM_INTERACTIVE
    assert "Quiz" in STATION_SYSTEM_INTERACTIVE
    assert "800" in STATION_SYSTEM_INTERACTIVE
    assert "30" in STATION_SYSTEM_INTERACTIVE


def test_plain_system_prompt_forbids_component_tags() -> None:
    # plain mode prompt MUST instruct the LLM to skip every component tag.
    assert "Checkpoint" in STATION_SYSTEM_PLAIN  # mentioned for the conversion rule
    assert "Quiz" in STATION_SYSTEM_PLAIN
    assert "QAEntry" in STATION_SYSTEM_PLAIN
    assert "task list" in STATION_SYSTEM_PLAIN or "- [ ]" in STATION_SYSTEM_PLAIN
    assert "思考題" in STATION_SYSTEM_PLAIN


def test_render_station_prompt_includes_core_context() -> None:
    rendered = render_station_prompt(
        mode="interactive",
        target_persona="experienced engineer",
        station_title="Storage Interface",
        station_index=2,
        task="add gdrive adapter",
        related_files_excerpt="src/storage.ts:1-20\n...",
    )
    assert "add gdrive adapter" in rendered
    assert "Storage Interface" in rendered
    assert "experienced engineer" in rendered
    assert "interactive" in rendered
    assert "src/storage.ts" in rendered


def test_render_station_prompt_carries_correction_hint() -> None:
    rendered = render_station_prompt(
        mode="interactive",
        target_persona="junior",
        station_title="X",
        station_index=1,
        task="t",
        correction_hint="missing_checkpoint, too_long",
    )
    assert "missing_checkpoint" in rendered
    assert "too_long" in rendered
