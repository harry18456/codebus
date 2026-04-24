"""RED tests for the Explorer ReAct main loop.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReAct loop executes think-act-observe-judge-log-update each iteration
  Requirement: Explorer Think step validates ExplorerAction via Instructor
  Requirement: Explorer loop stops on budget exhaustion, empty queue, or cancel signal

Spies keep the tests small:

- ``_CountingJudge`` / ``_CountingCoverage`` tally invocations + return
  canned verdicts so the loop converges deterministically.
- ``_RecordingLogger`` subclasses the real ``ReasoningLogger`` so the
  on-disk JSONL is produced AND we can count writes in-memory.
- ``_inner_chat_spy`` wraps the ``MockProvider`` inside the ``TrackedProvider``
  so tests can verify ``provider.chat(..., response_model=ExplorerAction)``
  is the only call shape reaching the wire (spec scenario `Think returns
  validated ExplorerAction instance`).

All tests route Explorer through a real ``TrackedProvider`` because the
spec forbids Explorer from bypassing the tracking wrapper (scenario
`Think rejects raw (untracked) providers at call-site`).
"""
from __future__ import annotations

import asyncio
from collections.abc import Callable
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


# ----------------------- spies ---------------------------------------------


class _CountingJudge:
    def __init__(self, verdict_factory: Callable[[int], Any]) -> None:
        self.calls: list[tuple[int, int]] = []  # (step_count, n_results)
        self._verdict_factory = verdict_factory

    async def evaluate(self, state: Any, results: list[Any]) -> Any:
        self.calls.append((state.step_count, len(results)))
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
    """No-op tool surface — Explorer emits no tool_calls in the default path."""

    async def primary_search(self, query: str) -> list[Any]:
        return []

    async def fetch(self, target: Any) -> Any:
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:
        return []


class _EchoTools:
    """Every tool-name maps to a coroutine that echoes its kwargs as JSON."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, dict]] = []

    async def echo(self, **kwargs: Any) -> str:
        self.calls.append(("echo", kwargs))
        return f"echo:{kwargs.get('msg', '')}"


class _BoomTools:
    """`fetch` raises; loop MUST wrap the error into ToolResult.error."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    async def fetch(self, **kwargs: Any) -> Any:
        self.calls.append("fetch")
        raise RuntimeError("tool detonated")


# ----------------------- helpers -------------------------------------------


def _push_actions(script: MockScript, actions: list[Any]) -> None:
    for a in actions:
        script.push(a)


def _make_judge_verdict() -> Any:
    from codebus_agent.agent.types import JudgeVerdict

    return JudgeVerdict(
        relevance=0.5,
        should_follow_imports=False,
        should_add_station=False,
        reason="fine",
    )


def _wrap_inner_chat_spy(provider: TrackedProvider) -> list[dict]:
    """Record the kwargs every inner.chat call receives (for assertions)."""
    captured: list[dict] = []
    original = provider._inner.chat

    async def wrapped(messages: Any, *, response_model: Any, **kwargs: Any) -> Any:
        captured.append(
            {
                "messages": list(messages),
                "response_model": response_model,
                "extra_kwargs": kwargs,
            }
        )
        return await original(messages, response_model=response_model, **kwargs)

    provider._inner.chat = wrapped  # type: ignore[method-assign]
    return captured


# ----------------------- tests (Section 10 RED) ----------------------------


