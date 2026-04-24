"""RED tests for FolderTools.tool_specs().

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/agent-core/spec.md
  Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols
    (modified — adds optional tool_specs method)
openspec/changes/explorer-tools-p1/specs/explorer-tools/spec.md
  Requirement: trace_import resolves symbols to definition paths via regex
  Requirement: find_callers returns sanitized call-site FileMatches
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
    expected = {
        "search",
        "list_dir",
        "read_file",
        "mark_station",
        "trace_import",
        "find_callers",
    }
    assert expected.issubset(names), (
        f"tool_specs MUST cover all six tools (P0 four + P1 two); got {names}"
    )

    for spec in specs:
        assert "name" in spec and isinstance(spec["name"], str)
        assert "description" in spec and isinstance(spec["description"], str)
        assert "parameters" in spec and isinstance(spec["parameters"], dict)
        # Every spec MUST have a valid JSON-schema-ish parameters shape.
        params = spec["parameters"]
        assert params.get("type") == "object", (
            f"spec {spec['name']!r} parameters.type MUST be 'object'; got {params}"
        )
        assert "properties" in params and isinstance(params["properties"], dict)
        assert "required" in params and isinstance(params["required"], list)


def test_p1_tool_specs_declare_symbol_parameter(
    tool_context: ToolContext, explorer_state
) -> None:
    """trace_import / find_callers both take a single required ``symbol`` string."""
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    by_name = {s["name"]: s for s in tools.tool_specs()}

    for p1_name in ("trace_import", "find_callers"):
        spec = by_name.get(p1_name)
        assert spec is not None, f"{p1_name} MUST appear in tool_specs"
        params = spec["parameters"]
        assert params["required"] == ["symbol"], (
            f"{p1_name} MUST require 'symbol'; got {params['required']}"
        )
        props = params["properties"]
        assert "symbol" in props and props["symbol"].get("type") == "string", (
            f"{p1_name}.symbol MUST be string-typed; got {props.get('symbol')}"
        )
