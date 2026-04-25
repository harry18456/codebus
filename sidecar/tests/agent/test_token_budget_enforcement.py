"""RED tests for Explorer loop token-budget enforcement.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/specs/agent-core/spec.md
  MODIFIED Requirement: Explorer loop stops on budget exhaustion, empty
    queue, or cancel signal (four-branch + new token_probe precondition)

Section 6 pins:
  - token budget exhaustion collapses the loop on the next iteration
    with stopped_reason="budget_tokens_exhausted"
  - token_probe=None leaves token budget unenforced (legacy path)
  - precedence order: cancel > token > steps > queue
  - innermost stopped_reason propagates through coverage-gap recursion
"""
from __future__ import annotations

import asyncio
from collections.abc import Callable
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


# ----------------------- spies (mirror test_explorer_loop.py) --------------


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
    async def primary_search(self, query: str) -> list[Any]:
        return []

    async def fetch(self, target: Any) -> Any:
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:
        return []


def _push_actions(script: MockScript, actions: list[Any]) -> None:
    for a in actions:
        script.push(a)


def _make_verdict() -> Any:
    from codebus_agent.agent.types import JudgeVerdict

    return JudgeVerdict(
        relevance=0.5,
        should_follow_imports=False,
        should_add_station=False,
        reason="neutral",
    )


# ----------------------- Section 6 RED -------------------------------------


async def test_token_budget_exhaustion_terminates_loop(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_token_probe,
) -> None:
    """Spec scenario `Token budget exhaustion terminates loop`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    probe = scripted_token_probe(total=5_000)
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=10,
        budget_tokens_left=5_000,  # probe.total() >= budget → exhausted
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        token_probe=probe,
    )

    # _think never fires — probe flipped the budget before the first Think.
    assert mock_script_reasoning.empty
    assert logger.writes == []
    assert result.stopped_reason == "budget_tokens_exhausted"


async def test_missing_token_probe_leaves_budget_unenforced(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
) -> None:
    """Spec scenario `Missing token probe leaves token budget unenforced`."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerAction, ExplorerState

    budget = 3
    _push_actions(
        mock_script_reasoning,
        [
            ExplorerAction(thought=f"t{i}", tool_calls=[], stop=False)
            for i in range(budget)
        ],
    )
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    # budget_tokens_left is tiny; with token_probe=None it must NOT be enforced.
    state = ExplorerState(
        task="t",
        budget_steps_left=budget,
        budget_tokens_left=1,
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        # token_probe omitted (default None) — legacy behaviour.
    )

    assert result.stopped_reason == "budget_exhausted"  # step budget drained first
    assert len(logger.writes) == budget


async def test_precedence_token_budget_over_step_budget(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_token_probe,
) -> None:
    """Spec Decision 3 precedence: cancel > token > steps > queue.

    Both token budget AND step budget exhausted at loop entry → token
    wins the reason string (token branch evaluated first).
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    probe = scripted_token_probe(total=10_000)
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=0,      # step budget already drained
        budget_tokens_left=5_000, # probe.total() >= budget → also exhausted
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        token_probe=probe,
    )

    assert result.stopped_reason == "budget_tokens_exhausted"


async def test_cancel_still_wins_over_token_budget(
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_token_probe,
) -> None:
    """Spec Decision 3 precedence: cancel beats every other branch."""
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState

    cancel = asyncio.Event()
    cancel.set()  # already set at loop entry

    probe = scripted_token_probe(total=10_000)
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=10,
        budget_tokens_left=5_000,  # probe says exceeded too
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=_CountingCoverage(),
        logger=logger,
        cancel_event=cancel,
        token_probe=probe,
    )

    assert result.stopped_reason == "cancelled"


async def test_stopped_reason_propagates_through_coverage_recursion(
    mock_script_reasoning: MockScript,
    mock_reasoning_provider: TrackedProvider,
    workspace_dir: Path,
    scripted_coverage_checker,
    scripted_token_probe,
) -> None:
    """Innermost `budget_tokens_exhausted` propagates through tail-recursion.

    Outer converges on queue_empty (pre-populated stations). Coverage
    returns 1 gap → recurse into `_depth=1`. Inner frame's `_should_stop`
    fires on token probe at the very first iteration → no iterations,
    inner returns `budget_tokens_exhausted`. Outer returns inner's
    result directly (tail recursion, per coverage-gap-recurse design).
    """
    from codebus_agent.agent.explorer import run_explorer
    from codebus_agent.agent.types import ExplorerState, Gap, Station

    coverage = scripted_coverage_checker(gap_queue=[[Gap(description="g")], []])

    # Probe: first two checks (outer pre-think, outer coverage round) must
    # be under budget so outer can converge normally; third check (inner
    # first iteration) jumps past budget → inner converges on tokens.
    probe = scripted_token_probe(totals=[0, 0, 0, 10_000, 10_000])
    logger = _RecordingLogger(workspace_dir / "reasoning_log.jsonl")
    state = ExplorerState(
        task="t",
        budget_steps_left=5,
        budget_tokens_left=5_000,
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
        pending_queue=[],  # queue_empty convergence immediately
    )

    result = await run_explorer(
        state=state,
        provider=mock_reasoning_provider,
        tools=_DummyTools(),
        judge=_CountingJudge(lambda _s: _make_verdict()),
        coverage=coverage,
        logger=logger,
        token_probe=probe,
    )

    # Outer didn't iterate (queue_empty immediately).
    # Coverage returned 1 gap → recursed.
    # Inner's _should_stop hit token budget → stopped_reason="budget_tokens_exhausted".
    # Outer returns inner's ExplorerResult unchanged.
    assert result.stopped_reason == "budget_tokens_exhausted"
    assert coverage.calls >= 1  # at least outer coverage round fired


def test_explorer_result_stopped_reason_literal_includes_budget_tokens_exhausted() -> None:
    """Spec `ExplorerResult.stopped_reason` Literal expands to four values."""
    from typing import get_args

    from codebus_agent.agent.types import ExplorerResult

    literal = ExplorerResult.model_fields["stopped_reason"].annotation
    values = set(get_args(literal))
    assert values == {
        "budget_exhausted",
        "queue_empty",
        "cancelled",
        "budget_tokens_exhausted",
    }
