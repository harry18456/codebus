"""Tests for Q&A budget constants.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/qa-agent/spec.md
  Requirement: Q&A budget constants are module-level
"""
from __future__ import annotations


def test_constants_present_with_correct_values() -> None:
    from codebus_agent.agent.qa import (
        _QA_DEDUP_THRESHOLD,
        _QA_MAX_ADD_TO_KB_PER_QUESTION,
        _QA_MAX_ADD_TO_KB_PER_SESSION,
        _QA_MAX_CHUNK_SIZE_CHARS,
        _QA_MAX_STEPS,
    )

    assert _QA_MAX_STEPS == 10
    assert _QA_MAX_ADD_TO_KB_PER_SESSION == 20
    assert _QA_MAX_CHUNK_SIZE_CHARS == 2000
    assert _QA_MAX_ADD_TO_KB_PER_QUESTION == 5
    assert _QA_DEDUP_THRESHOLD == 0.95
