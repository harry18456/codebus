"""RED tests for agent-core Protocols.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols

Day-1 abstraction is load-bearing: Q&A Agent (Module 8) and Topic-mode
Explorer (Phase 2) both plug into the same loop by satisfying these
Protocols structurally. The tests below pin that:

* `runtime_checkable` is set (so `isinstance` duck-checks work in tests)
* `primary_search` carries abstract types (`str` → `list[SearchHit]`),
  not Folder-mode-specific types like `Path`.
"""
from __future__ import annotations

import inspect
import typing
from typing import Any


class _MockTools:
    """Bare-bones structural impl — no inheritance from `ExplorerTools`."""

    async def primary_search(self, query: str) -> list[Any]:  # noqa: D401
        return []

    async def fetch(self, target: Any) -> Any:
        return None

    async def follow_reference(self, symbol: str) -> list[Any]:
        return []


class _MockJudge:
    async def evaluate(self, state: Any, results: Any) -> Any:
        return None


class _MockCoverage:
    async def check(self, state: Any) -> list[Any]:
        return []


def test_mock_tools_satisfies_explorer_tools_protocol() -> None:
    from codebus_agent.agent.protocols import ExplorerTools

    mock = _MockTools()
    assert isinstance(mock, ExplorerTools), (
        "A structural impl with primary_search / fetch / follow_reference "
        "coroutines MUST satisfy ExplorerTools via @runtime_checkable"
    )


def test_mock_judge_satisfies_judge_protocol() -> None:
    from codebus_agent.agent.protocols import Judge

    mock = _MockJudge()
    assert isinstance(mock, Judge)


def test_mock_coverage_satisfies_coverage_checker_protocol() -> None:
    from codebus_agent.agent.protocols import CoverageChecker

    mock = _MockCoverage()
    assert isinstance(mock, CoverageChecker)


def test_protocols_do_not_bind_folder_mode_types() -> None:
    """Protocol signatures MUST use abstract types (str / SearchHit / Content / Target).

    Spec scenario `Protocols do not bind Folder-mode types` — signatures
    must stay abstract so a Phase-2 `TopicTools` impl can satisfy the
    same contract without core-loop changes.
    """
    from codebus_agent.agent.protocols import (
        Content,
        ExplorerTools,
        SearchHit,
        Target,
    )

    sig = inspect.signature(ExplorerTools.primary_search)
    non_self = [p for p in sig.parameters.values() if p.name != "self"]
    assert len(non_self) == 1
    assert non_self[0].name == "query"

    # `from __future__ import annotations` keeps annotations as strings
    # at signature time; resolve them via `get_type_hints` so the assertion
    # compares the actual type.
    hints = typing.get_type_hints(ExplorerTools.primary_search)
    assert hints["query"] is str, (
        f"primary_search.query MUST resolve to `str`, got {hints['query']!r}"
    )
    return_hint = hints["return"]
    return_str = str(return_hint)
    assert "SearchHit" in return_str, (
        f"primary_search return MUST be abstract (SearchHit), got {return_hint!r}"
    )
    assert "Path" not in return_str and "pathlib" not in return_str, (
        f"primary_search return MUST NOT leak Path / pathlib types, got {return_hint!r}"
    )

    # Fetch / follow_reference similarly abstract.
    fetch_hints = typing.get_type_hints(ExplorerTools.fetch)
    fetch_return = str(fetch_hints["return"])
    assert "Content" in fetch_return and "Path" not in fetch_return

    follow_hints = typing.get_type_hints(ExplorerTools.follow_reference)
    follow_return = str(follow_hints["return"])
    assert "Target" in follow_return and "Path" not in follow_return

    # Sanity: SearchHit / Content / Target all resolve from the agent module.
    assert SearchHit is not None
    assert Content is not None
    assert Target is not None
