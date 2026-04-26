"""Tests for `_hits_confident` three-condition gate.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: `_hits_confident` declares three threshold conditions
"""
from __future__ import annotations

from datetime import datetime, timezone

from codebus_agent.agent.qa import _hits_confident
from codebus_agent.kb.payload import KBHit, KBPayload


def _hit(score: float, text: str = "") -> KBHit:
    payload = KBPayload(
        source_kind="code",
        file_path="src/x.py",
        line_start=1,
        line_end=2,
        text=text,
        text_hash="0" * 64,
        added_by="qa_agent",
        chunk_index=0,
        chunk_total=1,
        created_at=datetime.now(timezone.utc),
    )
    return KBHit(point_id="pt", score=score, payload=payload)


def test_all_three_conditions_met() -> None:
    """top-1 > 0.75 + top-3 mean > 0.65 + entity coverage → True."""
    hits = [
        _hit(0.82, "storage adapter implementation"),
        _hit(0.75, "storage utilities"),
        _hit(0.68, "storage interface"),
    ]
    question = "how does the storage adapter work"
    assert _hits_confident(question, hits) is True


def test_insufficient_hits_returns_false() -> None:
    """`len(hits) < 3` → False regardless of individual scores."""
    hits = [_hit(0.99, "anything")]
    assert _hits_confident("question", hits) is False


def test_high_top1_no_entity_coverage_returns_false() -> None:
    """High top-1 but top-5 lack any question-significant token → False."""
    hits = [
        _hit(0.90, "ALPHA BETA GAMMA"),
        _hit(0.80, "DELTA"),
        _hit(0.70, "EPSILON"),
    ]
    question = "how do payment refunds work"
    assert _hits_confident(question, hits) is False


def test_low_top1_returns_false() -> None:
    """top-1 ≤ 0.75 → False regardless of other conditions."""
    hits = [
        _hit(0.74, "storage adapter"),
        _hit(0.73, "storage adapter"),
        _hit(0.72, "storage adapter"),
    ]
    assert _hits_confident("storage adapter", hits) is False
