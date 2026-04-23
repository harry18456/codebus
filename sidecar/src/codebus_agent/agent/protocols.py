"""Explorer core structural Protocols.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ExplorerTools, Judge, and CoverageChecker are structural Protocols

These Protocols are the day-1 abstraction seam between the ReAct core
(`run_explorer`) and the pluggable outside world:

- ``ExplorerTools`` — concrete Folder-mode tools land in the follow-up
  ``explorer-tools-p0`` change; Topic-mode tools (Phase 2) and Q&A Agent
  tools (Module 8) satisfy the same Protocol without touching the loop.
- ``Judge`` / ``CoverageChecker`` — single-call evaluators; same split.

``@runtime_checkable`` is set so tests can duck-check impls, but
``run_explorer`` MUST NOT use ``isinstance`` in its hot path — static
type checking is the production safety net.

Helper Pydantic types (``SearchHit`` / ``Content`` / ``Target`` /
``Gap``) live here alongside the Protocols so a TopicTools impl can
satisfy the same contract without pulling Folder-mode specifics.
"""
from __future__ import annotations

from typing import Any, Protocol, runtime_checkable

from pydantic import BaseModel, Field

from .types import Gap, JudgeVerdict, ToolResult


__all__ = [
    "Content",
    "CoverageChecker",
    "ExplorerTools",
    "Gap",
    "Judge",
    "SearchHit",
    "Target",
]


class SearchHit(BaseModel):
    """Primary-search result — abstract shape, no Folder-mode leakage."""

    path: str
    snippet: str
    score: float = Field(ge=0, le=1)


class Content(BaseModel):
    """Fetched content — abstract shape (Folder-mode feeds file text; Topic feeds doc text)."""

    path: str
    text: str
    lines_range: tuple[int, int] | None = None


class Target(BaseModel):
    """Reference-follow target — Explorer may enqueue for future iterations."""

    kind: str
    args: dict[str, Any] = Field(default_factory=dict)
    priority: int = 0


@runtime_checkable
class ExplorerTools(Protocol):
    """Three-method tool surface; Folder / Topic / Q&A each provide their own impl."""

    async def primary_search(self, query: str) -> list[SearchHit]:
        ...

    async def fetch(self, target: Target) -> Content:
        ...

    async def follow_reference(self, symbol: str) -> list[Target]:
        ...


@runtime_checkable
class Judge(Protocol):
    """One-shot relevance evaluator — see `Judge evaluation runs as one-shot call per iteration`."""

    async def evaluate(
        self, state: Any, results: list[ToolResult]
    ) -> JudgeVerdict:
        ...


@runtime_checkable
class CoverageChecker(Protocol):
    """Gap detector — P0 contract only; recursion lands in `coverage-gap-recurse`."""

    async def check(self, state: Any) -> list[Gap]:
        ...
