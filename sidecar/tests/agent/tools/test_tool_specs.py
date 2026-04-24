"""RED tests for FolderTools.tool_specs().

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/agent-core/spec.md
  Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols
    (modified — adds optional tool_specs method)
"""
from __future__ import annotations

from codebus_agent.sandbox import ToolContext


def test_folder_tools_advertises_tool_surface_via_tool_specs(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    specs = tools.tool_specs()

    assert isinstance(specs, list)
    names = {s["name"] for s in specs}
    assert {"search", "list_dir", "read_file", "mark_station"}.issubset(names), (
        f"tool_specs MUST cover all four P0 tools; got {names}"
    )

    for spec in specs:
        assert "name" in spec and isinstance(spec["name"], str)
        assert "description" in spec and isinstance(spec["description"], str)
        assert "parameters" in spec and isinstance(spec["parameters"], dict)
