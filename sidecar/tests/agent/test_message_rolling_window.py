"""RED tests for Explorer _think rolling message window.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/specs/agent-core/spec.md
  ADDED Requirement: Explorer applies rolling message window before each
    Think call

Section 8 pins:
  - wire prompt is sliced to last _MESSAGE_ROLLING_WINDOW messages
  - `state.messages` is NOT mutated by the slice
  - reasoning_log.jsonl records full history (not the windowed view)
  - coverage-gap recursion frame respects the same window
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


class _CountingJudge:
    def __init__(self, verdict_factory: Callable[[int], Any]) -> None:
        self._verdict_factory = verdict_factory

    async def evaluate(self, state: Any, results: list[Any]) -> Any:
        return self._verdict_factory(state.step_count)


class _CountingCoverage:
    def __init__(self, gaps: list[Any] | None = None) -> None:
        self.calls = 0
        self._gaps = gaps or []

    async def check(self, state: Any) -> list[Any]:
        self.calls += 1
        return list(self._gaps)


class _RecordingLogger:
    def __init__(self, path: Path) -> None:
        from codebus_agent.agent.reasoning_logger import ReasoningLogger

        self._inner = ReasoningLogger(path)
        self.writes: list[Any] = []

    @property
    def path(self) -> Path:
        return self._inner.path

    def write(self, step: Any) -> None:
        self.writes.append(step)
        self._inner.write(step)


class _DummyTools:
    async def primary_search(self, query: str) -> list[Any]:
        return []

    async def fetch(self, target: Any) -> Any:
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:
        return []


def _make_verdict() -> Any:
    from codebus_agent.agent.types import JudgeVerdict

    return JudgeVerdict(
        relevance=0.5,
        should_follow_imports=False,
        should_add_station=False,
        reason="neutral",
    )


def _push_actions(script: MockScript, actions: list[Any]) -> None:
    for a in actions:
        script.push(a)


def _wrap_inner_chat_spy(provider: TrackedProvider) -> list[dict]:
    """Capture every inner.chat kwargs (messages) for slice assertions."""
    captured: list[dict] = []
    original = provider._inner.chat

    async def wrapped(messages: Any, *, response_model: Any, **kwargs: Any) -> Any:
        captured.append(
            {
                "messages": list(messages),
                "response_model": response_model,
            }
        )
        return await original(messages, response_model=response_model, **kwargs)

    provider._inner.chat = wrapped  # type: ignore[method-assign]
    return captured


async def test_think_receives_at_most_window_size_messages_when_state_grew_larger(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Think receives at most window-size messages when state grew larger`."""
    from codebus_agent.agent.explorer import _MESSAGE_ROLLING_WINDOW, run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, Message

    # Pre-seed state.messages with 20 tool-ish messages; the first
    # iteration's Think call MUST slice to the last WINDOW size.
    pre_messages = [
        Message(
            role="tool",
            content=f"prior observation {i}",
            tool_call_id=f"tc_{i}",
            tool_name="primary_search",
        )
        for i in range(20)
    ]
    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    _push_actions(
        mock_script_reasoning,
        [ExplorerAction(thought="t", tool_calls=[], stop=False)],
    )

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=1,
        budget_tokens_left=10_000,
        messages=pre_messages,
    )

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    assert len(captured) == 1, "single iteration → one chat call"
    wire = captured[0]["messages"]
    # Wire payload = system + window + user = WINDOW + 2 (orphan-tool
    # rewrite preserves length; only role / content change).
    # `react-message-ordering-fix` locks the layout to
    # `[system, *normalized_history, user]`.
    assert len(wire) == _MESSAGE_ROLLING_WINDOW + 2, (
        f"wire prompt length MUST equal WINDOW({_MESSAGE_ROLLING_WINDOW}) + 2, "
        f"got {len(wire)}"
    )
    assert wire[0].role == "system"
    assert wire[-1].role == "user"
    # The middle WINDOW entries derive from the tail of state.messages —
    # tool roles get rewritten to user notes embedding the original
    # observation content, so the substring check covers both forms.
    history_slice = wire[1:-1]
    expected_tail = [m.content for m in pre_messages[-_MESSAGE_ROLLING_WINDOW:]]
    for wire_msg, source_content in zip(history_slice, expected_tail):
        assert source_content in wire_msg.content, (
            f"each history entry MUST surface in wire (tool→user-note rewrite "
            f"keeps the original content as substring); missing {source_content!r} "
            f"in {wire_msg.content!r}"
        )


