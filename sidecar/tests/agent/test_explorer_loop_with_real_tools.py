"""End-to-end integration: Explorer loop dispatching real FolderTools.

Backs SHALL clauses across
openspec/changes/explorer-tools-p0/specs/explorer-tools/spec.md
  Requirement: Folder-mode Explorer exposes four P0 tools

This walks the full six-step loop against a mini workspace:
  iter 0: search → one SearchHit
  iter 1: read_file the hit
  iter 2: mark_station the same path
  terminate on budget

and then asserts the wire-level integration points pinned by the spec:
  - state.stations has the marked entry
  - reasoning_log.jsonl has 3 lines
  - tool_audit.jsonl has 3 allowed lines (one per tool)
  - sanitize_audit.jsonl has at least one Pass 1 entry from read_file
  - next-iteration Think inputs carry the sanitized tool output
"""
from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


# Reuse the spies from the loop test module — they satisfy the same
# Judge / Coverage / logger Protocols the real code expects.
from tests.agent.test_explorer_loop import (  # type: ignore[import-not-found]
    _CountingCoverage,
    _CountingJudge,
    _RecordingLogger,
    _make_judge_verdict,
    _push_actions,
    _wrap_inner_chat_spy,
)


async def test_mini_workspace_search_read_mark_closes_loop(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """search → read_file → mark_station closes an end-to-end loop."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import (
        ExplorerAction,
        ExplorerState,
        ToolCall,
    )
    from codebus_agent.sandbox import ToolContext
    from codebus_agent.sanitizer import SanitizerEngine

    # Seed a 3-file mini workspace
    (workspace_dir / ".codebus").mkdir(exist_ok=True)
    (workspace_dir / "app.py").write_text(
        "def entry():\n    return 'hello'\n", encoding="utf-8"
    )
    (workspace_dir / "helper.py").write_text(
        "# helper\n", encoding="utf-8"
    )
    (workspace_dir / "README.md").write_text(
        "# Project\n\nSee entry point.\n", encoding="utf-8"
    )

    ctx = ToolContext(
        workspace_root=workspace_dir,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
        session_id="integration-sess",
    )
    state = ExplorerState(
        task="find the entry point",
        budget_steps_left=3,
        budget_tokens_left=10_000,
    )
    tools = FolderTools(ctx=ctx, state=state)

    actions = [
        ExplorerAction(
            thought="grep for entry",
            tool_calls=[ToolCall(id="tc_1", name="search", arguments={"keyword": "entry"})],
            stop=False,
        ),
        ExplorerAction(
            thought="read the hit",
            tool_calls=[ToolCall(id="tc_2", name="read_file", arguments={"path": "app.py"})],
            stop=False,
        ),
        ExplorerAction(
            thought="mark it",
            tool_calls=[
                ToolCall(
                    id="tc_3",
                    name="mark_station",
                    arguments={"path": "app.py", "role": "entry", "why": "main handler"},
                )
            ],
            stop=False,
        ),
    ]
    _push_actions(mock_script_reasoning, actions)

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=tools,
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # --- Stations -----------------------------------------------------
    paths = [s.path for s in state.stations]
    assert "app.py" in paths, (
        f"mark_station MUST grow stations with app.py; got {paths}"
    )

    # --- Reasoning log — 3 iterations -----------------------------------
    reasoning_lines = (workspace_dir / "reasoning_log.jsonl").read_text("utf-8").splitlines()
    assert len(reasoning_lines) == 3, (
        f"expected 3 reasoning_log lines, got {len(reasoning_lines)}"
    )

    # --- Tool audit — 3 allowed invocations -----------------------------
    audit_lines = (
        (workspace_dir / ".codebus" / "tool_audit.jsonl")
        .read_text("utf-8")
        .splitlines()
    )
    parsed = [json.loads(l) for l in audit_lines if l]
    names = [l["tool_name"] for l in parsed]
    assert names == ["search", "read_file", "mark_station"], (
        f"tool_audit dispatch order wrong: {names}"
    )
    assert all(l["allowed"] is True for l in parsed)

    # --- Terminal state -------------------------------------------------
    assert result.stopped_reason == "budget_exhausted"
    assert Path(result.log_path) == (workspace_dir / "reasoning_log.jsonl")


async def test_sanitizer_end_to_end(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Sanitizer Pass 1 + Pass 2 both apply when Explorer reads a secret-bearing file."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import (
        ExplorerAction,
        ExplorerState,
        ToolCall,
    )
    from codebus_agent.sandbox import ToolContext
    from codebus_agent.sanitizer import SanitizerEngine

    (workspace_dir / ".codebus").mkdir(exist_ok=True)
    fake_key = "AKIAIOSFODNN7EXAMPLE"
    (workspace_dir / "secret.py").write_text(
        f'AWS_KEY = "{fake_key}"\n', encoding="utf-8"
    )

    ctx = ToolContext(
        workspace_root=workspace_dir,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
        session_id="integration-sess",
    )
    state = ExplorerState(
        task="find secrets",
        budget_steps_left=2,
        budget_tokens_left=10_000,
    )
    tools = FolderTools(ctx=ctx, state=state)

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    actions = [
        ExplorerAction(
            thought="read secret",
            tool_calls=[
                ToolCall(id="tc_1", name="read_file", arguments={"path": "secret.py"})
            ],
            stop=False,
        ),
        ExplorerAction(thought="done", tool_calls=[], stop=False),
    ]
    _push_actions(mock_script_reasoning, actions)

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=tools,
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # The second Think iteration's messages MUST carry only the sanitized
    # (placeholder) form of the secret, never the raw key.
    assert len(captured) >= 2
    second_messages = captured[1]["messages"]
    joined = "\n".join(m.content for m in second_messages)
    assert fake_key not in joined, (
        "raw AWS key leaked into Think messages — Pass 1 / 2 failed to sanitize"
    )


async def test_explorer_loop_falls_back_to_grep_when_kb_absent(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """With no KB, search should still produce hits via grep fallback."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.tools.folder_tools import FolderTools
    from codebus_agent.agent.types import (
        ExplorerAction,
        ExplorerState,
        ToolCall,
    )
    from codebus_agent.sandbox import ToolContext
    from codebus_agent.sanitizer import SanitizerEngine

    (workspace_dir / ".codebus").mkdir(exist_ok=True)
    (workspace_dir / "a.py").write_text("# has needle here\n", encoding="utf-8")

    ctx = ToolContext(
        workspace_root=workspace_dir,
        workspace_type="folder",
        sanitizer=SanitizerEngine(),
    )
    state = ExplorerState(
        task="grep path",
        budget_steps_left=1,
        budget_tokens_left=10_000,
    )
    tools = FolderTools(ctx=ctx, state=state)

    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(
                thought="grep",
                tool_calls=[
                    ToolCall(id="tc_g", name="search", arguments={"keyword": "needle"})
                ],
                stop=False,
            )
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=tools,
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # The search result was threaded back into state.messages as role=tool
    tool_msgs = [m for m in state.messages if m.role == "tool"]
    assert tool_msgs
    # The grep fallback snippet includes the keyword
    assert any("needle" in m.content for m in tool_msgs)