async def test_each_iteration_executes_think_act_observe_judge_log_update(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Each iteration writes exactly one Step line`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    n = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"think {i}", tool_calls=[], stop=False)
            for i in range(n)
        ],
    )

    judge = _CountingJudge(lambda _s: _make_judge_verdict())
    coverage = _CountingCoverage()
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")

    state = ExplorerState(
        task="explore", budget_steps_left=n, budget_tokens_left=10_000
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=judge,
        coverage=coverage,
        logger=logger,
    )

    # Think ran N times → script queue drained.
    assert mock_script_reasoning.empty
    # Judge + logger fired N times, Step.step = 0..N-1.
    assert len(judge.calls) == n
    assert len(logger.writes) == n
    assert [s.step for s in logger.writes] == list(range(n))
    # Each Step carries a judge_verdict (Judge ran before Log).
    assert all(s.judge_verdict is not None for s in logger.writes)
    # Budget hit zero → stopped_reason = budget_exhausted.
    assert result.stopped_reason == "budget_exhausted"


async def test_explorer_think_validates_explorer_action_via_instructor(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Think returns validated ExplorerAction instance`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    _push_actions(
        mock_script_reasoning,
        [ExplorerAction(thought="pinned", tool_calls=[], stop=False)],
    )

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # `provider.chat` was called at least once with response_model=ExplorerAction.
    assert captured, "inner.chat MUST be reached by _think"
    shapes = {c["response_model"] for c in captured}
    assert ExplorerAction in shapes, (
        f"Think MUST pass response_model=ExplorerAction; saw {shapes}"
    )
    # The validated action shows up as the Step's thought.
    assert logger.writes[0].thought == "pinned"


async def test_observations_feed_forward_into_next_think(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Observations feed forward into next Think call`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, ToolCall

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    first = ExplorerAction(
        thought="call echo",
        tool_calls=[ToolCall(id="tc_1", name="echo", arguments={"msg": "hello"})],
        stop=False,
    )
    second = ExplorerAction(thought="done", tool_calls=[], stop=False)
    _push_actions(mock_script_reasoning, [first, second])

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=2, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_EchoTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    assert len(captured) >= 2
    second_messages = captured[1]["messages"]
    # Role `tool` with content reflecting the echo output.
    tool_contents = [
        m.content for m in second_messages if getattr(m, "role", None) == "tool"
    ]
    assert tool_contents, (
        f"iteration K+1 Think MUST see role='tool' messages; saw roles="
        f"{[getattr(m, 'role', None) for m in second_messages]}"
    )
    assert any("hello" in c for c in tool_contents), (
        f"tool message MUST reflect first iteration's output; saw {tool_contents}"
    )


async def test_tool_errors_do_not_crash_loop(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Tool errors do not crash the loop`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, ToolCall

    first = ExplorerAction(
        thought="try fetch",
        tool_calls=[ToolCall(id="tc_1", name="fetch", arguments={"target": "x"})],
        stop=False,
    )
    second = ExplorerAction(thought="continue", tool_calls=[], stop=False)
    _push_actions(mock_script_reasoning, [first, second])

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=2, budget_tokens_left=10_000)
    tools = _BoomTools()

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=tools,
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # Loop survived the tool error and completed both iterations.
    assert len(logger.writes) == 2
    # First step's tool_results carries the captured error.
    first_step = logger.writes[0]
    assert first_step.tool_results, "first iteration MUST record a ToolResult"
    failed = first_step.tool_results[0]
    assert failed.error is not None
    assert "tool detonated" in failed.error or "detonated" in (failed.output or "")
    assert result.stopped_reason == "budget_exhausted"
    assert tools.calls == ["fetch"]


async def test_coverage_recursion_hook_remains_dormant_in_p0(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Coverage recursion hook remains dormant in P0`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, Gap

    n = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"t{i}", tool_calls=[], stop=False)
            for i in range(n)
        ],
    )
    # A Coverage checker returning LOTS of gaps — must NOT trigger recursion.
    heavy_coverage = _CountingCoverage(
        gaps=[Gap(description=f"gap {i}") for i in range(20)]
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=n, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=heavy_coverage,
        logger=logger,
    )

    # Total iterations == N. If recursion fired, writes would exceed N.
    assert len(logger.writes) == n, (
        f"coverage recursion leaked — expected {n} writes, got {len(logger.writes)}"
    )
    # Coverage.check MAY be called 0 or 1 time (dormant hook); MUST NOT be
    # invoked in a recursive branch.
    assert heavy_coverage.calls <= 1


async def test_budget_exhaustion_terminates_loop(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Budget exhaustion terminates loop`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=0, budget_tokens_left=10_000)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    assert captured == [], "_think MUST NOT be invoked when budget starts at 0"
    assert logger.writes == []
    assert result.stopped_reason == "budget_exhausted"


async def test_cancel_event_short_circuits_mid_run(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Cancel event short-circuits mid-run`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    cancel = asyncio.Event()
    k = 2  # run 2 iterations, then cancel

    class _CancellingLogger(_RecordingLogger):
        def write(self, step: Any) -> None:
            super().write(step)
            if len(self.writes) >= k:
                cancel.set()

    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"t{i}", tool_calls=[], stop=False)
            for i in range(5)
        ],
    )

    logger = _CancellingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=5, budget_tokens_left=10_000)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        cancel_event=cancel,
    )

    assert result.stopped_reason == "cancelled"
    assert len(logger.writes) == k, (
        f"Cancel MUST abort before the next Think; got {len(logger.writes)} writes"
    )


async def test_queue_empty_with_enough_stations_terminates_cleanly(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Queue empty + enough stations terminates cleanly`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState, Station

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    pre_stations = [
        Station(path=f"s{i}.py", role="entry", relevance=0.7, why="seed", depends_on=[])
        for i in range(3)
    ]
    state = ExplorerState(
        task="t",
        budget_steps_left=5,
        budget_tokens_left=10_000,
        pending_queue=[],  # empty from the start
        stations=pre_stations,
    )

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    # Initial state already satisfies queue_empty convergence → no iterations.
    assert captured == [], (
        "queue_empty + stations>=MIN MUST short-circuit before first Think"
    )
    assert logger.writes == []
    assert result.stopped_reason == "queue_empty"


async def test_run_explorer_falls_back_to_empty_tool_specs_when_absent(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """`_DummyTools` lacks `tool_specs()` — run_explorer MUST fall back to [].

    Backs explorer-tools-p0 spec scenario
    `tool_specs method is optional on ExplorerTools`.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    captured = _wrap_inner_chat_spy(mock_reasoning_provider)
    _push_actions(
        mock_script_reasoning,
        [ExplorerAction(thought="t0", tool_calls=[], stop=False)],
    )

    dummy_tools = _DummyTools()
    assert not hasattr(dummy_tools, "tool_specs"), (
        "Precondition: _DummyTools MUST NOT carry tool_specs"
    )

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)

    # No AttributeError — run_explorer handles missing tool_specs gracefully.
    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=dummy_tools,
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    assert len(captured) == 1  # _think fired exactly once
    assert len(logger.writes) == 1
