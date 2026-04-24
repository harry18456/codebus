"""RED tests for LLMCoverageChecker.

Backs SHALL clauses in
openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: LLMCoverageChecker produces one-shot CoverageResult

Section 2 pins the three behavioural scenarios of the one-shot
contract (prompt-module version pin lives alongside determinism tests
in prompts/test_coverage_prompt.py):

- `check issues one-shot structured call`: `chat` fires once with
  `response_model=CoverageResult`, `result.gaps` returned unchanged.
- `check does not mutate ExplorerState`: all six tracked fields stay
  identical pre vs. post invocation.
- `set_emitter propagates to TrackedProvider`: spy emitter sees
  `usage_delta` / `llm_call` events from the next chat call.
"""
from __future__ import annotations

from collections.abc import Callable
from copy import deepcopy
from pathlib import Path
from typing import Any

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.tracked import TrackedProvider


class _SpyEmitter:
    """Structural `SSEEmitter` conformer that captures every emit in order."""

    def __init__(self) -> None:
        self.events: list[dict] = []

    def emit(self, event: dict) -> None:
        self.events.append(event)


async def test_check_issues_one_shot_structured_call(
    mock_script_coverage: MockScript,
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Spec scenario `check issues one-shot structured call`."""
    from codebus_agent.agent.coverage import LLMCoverageChecker
    from codebus_agent.agent.types import (
        CoverageResult,
        ExplorerState,
        Gap,
    )

    pinned = CoverageResult(
        gaps=[
            Gap(description="missing import chain in adapter", suggested_target="src/adapter.py"),
            Gap(description="entrypoint not yet followed", suggested_target=None),
        ]
    )
    mock_script_coverage.push(pinned)

    # Wrap the inner provider so we can assert `chat` was called exactly
    # once with `response_model=CoverageResult`. This matches the spy
    # pattern used in `test_explorer_loop.py` / `test_judge.py`.
    checker = LLMCoverageChecker(mock_coverage_provider_factory, workspace_dir)
    captured: list[dict] = []
    inner = checker._provider._inner  # type: ignore[attr-defined]
    original = inner.chat

    async def wrapped(messages: Any, *, response_model: Any, **kwargs: Any) -> Any:
        captured.append(
            {
                "messages": list(messages),
                "response_model": response_model,
                "extra_kwargs": kwargs,
            }
        )
        return await original(messages, response_model=response_model, **kwargs)

    inner.chat = wrapped  # type: ignore[method-assign]

    state = ExplorerState(
        task="explore KB", budget_steps_left=5, budget_tokens_left=1000
    )
    gaps = await checker.check(state)

    assert len(captured) == 1, (
        f"LLMCoverageChecker.check MUST issue exactly one chat call; saw "
        f"{len(captured)}"
    )
    assert captured[0]["response_model"] is CoverageResult, (
        f"check MUST pass response_model=CoverageResult; saw "
        f"{captured[0]['response_model']!r}"
    )
    # Returned gaps MUST equal the pinned CoverageResult.gaps unchanged.
    assert gaps == pinned.gaps
    assert all(isinstance(g, Gap) for g in gaps)


async def test_check_does_not_mutate_explorer_state(
    mock_script_coverage: MockScript,
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Spec scenario `check does not mutate ExplorerState`."""
    from codebus_agent.agent.coverage import LLMCoverageChecker
    from codebus_agent.agent.types import (
        CoverageResult,
        ExplorerState,
        Gap,
        Message,
        Station,
    )

    mock_script_coverage.push(
        CoverageResult(gaps=[Gap(description="seed gap", suggested_target="src/x.py")])
    )
    state = ExplorerState(
        task="explore KB",
        budget_steps_left=10,
        budget_tokens_left=1000,
        step_count=4,
        stations=[
            Station(
                path="a.py",
                role="entry",
                relevance=0.7,
                why="start",
                depends_on=[],
            )
        ],
        visited_files={"a.py", "b.py"},
        pending_queue=["c.py"],
        messages=[Message(role="user", content="task primer")],
    )
    before = deepcopy(state.model_dump())

    checker = LLMCoverageChecker(mock_coverage_provider_factory, workspace_dir)
    await checker.check(state)

    after = state.model_dump()
    assert after == before, (
        f"LLMCoverageChecker.check must not mutate ExplorerState; "
        f"diff:\n  before={before}\n  after={after}"
    )


async def test_set_emitter_propagates_to_tracked_provider(
    mock_script_coverage: MockScript,
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Spec scenario `set_emitter propagates to TrackedProvider`.

    Asserts two event types land on the spy after `set_emitter`:
    - `usage_delta` (TrackedProvider._emit_usage_delta)
    - `llm_call`    (LLMCallLogger.emit via inner logger)
    """
    from codebus_agent.agent.coverage import LLMCoverageChecker
    from codebus_agent.agent.types import (
        CoverageResult,
        ExplorerState,
        Gap,
    )

    mock_script_coverage.push(
        CoverageResult(gaps=[Gap(description="a gap")])
    )
    spy = _SpyEmitter()
    checker = LLMCoverageChecker(mock_coverage_provider_factory, workspace_dir)
    checker.set_emitter(spy)

    state = ExplorerState(
        task="t", budget_steps_left=1, budget_tokens_left=1000
    )
    await checker.check(state)

    types = [e.get("type") for e in spy.events]
    assert "usage_delta" in types, (
        f"set_emitter MUST propagate so TrackedProvider emits usage_delta; "
        f"saw event types {types}"
    )
    assert "llm_call" in types, (
        f"set_emitter MUST propagate so LLMCallLogger emits llm_call; "
        f"saw event types {types}"
    )
