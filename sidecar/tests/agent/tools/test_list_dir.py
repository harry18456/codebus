"""RED tests for FolderTools.list_dir.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: list_dir and read_file enforce ensure_in_workspace
"""
from __future__ import annotations

import os
from pathlib import Path

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext


async def test_list_dir_happy_path(tool_context: ToolContext, explorer_state) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    entries = await tools.list_dir(".")

    # temp_workspace fixture seeds: app.py / secret.py / helper.py / README.md / image.png / subdir/
    # Plus .codebus/ is explicitly excluded per spec.
    names = {e.name for e in entries}
    assert "app.py" in names
    assert "subdir" in names
    assert ".codebus" not in names, "list_dir MUST exclude the .codebus audit subdir"

    # Kinds are properly distinguished
    kind_by_name = {e.name: e.kind for e in entries}
    assert kind_by_name["app.py"] == "file"
    assert kind_by_name["subdir"] == "dir"


async def test_list_dir_nested_path_accepted(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    entries = await tools.list_dir("subdir")

    names = {e.name for e in entries}
    assert "nested.py" in names
    assert all(e.kind in {"file", "dir"} for e in entries)


async def test_list_dir_parent_escape_rejected(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    with pytest.raises(PathEscapeError):
        await tools.list_dir("../..")


@pytest.mark.skipif(
    os.name == "nt",
    reason="symlink creation on Windows requires elevated privileges",
)
async def test_list_dir_symlink_escape_rejected(
    tool_context: ToolContext, explorer_state, tmp_path: Path
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Create a target dir OUTSIDE the workspace and a symlink INSIDE it
    outside = tmp_path / "outside"
    outside.mkdir()
    (outside / "evil.py").write_text("secret\n", encoding="utf-8")

    ws = tool_context.workspace_root
    link = ws / "link_to_outside"
    os.symlink(outside, link, target_is_directory=True)

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    with pytest.raises(PathEscapeError):
        await tools.list_dir("link_to_outside")
