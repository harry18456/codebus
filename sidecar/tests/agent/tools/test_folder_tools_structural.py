"""RED tests for FolderTools structural conformance.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools
"""
from __future__ import annotations

from pathlib import Path

from codebus_agent.agent.protocols import ExplorerTools
from codebus_agent.sandbox import ToolContext


def test_folder_tools_satisfies_explorer_tools_protocol(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    assert isinstance(tools, ExplorerTools), (
        "FolderTools MUST satisfy ExplorerTools via runtime_checkable — "
        "primary_search / fetch / follow_reference must be present"
    )


async def test_tool_dispatch_by_explorer_action_name(
    tool_context: ToolContext, explorer_state
) -> None:
    """The loop's getattr(tools, call.name) reaches search / list_dir / read_file / mark_station."""
    from codebus_agent.agent.explorer import _execute_one
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import ToolCall

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    # Dispatch success criterion: tool_name is routed to the method we
    # named. Whether that method raises (NotImplementedError in the stub
    # phase) or returns OK isn't the dispatch test's concern — sections
    # 9 / 11 / 13 / 15 cover the behavioural contracts.
    for name in ("search", "list_dir", "read_file", "mark_station"):
        args = {
            "search": {"keyword": "x"},
            "list_dir": {"path": "."},
            "read_file": {"path": "app.py"},
            "mark_station": {"path": "app.py", "role": "seed", "why": "..."},
        }[name]
        result = await _execute_one(
            ToolCall(id=f"tc_{name}", name=name, arguments=args), tools
        )
        assert result.tool_name == name
        # Dispatch landed on the method (either returned OK, or raised
        # from the method body). A missing-dispatch path would produce
        # `error="unknown tool 'X'"` — reject that.
        assert "unknown tool" not in (result.error or ""), (
            f"dispatch failed to reach FolderTools.{name}: {result.error}"
        )


async def test_unknown_tool_name_yields_tool_result_error(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.explorer import _execute_one
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import ToolCall

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    result = await _execute_one(
        ToolCall(id="tc_trace", name="trace_import", arguments={"symbol": "foo"}), tools
    )
    assert result.error is not None
    assert "trace_import" in (result.error or "") + (result.output or "")
