"""Tests for Judge / CoverageChecker `provider` property surface.

Backs SHALL clauses in
openspec/changes/context-compression-token-budget/design.md
  Decision 7: Judge / Coverage 暴露 `provider` property

Explorer's HTTP endpoint needs to aggregate `session_total_tokens`
across reasoning + judge + coverage TrackedProviders. Judge and
Coverage hold their own workspace-scoped TrackedProvider instance;
exposing a read-only property is the cleanest surface for the
aggregator without leaking underscore-prefixed internals.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from codebus_agent.providers.tracked import TrackedProvider


def test_llm_judge_exposes_provider_property(
    mock_judge_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    from codebus_agent.agent.judge import LLMJudge

    judge = LLMJudge(mock_judge_provider_factory, workspace_dir)
    assert isinstance(judge.provider, TrackedProvider)
    assert judge.provider is judge._provider


def test_llm_coverage_checker_exposes_provider_property(
    mock_coverage_provider_factory: Callable[[Path], TrackedProvider],
    workspace_dir: Path,
) -> None:
    from codebus_agent.agent.coverage import LLMCoverageChecker

    checker = LLMCoverageChecker(mock_coverage_provider_factory, workspace_dir)
    assert isinstance(checker.provider, TrackedProvider)
    assert checker.provider is checker._provider
