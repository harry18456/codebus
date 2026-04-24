"""RED tests for run_explorer coverage-gap recursion.

Backs SHALL clauses in
openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: Coverage-gap recursion runs after main loop convergence

and openspec/changes/coverage-gap-recurse/specs/explorer-sse/spec.md
  Requirement: Coverage round emits coverage_gaps SSE event

Section 4 pins the recursion behaviour:
  - empty gaps terminate without recursion (no extra Step, no enqueue)
  - non-empty gaps + budget trigger exactly one round of recursion
  - `_depth` at MAX cap halts further recursion
  - `budget_steps_left == 0` halts recursion even with gaps present
  - `Gap.suggested_target is None` → placeholder `gap:<desc[:80]>`
  - innermost `stopped_reason` propagates through the recursive return
    chain

Section 6 pins the `coverage_gaps` SSE event schema and its
`skip_reason` precedence.

Test doubles:
  - `_RecordingLogger` wraps `ReasoningLogger` so we can count Step
    writes AND keep the on-disk JSONL for drift inspection.
  - `_DummyTools` / `_CountingJudge` mirror `test_explorer_loop.py`
    patterns — the loop emits no tool_calls, Judge returns a neutral
    verdict every iteration.
  - `scripted_coverage_checker` comes from the session conftest and
    supports a `gap_queue` so tests can stage "first round reports
    gaps, subsequent rounds report none" without a stateful inline
    mock.
  - `_SpyEmitter` captures emitted SSE events for Section 6 assertions.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


# ----------------------- spies ---------------------------------------------


class _SpyEmitter:
    """Structural `SSEEmitter` conformer."""

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


# ----------------------- helpers -------------------------------------------


def _push_actions(script: MockScript, actions: list[Any]) -> None:
    for a in actions:
        script.push(a)


def _make_neutral_verdict() -> Any:
    from codebus_agent.agent.types import JudgeVerdict

    return JudgeVerdict(
        relevance=0.5,
        should_follow_imports=False,
        should_add_station=False,
        reason="neutral",
    )


def _pre_state_with_queue_empty_ready(
    budget: int,
    *,
    pending_queue: list[str] | None = None,
) -> Any:
    """Build an ExplorerState that (by default) converges on queue_empty.

    Three pre-populated stations satisfy `_MIN_STATIONS_FOR_CONVERGENCE`
    (=3) so with an empty `pending_queue` the outer while loop short-
    circuits on the first `_should_stop` check — no iterations, budget
    untouched, coverage round fires immediately.

    Setting `pending_queue=[...]` keeps the loop running until budget
    exhausts, which lets tests drive inner-recursion convergence.
    """
    from codebus_agent.agent.types import ExplorerState, Station

    return ExplorerState(
        task="t",
        budget_steps_left=budget,
        budget_tokens_left=10_000,
        stations=[
            Station(
                path=f"seed_{i}.py",
                role="entry",
                relevance=0.7,
                why="seed",
                depends_on=[],
            )
            for i in range(3)
        ],
        pending_queue=list(pending_queue) if pending_queue is not None else [],
    )


def _coverage_steps(writes: list[Any]) -> list[Any]:
    """Pick out the coverage-round Step lines from the recording logger."""
    return [s for s in writes if s.thought.startswith("[coverage]")]


# ----------------------- Section 4 RED -------------------------------------


async def test_empty_gaps_terminate_without_recursion(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Empty gaps terminate without recursion`.

    coverage.check returns [] → no recurse, no coverage Step written,
    `_enqueue_gap_investigation` NOT invoked (state.pending_queue +
    state.messages both unchanged).
    """
    from codebus_agent.agent.explorer import run_explorer

    coverage = scripted_coverage_checker(gaps=[])
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = _pre_state_with_queue_empty_ready(budget=5)
    pre_queue = list(state.pending_queue)
    pre_messages = list(state.messages)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
    )

    assert coverage.calls == 1, (
        f"coverage.check MUST be invoked exactly once after convergence; "
        f"saw {coverage.calls}"
    )
    assert _coverage_steps(logger.writes) == [], (
        "empty gaps MUST NOT emit a coverage Step (design Decision 8)"
    )
    # `_enqueue_gap_investigation` runs only when gaps are non-empty; assert
    # the two state slots it mutates stayed at their pre-call values.
    assert state.pending_queue == pre_queue
    assert state.messages == pre_messages
    # Outer converged via queue_empty at step 0.
    assert result.stopped_reason == "queue_empty"


