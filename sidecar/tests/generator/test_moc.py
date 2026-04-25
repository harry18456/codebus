"""Tests for MOC assembler (Section 9)."""
from __future__ import annotations

import re
from datetime import datetime, timezone
from pathlib import Path

from codebus_agent.generator.moc import assemble_moc
from codebus_agent.generator.types import StationSummary


_TS = datetime(2026, 4, 25, 10, 30, 0, tzinfo=timezone.utc)


def _summaries() -> list[StationSummary]:
    return [
        StationSummary(station_id="s01-overview", title="Repo Overview", duration=10),
        StationSummary(station_id="s02-storage", title="Storage", duration=15),
        StationSummary(
            station_id="s03-adapter", title="Adapter Pattern", duration=20
        ),
    ]


def test_interactive_moc_contains_numbered_station_list_with_standard_markdown_links(
    tmp_path: Path,
) -> None:
    out = tmp_path / "tutorial.md"
    assemble_moc(
        task="add gdrive adapter",
        total_minutes=45,
        generated_at=_TS,
        workspace_name="timeline",
        station_summaries=_summaries(),
        mode="interactive",
        output_path=out,
    )
    text = out.read_text(encoding="utf-8")
    # Must contain three numbered list items pointing at standard
    # markdown links (no wikilinks `[[...]]`).
    pattern = re.compile(
        r"^\d+\.\s+🚏\s+\[.*\]\(\.\/stations\/s\d{2}-[a-z0-9-]+\.md\)",
        re.MULTILINE,
    )
    matches = pattern.findall(text)
    assert len(matches) == 3, f"expected 3 numbered items, got {matches!r}\n{text}"
    assert "[[" not in text, "wikilink syntax MUST NOT appear (D-029 §十六.1)"


def test_interactive_moc_ends_with_qaentry_element(tmp_path: Path) -> None:
    out = tmp_path / "tutorial.md"
    assemble_moc(
        task="t",
        total_minutes=45,
        generated_at=_TS,
        workspace_name="ws",
        station_summaries=_summaries(),
        mode="interactive",
        output_path=out,
    )
    text = out.read_text(encoding="utf-8")
    assert text.count("<QAEntry") == 1
    end_heading_idx = text.find("🎯 下車（完成）")
    qaentry_idx = text.find("<QAEntry")
    assert end_heading_idx >= 0, "missing 🎯 下車（完成）heading"
    assert qaentry_idx > end_heading_idx, "QAEntry must follow the end heading"


def test_plain_moc_replaces_qaentry_with_plain_sentence(tmp_path: Path) -> None:
    out = tmp_path / "tutorial.md"
    assemble_moc(
        task="t",
        total_minutes=45,
        generated_at=_TS,
        workspace_name="ws",
        station_summaries=_summaries(),
        mode="plain",
        output_path=out,
    )
    text = out.read_text(encoding="utf-8")
    assert "<QAEntry" not in text
    assert "本專案有 Q&A 功能可對話式繼續學習。" in text
