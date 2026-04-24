"""RED tests for tool-layer Pydantic schemas.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools
"""
from __future__ import annotations

import json

import pytest
from pydantic import ValidationError


def test_search_hit_round_trips() -> None:
    from codebus_agent.agent.tools.schemas import SearchHit

    raw = {"path": "src/app.py", "snippet": "def entry():\n    pass", "score": 0.73}
    hit = SearchHit.model_validate_json(json.dumps(raw))
    dumped = json.loads(hit.model_dump_json())
    assert dumped == raw


def test_search_hit_score_out_of_range_rejected() -> None:
    from codebus_agent.agent.tools.schemas import SearchHit

    with pytest.raises(ValidationError):
        SearchHit(path="a", snippet="b", score=1.5)
    with pytest.raises(ValidationError):
        SearchHit(path="a", snippet="b", score=-0.1)


def test_dir_entry_kind_enum() -> None:
    from codebus_agent.agent.tools.schemas import DirEntry

    ok = DirEntry(name="app.py", kind="file", size=120)
    assert ok.kind == "file"

    with pytest.raises(ValidationError):
        DirEntry(name="weird", kind="socket", size=0)


def test_dir_entry_size_non_negative() -> None:
    from codebus_agent.agent.tools.schemas import DirEntry

    DirEntry(name="empty", kind="file", size=0)  # OK
    with pytest.raises(ValidationError):
        DirEntry(name="bad", kind="file", size=-1)


def test_file_match_shape() -> None:
    """Backs openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
    Requirement: find_callers returns sanitized call-site FileMatches.

    FileMatch is intentionally minimal — path / line / snippet. No
    column / end_line / ast_node metadata; Agent callers fall back to
    ``read_file`` when they need surrounding lines.
    """
    from codebus_agent.agent.tools.schemas import FileMatch

    raw = {"path": "src/app.py", "line": 14, "snippet": "kb = KnowledgeBase(path)"}
    fm = FileMatch.model_validate_json(json.dumps(raw))
    assert fm.path == raw["path"]
    assert fm.line == raw["line"]
    assert fm.snippet == raw["snippet"]

    with pytest.raises(ValidationError):
        FileMatch(path="a", line=0, snippet="")  # line must be ≥ 1
    with pytest.raises(ValidationError):
        FileMatch(path="a", line=-5, snippet="")
