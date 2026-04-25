"""Agent core — Explorer ReAct loop skeleton.

Implements `openspec/changes/explorer-react-loop-p0`, landing the minimum
self-written ReAct + Instructor/Pydantic stack (D-012) needed for Module
4 Explorer Agent work. The surface is intentionally thin:

- ``types`` — Pydantic data structures for the loop
- ``protocols`` — ``typing.Protocol`` seams for pluggable impls
  (``ExplorerTools`` / ``Judge`` / ``CoverageChecker``)
- ``explorer`` — ``run_explorer`` async main loop
- ``judge`` — ``LLMJudge`` one-shot verdict producer
- ``reasoning_logger`` — append-only JSONL writer (D-022 audit layer #4)
- ``prompts`` — system prompt constants + render helpers per role

Roll-up re-exports (populated at the end of Section 11) keep the public
surface narrow: anything Explorer-specific lives inside a submodule so
Q&A Agent (Module 8) can import the same ReAct core without pulling
Folder-mode baggage (`docs/agent-explorer-spec.md` §十二).
"""
from __future__ import annotations

from .budget import AggregatedTokenProbe, TokenBudgetProbe
from .coverage import LLMCoverageChecker

__all__: list[str] = [
    "AggregatedTokenProbe",
    "LLMCoverageChecker",
    "TokenBudgetProbe",
]
