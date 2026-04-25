"""Tests for markdown validator (Section 5).

Backs Requirement
``Markdown validator enforces D-029 component rules``.
"""
from __future__ import annotations

from pathlib import Path

from codebus_agent.generator.validator import validate_station_markdown


_WS = Path("/tmp/ws")


def test_interactive_mode_rejects_markdown_without_checkpoint() -> None:
    md = "# Title\n\nSome text without any custom components.\n"
    result = validate_station_markdown(
        md, station_idx=2, mode="interactive", workspace_root=_WS
    )
    assert "missing_checkpoint" in result.issues


def test_interactive_mode_rejects_two_quizzes() -> None:
    md = (
        "# Title\n\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] foo\n</Checkpoint>\n\n"
        "<Quiz id=\"s2-q1\" correct=\"a\">\n- a) one\n- b) two\n</Quiz>\n\n"
        "<Quiz id=\"s2-q2\" correct=\"b\">\n- a) one\n- b) two\n</Quiz>\n"
    )
    result = validate_station_markdown(
        md, station_idx=2, mode="interactive", workspace_root=_WS
    )
    assert "too_many_quizzes" in result.issues


def test_quiz_with_invalid_correct_attribute_is_rejected() -> None:
    md = (
        "# Title\n\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] foo\n</Checkpoint>\n\n"
        "<Quiz id=\"s2-q1\" correct=\"e\">\n- a) one\n- b) two\n</Quiz>\n"
    )
    result = validate_station_markdown(
        md, station_idx=2, mode="interactive", workspace_root=_WS
    )
    assert "quiz_bad_correct: e" in result.issues


def test_length_over_800_chars_fails_validation() -> None:
    body = "X" * 1500
    md = (
        "# Title\n\n"
        f"{body}\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] foo\n</Checkpoint>\n"
    )
    result = validate_station_markdown(
        md, station_idx=2, mode="interactive", workspace_root=_WS
    )
    assert "too_long" in result.issues


def test_coderef_pointing_outside_workspace_fails_validation() -> None:
    md = (
        "# Title\n\n"
        "<CodeRef file=\"../../etc/passwd\" lines=\"1-10\" />\n\n"
        "<Checkpoint id=\"station-2-check\">\n- [ ] foo\n</Checkpoint>\n"
    )
    result = validate_station_markdown(
        md, station_idx=2, mode="interactive", workspace_root=_WS
    )
    assert "coderef_escape: ../../etc/passwd" in result.issues


def test_plain_mode_tolerates_absence_of_components() -> None:
    md = "# Title\n\nPlain prose without any custom elements.\n"
    result = validate_station_markdown(
        md, station_idx=1, mode="plain", workspace_root=_WS
    )
    component_issues = [
        i
        for i in result.issues
        if i == "missing_checkpoint"
        or i == "too_many_quizzes"
        or i.startswith("quiz_")
        or i.startswith("coderef_")
    ]
    assert component_issues == [], (
        f"plain mode must skip component-specific issues, got: {result.issues}"
    )
