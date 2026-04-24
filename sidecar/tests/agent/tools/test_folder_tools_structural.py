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
    """The loop's getattr(tools, call.name) reaches every concrete method.

    Post explorer-tools-p1, FolderTools carries six methods: the P0 four
    (search / list_dir / read_file / mark_station) plus the P1 symbol-
    navigation pair (trace_import / find_callers). All MUST be reachable
    via ``getattr(tools, call.name)`` so ``ExplorerAction.tool_calls``
    can reach them without a separate dispatch table.
    """
    from codebus_agent.agent.explorer import _execute_one
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import ToolCall

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    # Dispatch success criterion: tool_name is routed to the method we
    # named. Whether that method raises (NotImplementedError in the stub
    # phase) or returns OK isn't the dispatch test's concern — the
    # per-tool test files cover behavioural contracts.
    names = (
        "search",
        "list_dir",
        "read_file",
        "mark_station",
        "trace_import",
        "find_callers",
    )
    args_by_name = {
        "search": {"keyword": "x"},
        "list_dir": {"path": "."},
        "read_file": {"path": "app.py"},
        "mark_station": {"path": "app.py", "role": "seed", "why": "..."},
        "trace_import": {"symbol": "entry"},
        "find_callers": {"symbol": "entry"},
    }
    for name in names:
        result = await _execute_one(
            ToolCall(id=f"tc_{name}", name=name, arguments=args_by_name[name]),
            tools,
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
    """Unknown tool names collapse into ToolResult.error without raising.

    ``trace_import`` / ``find_callers`` landed in explorer-tools-p1, so
    they are no longer valid placeholders for the unknown-tool path.
    ``find_nonexistent`` stands in — a deliberately never-implemented
    name that ``FolderTools.__getattr__`` will miss.
    """
    from codebus_agent.agent.explorer import _execute_one
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import ToolCall

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    result = await _execute_one(
        ToolCall(
            id="tc_missing",
            name="find_nonexistent",
            arguments={"symbol": "foo"},
        ),
        tools,
    )
    assert result.error is not None
    assert "find_nonexistent" in (result.error or "") + (result.output or "")
