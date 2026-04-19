"""ToolContext discriminator tests — backs SHALL clauses in
openspec/changes/m1-power-on/specs/tool-sandbox/spec.md
  Requirement: ToolContext carries workspace type discriminator
"""
from __future__ import annotations

from pathlib import Path

import pytest
from pydantic import ValidationError

from codebus_agent.sandbox import ToolContext


def test_folder_workspace_type_accepted(tmp_path: Path) -> None:
    """Scenario: Folder workspace accepted."""
    ctx = ToolContext(workspace_root=tmp_path, workspace_type="folder")
    assert ctx.workspace_type == "folder"


def test_topic_workspace_type_accepted_at_schema_level(tmp_path: Path) -> None:
    """Scenario: Topic workspace accepted at schema level.

    The 'topic' value must pass validation day 1 even though M1 has no
    topic-mode behavior — breaking later requires a schema migration,
    which D-002 forbids.
    """
    ctx = ToolContext(workspace_root=tmp_path, workspace_type="topic")
    assert ctx.workspace_type == "topic"


@pytest.mark.parametrize(
    "bad_type",
    ["", "Folder", "FOLDER", "file", "repo", None, 42],
)
def test_invalid_workspace_type_rejected(tmp_path: Path, bad_type) -> None:
    """Scenario: Invalid workspace type rejected."""
    with pytest.raises(ValidationError):
        ToolContext(workspace_root=tmp_path, workspace_type=bad_type)


def test_tool_context_is_frozen(tmp_path: Path) -> None:
    """ToolContext is authoritative per-run — once built it must be
    immutable so tools cannot silently relocate the workspace mid-run.
    """
    ctx = ToolContext(workspace_root=tmp_path, workspace_type="folder")
    with pytest.raises(ValidationError):
        ctx.workspace_type = "topic"  # type: ignore[misc]
