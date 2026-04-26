"""Tests for `qa` task_id format and TaskKind enum.

Backs SHALL clauses in
openspec/changes/module-8-qa-p0/specs/sidecar-runtime/spec.md
  Requirement: task_id format (qa kind)
"""
from __future__ import annotations

import re
import typing as _t


def test_qa_kind_matches_regex() -> None:
    from codebus_agent.api.tasks import _generate_task_id

    task_id = _generate_task_id("qa")
    assert re.fullmatch(r"^qa_[0-9a-f]{8}$", task_id), task_id


def test_taskkind_includes_qa() -> None:
    from codebus_agent.api.tasks import TaskKind

    args = _t.get_args(TaskKind)
    assert "qa" in args


def test_qa_failed_in_error_codes() -> None:
    from codebus_agent.api.tasks import ERROR_CODES

    assert "QA_FAILED" in ERROR_CODES
