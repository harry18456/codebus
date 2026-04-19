"""Backs SHALL clauses in
openspec/changes/m1-power-on/specs/usage-tracking/spec.md
  Requirement: UsageTracker writes token_usage.jsonl
    Scenario: One line per chat call
    Scenario: Required fields present
    Scenario: Embed calls tracked
"""
from __future__ import annotations

import json
from pathlib import Path

from codebus_agent.providers.usage_tracker import UsageTracker

REQUIRED_FIELDS = {
    "timestamp",
    "provider",
    "model",
    "operation",
    "input_tokens",
    "output_tokens",
    "cost_usd",
}


def _read_lines(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def test_record_chat_appends_single_line(tmp_path: Path) -> None:
    """Scenario: One line per chat call."""
    jsonl = tmp_path / "token_usage.jsonl"
    tracker = UsageTracker(jsonl)

    tracker.record(
        provider="mock",
        model="mock-chat-v1",
        operation="chat",
        input_tokens=42,
        output_tokens=17,
        cost_usd=0.0,
    )

    lines = _read_lines(jsonl)
    assert len(lines) == 1


def test_record_chat_writes_all_required_fields(tmp_path: Path) -> None:
    """Scenario: Required fields present (non-null)."""
    jsonl = tmp_path / "token_usage.jsonl"
    tracker = UsageTracker(jsonl)

    tracker.record(
        provider="mock",
        model="mock-chat-v1",
        operation="chat",
        input_tokens=10,
        output_tokens=5,
        cost_usd=0.0,
    )

    entry = _read_lines(jsonl)[0]
    assert REQUIRED_FIELDS <= set(entry.keys())
    for key in REQUIRED_FIELDS:
        assert entry[key] is not None, f"{key} must be non-null"


def test_record_embed_sets_output_tokens_zero(tmp_path: Path) -> None:
    """Scenario: Embed calls tracked."""
    jsonl = tmp_path / "token_usage.jsonl"
    tracker = UsageTracker(jsonl)

    tracker.record(
        provider="mock",
        model="mock-embed-v1",
        operation="embed",
        input_tokens=123,
        output_tokens=0,
        cost_usd=0.0,
    )

    entry = _read_lines(jsonl)[0]
    assert entry["operation"] == "embed"
    assert entry["output_tokens"] == 0


def test_multiple_records_appended_in_order(tmp_path: Path) -> None:
    jsonl = tmp_path / "token_usage.jsonl"
    tracker = UsageTracker(jsonl)

    for i in range(3):
        tracker.record(
            provider="mock",
            model="mock-chat-v1",
            operation="chat",
            input_tokens=i,
            output_tokens=i * 2,
            cost_usd=0.0,
        )

    lines = _read_lines(jsonl)
    assert [entry["input_tokens"] for entry in lines] == [0, 1, 2]


def test_record_creates_parent_directory(tmp_path: Path) -> None:
    jsonl = tmp_path / "nested" / "dir" / "token_usage.jsonl"
    tracker = UsageTracker(jsonl)

    tracker.record(
        provider="mock",
        model="mock-chat-v1",
        operation="chat",
        input_tokens=1,
        output_tokens=1,
        cost_usd=0.0,
    )

    assert jsonl.exists()
