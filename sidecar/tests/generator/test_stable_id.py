"""Tests for stable station id generator (Section 3).

Backs Requirement
``Stable station id generation produces s{NN}-{slug} with collision handling``
in `openspec/changes/module-5-generator-p0/specs/module-5-generator/spec.md`.
"""
from __future__ import annotations

from codebus_agent.generator.stable_id import generate_station_id


def test_ascii_title_produces_clean_slug() -> None:
    assert (
        generate_station_id(2, "Storage Interface Contract", set())
        == "s02-storage-interface-contract"
    )


def test_cjk_only_title_falls_back_to_station() -> None:
    assert generate_station_id(1, "儲存介面契約", set()) == "s01-station"


def test_slug_truncates_at_dash_boundary_under_40_chars() -> None:
    # 60-char title with dash boundaries; the slug pipeline should pick
    # a `-`-aligned cut at or below the 40-char limit, never a mid-word
    # truncation.
    title = "the quick brown fox jumps over the lazy dog twice today"
    assert len(title) > 40
    result = generate_station_id(1, title, set())
    assert result.startswith("s01-")
    slug = result[len("s01-") :]
    assert len(slug) <= 40
    # Cut must be a dash boundary — never end mid-word.
    assert not slug.endswith("-")
    next_char = title.lower()[len(slug)] if len(slug) < len(title) else " "
    assert next_char == " " or next_char == "-"


def test_collision_appends_dash_two_suffix() -> None:
    existing = {"s03-storage-interface-contract"}
    assert (
        generate_station_id(3, "Storage Interface Contract", existing)
        == "s03-storage-interface-contract-2"
    )


def test_index_zero_padded_to_two_digits() -> None:
    assert generate_station_id(7, "x", set()).startswith("s07-")
