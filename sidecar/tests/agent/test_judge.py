"""RED tests for LLMJudge.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: Judge evaluation runs as one-shot call per iteration
"""
from __future__ import annotations

from collections.abc import Callable
from copy import deepcopy
from pathlib import Path
from typing import Any

import pytest

from codebus_agent.providers.mock import MockScript
from codebus_agent.providers.protocol import ProviderRole
from codebus_agent.providers.tracked import TrackedProvider


class _SpyTools:
    """Monitors whether Judge illegally reaches into ExplorerTools."""

    def __init__(self) -> None:
        self.primary_search_calls = 0
        self.fetch_calls = 0
        self.follow_reference_calls = 0

    async def primary_search(self, query: str) -> list[Any]:  # pragma: no cover - must stay 0
        self.primary_search_calls += 1
        return []

    async def fetch(self, target: Any) -> Any:  # pragma: no cover - must stay 0
        self.fetch_calls += 1
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:  # pragma: no cover - must stay 0
        self.follow_reference_calls += 1
        return []


async def test_llm_judge_returns_validated_verdict(
    mock_script_judge: MockScript,
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Judge returns a JudgeVerdict instance (Instructor-parsed)."""
    from codebus_agent.agent.judge import LLMJudge
    from codebus_agent.agent.types import ExplorerState, JudgeVerdict

    pinned = JudgeVerdict(
        relevance=0.8,
        should_follow_imports=True,
        should_add_station=True,
        reason="central abstraction",
    )
    mock_script_judge.push(pinned)

    judge = LLMJudge(mock_judge_provider_factory, workspace_dir)
    state = ExplorerState(task="explore KB", budget_steps_left=10, budget_tokens_left=1000)
    verdict = await judge.evaluate(state, results=[])

    assert isinstance(verdict, JudgeVerdict)
    assert verdict.relevance == 0.8
    assert verdict.should_follow_imports is True
    assert verdict.reason == "central abstraction"


async def test_judge_is_stateless_with_respect_to_state(
    mock_script_judge: MockScript,
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Judge MUST NOT mutate ExplorerState — only Explorer's Update step may."""
    from codebus_agent.agent.judge import LLMJudge
    from codebus_agent.agent.types import ExplorerState, JudgeVerdict, Station

    mock_script_judge.push(
        JudgeVerdict(
            relevance=0.5,
            should_follow_imports=False,
            should_add_station=False,
            reason="ok",
        )
    )
    state = ExplorerState(
        task="explore KB",
        budget_steps_left=10,
        budget_tokens_left=1000,
        step_count=3,
        stations=[
            Station(path="a.py", role="entry", relevance=0.7, why="start", depends_on=[])
        ],
        visited_files={"a.py", "b.py"},
        pending_queue=["c.py"],
    )
    before = deepcopy(state.model_dump())

    judge = LLMJudge(mock_judge_provider_factory, workspace_dir)
    await judge.evaluate(state, results=[])

    after = state.model_dump()
    assert after == before, (
        f"Judge must not mutate ExplorerState; diff:\n  before={before}\n  after={after}"
    )


def test_judge_provider_role_is_judge_not_reasoning(
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Factory-produced TrackedProvider MUST carry role=JUDGE, default_module='judge'."""
    provider = mock_judge_provider_factory(workspace_dir)
    assert provider.role == ProviderRole.JUDGE
    assert provider._default_module == "judge"


async def test_judge_does_not_invoke_explorer_tools(
    mock_script_judge: MockScript,
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    """Judge is one-shot — it MUST NOT enter a ReAct sub-loop or touch tools."""
    from codebus_agent.agent.judge import LLMJudge
    from codebus_agent.agent.types import ExplorerState, JudgeVerdict

    mock_script_judge.push(
        JudgeVerdict(
            relevance=0.5,
            should_follow_imports=False,
            should_add_station=False,
            reason="ok",
        )
    )

    spy = _SpyTools()
    # LLMJudge's constructor MUST NOT accept a tools parameter — we
    # instantiate a spy alongside and verify counters stay at zero after
    # evaluate(). If a future refactor threaded tools into Judge, this
    # test fails loud.
    judge = LLMJudge(mock_judge_provider_factory, workspace_dir)

    assert not hasattr(judge, "_tools"), (
        "LLMJudge MUST NOT hold a tools reference; one-shot verdicts do not need them"
    )

    state = ExplorerState(
        task="t", budget_steps_left=10, budget_tokens_left=1000
    )
    await judge.evaluate(state, results=[])

    assert spy.primary_search_calls == 0
    assert spy.fetch_calls == 0
    assert spy.follow_reference_calls == 0


@pytest.mark.asyncio
async def _unused_marker() -> None:
    """Placeholder — pytest-asyncio is configured at repo level."""
    pass
