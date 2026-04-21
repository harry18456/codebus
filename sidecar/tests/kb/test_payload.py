"""KBPayload regression tests.

Backs SHALL clauses in
openspec/changes/module-2-kb-builder-p0/specs/knowledge-base/spec.md
  Requirement: KBPayload schema
    Scenario: Valid payload constructs without error
    Scenario: Invalid text_hash rejected
    Scenario: Invalid related_stations id rejected
    Scenario: chunk_total must cover chunk_index
"""
from __future__ import annotations

import hashlib
from datetime import datetime, timezone

import pytest
from pydantic import ValidationError

from codebus_agent.kb.payload import KBPayload


def _hash(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _base_kwargs(**overrides: object) -> dict[str, object]:
    text = overrides.pop("text", "hello") if "text" in overrides else "hello"
    base: dict[str, object] = {
        "source_kind": "code",
        "file_path": "src/x.py",
        "line_start": 1,
        "line_end": 3,
        "commit_oid": None,
        "text": text,
        "text_hash": _hash(text),
        "language": "python",
        "added_by": "scanner",
        "session_id": None,
        "chunk_index": 0,
        "chunk_total": 1,
        "created_at": datetime(2026, 4, 21, tzinfo=timezone.utc),
        "source_mtime": datetime(2026, 4, 20, tzinfo=timezone.utc),
        "sanitize_stats": {"email": 1},
        "related_stations": ["s01-overview", "s02-architecture-3"],
    }
    base.update(overrides)
    return base


def test_kbpayload_happy_path_round_trips() -> None:
    """Valid payload constructs and survives model_dump → model_validate."""
    payload = KBPayload(**_base_kwargs())

    dumped = payload.model_dump(mode="python")
    restored = KBPayload.model_validate(dumped)

    assert restored == payload
    # mode="json" must produce JSON-serialisable datetimes (ISO-8601 strings).
    json_dumped = payload.model_dump(mode="json")
    assert isinstance(json_dumped["created_at"], str)
    assert isinstance(json_dumped["source_mtime"], str)


@pytest.mark.parametrize(
    "bad_hash",
    [
        "deadbeef",  # too short
        "z" * 64,  # right length, non-hex
        "DEADBEEF" * 8,  # uppercase hex (must be lowercase)
        "",
    ],
)
def test_kbpayload_rejects_invalid_text_hash(bad_hash: str) -> None:
    """Scenario: Invalid text_hash rejected."""
    with pytest.raises(ValidationError):
        KBPayload(**_base_kwargs(text_hash=bad_hash))


@pytest.mark.parametrize(
    "bad_station",
    [
        "s1-x",  # single-digit prefix
        "s001-overview",  # three-digit prefix
        "s01-" + "a" * 41,  # slug exceeds 40 chars
        "s01-",  # empty slug
        "S01-overview",  # uppercase prefix
        "s01-Overview",  # uppercase in slug
    ],
)
def test_kbpayload_rejects_malformed_related_stations(bad_station: str) -> None:
    """Scenario: Invalid related_stations id rejected."""
    with pytest.raises(ValidationError):
        KBPayload(**_base_kwargs(related_stations=[bad_station]))


def test_kbpayload_enforces_chunk_index_total_invariant() -> None:
    """Scenario: chunk_total must cover chunk_index (chunk_total >= chunk_index + 1)."""
    with pytest.raises(ValidationError):
        KBPayload(**_base_kwargs(chunk_index=3, chunk_total=2))
