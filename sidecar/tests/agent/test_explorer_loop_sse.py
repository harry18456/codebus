"""RED tests for Explorer loop SSE emits (agent-sse-wiring §4).

Backs openspec/changes/agent-sse-wiring/specs/explorer-sse/spec.md
  Requirement: Explorer loop emits agent_thought / agent_action_result /
               judge_verdict events

Test doubles mirror those in `test_explorer_loop.py`:
  * `_CountingJudge` / `_CountingCoverage` for deterministic convergence
  * `_RecordingLogger` wraps the real `ReasoningLogger` so we can assert
    the full tool output still lands on disk after observation truncation.
  * `_SpyEmitter` captures every emitted event in order; structurally
    satisfies the `SSEEmitter` Protocol without inheriting it.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


# ----------------------- spies ---------------------------------------------


class _SpyEmitter:
    """Capture every emitted event in call order.

    Structurally conforms to `SSEEmitter` Protocol — no nominal inheritance.
    """

    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


class _CountingJudge:
    def __init__(self, verdict_factory: Callable[[int], Any]) -> None:
        self.calls: list[tuple[int, int]] = []
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
    """No-op surface — Explorer emits no tool_calls on the default path."""

    async def primary_search(self, query: str) -> list[Any]:
        return []

    async def fetch(self, target: Any) -> Any:
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:
        return []


class _EchoTools:
    """`echo(msg=...)` returns the arg as a string; used to assert
    `agent_action_result.observation` carries tool output.
    """

    async def echo(self, **kwargs: Any) -> str:
        return f"echo:{kwargs.get('msg', '')}"


class _FloodTools:
    """`flood()` returns a 10_000-char payload — drives the truncation check."""

    async def flood(self, **kwargs: Any) -> str:
        return "A" * 10_000


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


# ----------------------- tests ---------------------------------------------


async def test_three_event_types_fire_per_iteration_in_order(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Three event types fire per iteration in order`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, ToolCall

    action = ExplorerAction(
        thought="look around",
        tool_calls=[ToolCall(id="tc_1", name="echo", arguments={"msg": "hi"})],
        stop=False,
    )
    _push_actions(mock_script_reasoning, [action])

    emitter = _SpyEmitter()
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_EchoTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        emitter=emitter,
    )

    types_in_order = [e["type"] for e in emitter.events]
    # In a single iteration we must see: agent_thought, agent_action_result, judge_verdict.
    # Progress event is also emitted (see progress-event test) — we filter to the 3 mandatory.
    core_indices = [
        i
        for i, t in enumerate(types_in_order)
        if t in {"agent_thought", "agent_action_result", "judge_verdict"}
    ]
    core_types = [types_in_order[i] for i in core_indices]
    assert core_types == ["agent_thought", "agent_action_result", "judge_verdict"], (
        f"expected agent_thought → agent_action_result → judge_verdict per iteration; got {types_in_order}"
    )

    # Every mandatory event MUST carry step == state.step_count at iteration start (0).
    for i in core_indices:
        e = emitter.events[i]
        assert e.get("step") == 0, (
            f"event {e['type']} at index {i} MUST carry step=0; got {e}"
        )

    # agent_thought carries `thought` + `action` (list of tool calls serialized via model_dump).
    thought_evt = next(e for e in emitter.events if e["type"] == "agent_thought")
    assert thought_evt["thought"] == "look around"
    assert isinstance(thought_evt["action"], list)
    assert thought_evt["action"][0]["name"] == "echo"

    # agent_action_result carries `tool` + `observation`.
    action_evt = next(
        e for e in emitter.events if e["type"] == "agent_action_result"
    )
    assert action_evt["tool"] == "echo"
    assert "echo:hi" in action_evt["observation"]

    # judge_verdict carries `relevance` + `reason`.
    verdict_evt = next(e for e in emitter.events if e["type"] == "judge_verdict")
    assert verdict_evt["relevance"] == 0.5
    assert verdict_evt["reason"] == "fine"


async def test_missing_emitter_preserves_legacy_behavior(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Missing emitter preserves legacy behavior`.

    Calling `run_explorer(...)` without the `emitter` kwarg MUST NOT raise
    and MUST produce an identical `ExplorerResult` + identical `logger.writes`
    shape to the legacy (pre-SSE) form.
    """
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

    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=n, budget_tokens_left=10_000)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
    )

    assert len(logger.writes) == n
    assert result.stopped_reason == "budget_exhausted"


async def test_observation_truncation_bounds_channel_payload(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Observation truncation bounds channel payload`.

    Flood tool returns 10_000 chars; the emitted `agent_action_result.observation`
    MUST be ≤ 500 chars (plus truncation marker) while the `reasoning_log.jsonl`
    line MUST carry the full output verbatim.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState, ToolCall

    action = ExplorerAction(
        thought="flood",
        tool_calls=[ToolCall(id="tc_1", name="flood", arguments={})],
        stop=False,
    )
    _push_actions(mock_script_reasoning, [action])

    emitter = _SpyEmitter()
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=1, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_FloodTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        emitter=emitter,
    )

    action_evt = next(
        e for e in emitter.events if e["type"] == "agent_action_result"
    )
    # Spec: "at most 500 characters plus a truncation indicator".
    assert len(action_evt["observation"]) <= 600, (
        f"observation too long: {len(action_evt['observation'])}"
    )
    # The `reasoning_log.jsonl` line still carries the full 10_000-char output.
    first_step = logger.writes[0]
    assert len(first_step.tool_results) == 1
    assert len(first_step.tool_results[0].output) == 10_000, (
        "full tool output MUST land on disk verbatim (wire-log parity)"
    )


async def test_progress_event_also_fires_each_iteration(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Each iteration emits one `progress` event so the UI bar stays in sync."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    n = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"t{i}", tool_calls=[], stop=False)
            for i in range(n)
        ],
    )

    emitter = _SpyEmitter()
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(task="t", budget_steps_left=n, budget_tokens_left=10_000)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_judge_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        emitter=emitter,
    )

    progress_events = [e for e in emitter.events if e["type"] == "progress"]
    assert len(progress_events) == n, (
        f"expected {n} progress events (one per iteration), got {len(progress_events)}"
    )
    for p in progress_events:
        assert p["phase"] == "exploring"
        assert p["total"] == n, f"total MUST snapshot initial budget; got {p}"
    currents = [p["current"] for p in progress_events]
    # `current` reflects state.step_count at iteration end — Explorer increments
    # after each iteration so we expect 0→1→…→n-1 OR 1→2→…→n depending on
    # emit placement; spec says "each iteration" so we only assert monotonicity.
    assert currents == sorted(currents), f"progress `current` MUST be monotonic; got {currents}"