async def test_gaps_with_budget_trigger_one_recursion_round(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Gaps with budget trigger one recursion round`.

    Outer converges on queue_empty (budget untouched). Coverage returns
    two gaps → recurse with `_depth=1`, `_enqueue_gap_investigation`
    mutates state (pending_queue += 2, messages += 1 role="user").
    Inner has non-empty queue → drains budget to 0 → budget_exhausted,
    inner coverage returns [] → no further recurse.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, Gap, Message

    gaps = [
        Gap(description="missing Adapter wiring path", suggested_target="src/adapter.py"),
        Gap(description="protocol boundary uncovered", suggested_target="src/protocol.py"),
    ]
    coverage = scripted_coverage_checker(gap_queue=[gaps, []])

    budget = 5
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"gap-round-iter-{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = _pre_state_with_queue_empty_ready(budget=budget)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
    )

    # Coverage runs once for outer + once for inner convergence.
    assert coverage.calls == 2

    # `_enqueue_gap_investigation` mutations — exactly two queue entries
    # and one user-role message corresponding to the single round.
    assert len(state.pending_queue) == 2
    user_msgs = [m for m in state.messages if m.role == "user"]
    assert len(user_msgs) == 1
    assert isinstance(user_msgs[0], Message)
    # Summary body should reference the number of gaps and at least one
    # of the gap descriptions (first three at most per design Decision 6).
    assert "Coverage" in user_msgs[0].content
    assert "2" in user_msgs[0].content
    assert "Adapter" in user_msgs[0].content

    # Exactly one coverage Step written for the single round (inner
    # convergence with empty gaps → no second Step per Decision 8).
    cov_steps = _coverage_steps(logger.writes)
    assert len(cov_steps) == 1
    assert cov_steps[0].thought.startswith("[coverage] round-1 gaps=2 will_recurse=True"), (
        f"coverage Step thought mismatch: {cov_steps[0].thought!r}"
    )

    # Inner loop consumed all pinned actions → script empty; budget hit 0.
    assert mock_script_reasoning.empty
    # Innermost stopped_reason (budget_exhausted) propagates up.
    assert result.stopped_reason == "budget_exhausted"


async def test_max_depth_halts_further_recursion(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Max depth halts further recursion`.

    Enter with `_depth=2` (deepest allowed level) + 1 gap + budget=10.
    MUST NOT recurse (would exceed cap); MUST still write the coverage
    Step marking `will_recurse=False`; `_enqueue_gap_investigation`
    MUST NOT fire.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import Gap

    coverage = scripted_coverage_checker(gaps=[Gap(description="edge gap")])
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = _pre_state_with_queue_empty_ready(budget=10)
    pre_queue = list(state.pending_queue)
    pre_messages = list(state.messages)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        _depth=2,
    )

    # Coverage fires exactly once — no recursion into deeper frame.
    assert coverage.calls == 1
    # Step written, explicitly marking `will_recurse=False`.
    cov_steps = _coverage_steps(logger.writes)
    assert len(cov_steps) == 1
    assert cov_steps[0].thought.startswith("[coverage] round-3 gaps=1 will_recurse=False"), (
        f"coverage Step thought mismatch: {cov_steps[0].thought!r}"
    )
    # Enqueue never ran.
    assert state.pending_queue == pre_queue
    assert state.messages == pre_messages
    # Outer converged on queue_empty; there's no recursion, so stopped_reason
    # must still match outer convergence (not budget_exhausted).
    assert result.stopped_reason == "queue_empty"


