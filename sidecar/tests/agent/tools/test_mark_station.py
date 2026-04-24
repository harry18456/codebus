"""RED tests for FolderTools.mark_station.

Backs SHALL clauses in
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: mark_station mutates state without calling LLM
"""
from __future__ import annotations

import pytest

from codebus_agent.sandbox import PathEscapeError, ToolContext


class _BlowUpOnCall:
    """Any attribute access triggers a call; all calls raise, proving no LLM was invoked."""

    def __getattr__(self, item):
        raise AssertionError(
            f"mark_station illegally reached into LLM provider (attr={item!r})"
        )


async def test_mark_station_appends_to_state_without_llm(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    # Tuck a blow-up provider onto the context-adjacent state so any LLM
    # dispatch raises loudly. mark_station MUST NOT reach it.
    explorer_state.messages = []
    assert len(explorer_state.stations) == 0

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    result = await tools.mark_station("app.py", "entry", "main handler")

    assert result is None
    assert len(explorer_state.stations) == 1
    s = explorer_state.stations[0]
    assert s.path == "app.py"
    assert s.role == "entry"
    assert s.why == "main handler"
    assert s.relevance == 0.8  # P0 hardcoded default


async def test_mark_station_is_idempotent_for_identical_inputs(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)

    await tools.mark_station("app.py", "entry", "seed")
    await tools.mark_station("app.py", "entry", "seed")

    assert len(explorer_state.stations) == 1, (
        f"identical mark_station calls MUST collapse to one entry; "
        f"got {[s.path for s in explorer_state.stations]}"
    )


async def test_mark_station_out_of_workspace_path_rejected(
    tool_context: ToolContext, explorer_state
) -> None:
    from codebus_agent.agent.tools.folder_tools import FolderTools

    tools = FolderTools(ctx=tool_context, state=explorer_state)
    pre_count = len(explorer_state.stations)

    with pytest.raises(PathEscapeError):
        await tools.mark_station("../../etc/passwd", "entry", "escape attempt")

    assert len(explorer_state.stations) == pre_count