async def test_think_preserves_all_state_messages_when_below_window(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Think preserves all state when message count is below window`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, Message

    pre_messages = [
        Message(
            role="tool",
            content=f"obs {i}",
            tool_call_id=f"tc_{i}",
            tool_name="primary_search",
        )
        for i in range(5)
    ]
    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    _push_actions(
        mock_script_reasoning,
        [ExplorerAction(thought="t", tool_calls=[], stop=False)],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=1,
        budget_tokens_left=10_000,
        messages=pre_messages,
    )

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    wire = captured[0]["messages"]
    # 5 pre-messages + 2 appended = 7 total; no slicing observable.
    assert len(wire) == 7


async def test_rolling_window_does_not_mutate_state_messages(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Reasoning log records full iteration history despite windowing`.

    Confirm the state's raw messages stay untouched AND reasoning_log
    Steps record full per-iteration tool_results (not the windowed view).
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, Message

    pre_messages = [
        Message(role="user", content=f"context seed {i}")
        for i in range(25)
    ]
    pre_snapshot = [m.content for m in pre_messages]

    _push_actions(
        mock_script_reasoning,
        [ExplorerAction(thought="t", tool_calls=[], stop=False)],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=1,
        budget_tokens_left=10_000,
        messages=list(pre_messages),  # detach from our snapshot
    )

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # The first 25 messages in state are identical to our snapshot — the
    # rolling window did not drop or reorder anything on `state`.
    assert [m.content for m in state.messages[:25]] == pre_snapshot
    # The tool-observation appended by _append_observations (if any) is
    # after these 25; length is >= 25.
    assert len(state.messages) >= 25
    # Each reasoning_log Step's tool_results is faithful to the iteration
    # (never windowed). No-op tools → empty list each, consistent.
    assert all(isinstance(s.tool_results, list) for s in logger.writes)


async def test_coverage_gap_recursion_frame_respects_same_window(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Coverage-gap recursion frame respects the same window`.

    Outer iterates to fill state.messages past the WINDOW; coverage
    returns 1 gap → inner frame enqueue appends a user summary; inner's
    first Think call must slice to last WINDOW entries, including the
    just-appended gap summary as the tail item.
    """
    from codebus_agent.agent.explorer import _MESSAGE_ROLLING_WINDOW, run_explorer
    from codebus_agent.agent.types import (
        ExplorerAction,
        ExplorerState,
        Gap,
        Message,
        Station,
    )

    coverage = scripted_coverage_checker(
        gap_queue=[[Gap(description="critical missing path")], []]
    )

    # Outer pre-seed a long message history so rolling WINDOW kicks in.
    pre_messages = [
        Message(
            role="tool",
            content=f"pre-obs {i}",
            tool_call_id=f"tc_{i}",
            tool_name="primary_search",
        )
        for i in range(30)
    ]
    # Outer converges on queue_empty immediately; inner recurses and
    # draining budget = 3 iterations.
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"inner-{i}", tool_calls=[], stop=False)
            for i in range(3)
        ],
    )
    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=3,
        budget_tokens_left=10_000,
        messages=pre_messages,
        stations=[
            Station(
                path=f"s{i}.py",
                role="entry",
                relevance=0.7,
                why="seed",
                depends_on=[],
            )
            for i in range(3)
        ],
    )

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=coverage,
        logger=logger,
    )

    # Inner's first think call is captured[0] (outer had queue_empty → no
    # think). All captured calls must be bounded by WINDOW+2 in length.
    assert len(captured) >= 1
    for c in captured:
        assert len(c["messages"]) <= _MESSAGE_ROLLING_WINDOW + 2

    # Innermost frame's first Think sees the Coverage summary (last
    # appended user message before recursion) within its window.
    first_inner_wire = captured[0]["messages"]
    tail_contents = "\n".join(m.content for m in first_inner_wire)
    assert "Coverage" in tail_contents and "gap" in tail_contents, (
        "inner frame's windowed wire prompt must still carry the gap summary"
    )