async def test_budget_exhaustion_halts_recursion_even_with_gaps(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Budget exhaustion halts recursion even with gaps`.

    budget_steps_left=0 entering the function → main loop returns
    immediately on the `budget_exhausted` branch. Coverage returns 1
    gap, but precondition (2) `budget_steps_left > 0` fails → no
    recursion. Step MUST still be written with `will_recurse=False`;
    enqueue MUST NOT fire.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState, Gap

    coverage = scripted_coverage_checker(gaps=[Gap(description="late gap")])
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=10_000,
    )
    pre_queue = list(state.pending_queue)
    pre_messages = list(state.messages)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
    )

    assert coverage.calls == 1
    cov_steps = _coverage_steps(logger.writes)
    assert len(cov_steps) == 1
    assert cov_steps[0].thought.startswith("[coverage] round-1 gaps=1 will_recurse=False")
    # Enqueue did NOT fire — state slots unchanged.
    assert state.pending_queue == pre_queue
    assert state.messages == pre_messages
    # Outer already converged on budget exhaustion.
    assert result.stopped_reason == "budget_exhausted"


async def test_enqueue_gap_investigation_uses_placeholder_when_suggested_target_is_none(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `_enqueue_gap_investigation` placeholder rule.

    `Gap(description=..., suggested_target=None)` → pending_queue entry
    MUST be `f"gap:{description[:80]}"` exactly.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, Gap

    desc = "Storage adapter wiring is missing and may be the protocol boundary leaked across modules"
    gap = Gap(description=desc, suggested_target=None)
    coverage = scripted_coverage_checker(gap_queue=[[gap], []])

    budget = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"inner-{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = _pre_state_with_queue_empty_ready(budget=budget)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
    )

    assert state.pending_queue[-1] == f"gap:{desc[:80]}", (
        f"placeholder must be `gap:<desc[:80]>`; saw {state.pending_queue[-1]!r}"
    )


async def test_stopped_reason_propagates_through_recursion(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `innermost stopped_reason propagates`.

    Outer converges on queue_empty; inner recurses, drains budget,
    converges on budget_exhausted. Outermost return MUST surface the
    innermost `stopped_reason`.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, Gap

    coverage = scripted_coverage_checker(
        gap_queue=[[Gap(description="g")], []]
    )
    budget = 4
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"i{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = _pre_state_with_queue_empty_ready(budget=budget)

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
    )

    assert result.stopped_reason == "budget_exhausted", (
        f"outermost stopped_reason must reflect innermost convergence; "
        f"saw {result.stopped_reason!r}"
    )


# ----------------------- Section 6 RED -------------------------------------


async def test_coverage_gaps_event_fires_before_recursion(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `coverage_gaps event fires after check and before recursion`.

    Assertion order:
      1. Exactly one `coverage_gaps` event per coverage round.
      2. Event payload matches the pinned wire schema.
      3. Event is observed before the recursive frame's first
         `agent_thought` (which only exists because recursion fires).
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, Gap

    gaps = [
        Gap(description="adapter boundary", suggested_target="src/adapter.py"),
        Gap(description="protocol dispatch", suggested_target=None),
    ]
    coverage = scripted_coverage_checker(gap_queue=[gaps, []])

    budget = 4
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"inner-t{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    spy = _SpyEmitter()
    state = _pre_state_with_queue_empty_ready(budget=budget)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        emitter=spy,
    )

    cov_events = [e for e in spy.events if e.get("type") == "coverage_gaps"]
    # Exactly two coverage_gaps emits (outer round + inner empty round).
    assert len(cov_events) == 2, (
        f"expected two coverage_gaps events (one per round); saw {cov_events}"
    )

    outer = cov_events[0]
    assert outer["round"] == 0
    assert outer["will_recurse"] is True
    assert outer["skip_reason"] is None
    assert outer["gaps"] == [
        {"description": "adapter boundary", "suggested_target": "src/adapter.py"},
        {"description": "protocol dispatch", "suggested_target": None},
    ]

    # The outer coverage_gaps event must land BEFORE the inner frame's
    # first agent_thought (inner Think fires only when recursion enters).
    outer_idx = spy.events.index(outer)
    first_inner_thought_idx = next(
        (
            i
            for i, e in enumerate(spy.events)
            if e.get("type") == "agent_thought"
        ),
        None,
    )
    assert first_inner_thought_idx is not None, (
        "inner recursion frame must emit at least one agent_thought"
    )
    assert outer_idx < first_inner_thought_idx, (
        "coverage_gaps MUST be emitted before recursive agent_thought"
    )


async def test_coverage_gaps_event_no_gaps_skip_reason(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Empty gaps still emit with skip_reason="no_gaps"`."""
    from codebus_agent.agent.explorer import run_explorer

    coverage = scripted_coverage_checker(gaps=[])
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    spy = _SpyEmitter()
    state = _pre_state_with_queue_empty_ready(budget=5)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        emitter=spy,
    )

    cov_events = [e for e in spy.events if e.get("type") == "coverage_gaps"]
    assert len(cov_events) == 1
    ev = cov_events[0]
    assert ev["gaps"] == []
    assert ev["will_recurse"] is False
    assert ev["skip_reason"] == "no_gaps"


async def test_coverage_gaps_event_budget_exhausted_skip_reason(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Budget-exhausted round emits skip_reason="budget_exhausted"`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState, Gap

    coverage = scripted_coverage_checker(
        gaps=[Gap(description="late gap", suggested_target=None)]
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    spy = _SpyEmitter()
    state = ExplorerState(
        task="t",
        budget_steps_left=0,
        budget_tokens_left=10_000,
    )

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        emitter=spy,
    )

    cov_events = [e for e in spy.events if e.get("type") == "coverage_gaps"]
    assert len(cov_events) == 1
    ev = cov_events[0]
    assert ev["will_recurse"] is False
    assert ev["skip_reason"] == "budget_exhausted"
    assert len(ev["gaps"]) == 1


async def test_coverage_gaps_event_max_depth_skip_reason(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Max-depth round emits skip_reason="max_depth_reached"`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import Gap

    coverage = scripted_coverage_checker(
        gaps=[Gap(description="deep gap")]
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    spy = _SpyEmitter()
    state = _pre_state_with_queue_empty_ready(budget=10)

    await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        emitter=spy,
        _depth=2,
    )

    cov_events = [e for e in spy.events if e.get("type") == "coverage_gaps"]
    assert len(cov_events) == 1
    ev = cov_events[0]
    assert ev["round"] == 2
    assert ev["will_recurse"] is False
    assert ev["skip_reason"] == "max_depth_reached"
    assert len(ev["gaps"]) == 1


async def test_coverage_gaps_event_suppressed_when_emitter_none(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
) -> None:
    """Spec scenario `Missing emitter preserves legacy behavior`.

    With `emitter=None`, recursion decisions are unchanged but no SSE
    event fires anywhere (proven via spy NOT being referenced by the
    loop). This keeps the file-only path clean for golden-sample
    replay.
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, Gap

    coverage = scripted_coverage_checker(
        gap_queue=[[Gap(description="g1"), Gap(description="g2")], []]
    )
    budget = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"inner-{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    spy = _SpyEmitter()
    state = _pre_state_with_queue_empty_ready(budget=budget)

    # emitter=None → spy MUST stay empty even after recursion.
    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_neutral_verdict()),
        coverage=coverage,
        logger=logger,
        emitter=None,
    )

    assert spy.events == [], (
        "no emitter supplied — spy MUST stay empty; any event means the "
        "loop leaked emission outside the emitter-opt-in path"
    )
    # Recursion still happened (coverage.calls == 2) and the inner loop
    # drained budget even though no SSE channel was wired.
    assert coverage.calls == 2
    assert result.stopped_reason == "budget_exhausted"
