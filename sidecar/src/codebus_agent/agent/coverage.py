"""LLMCoverageChecker — one-shot gap detector run after main-loop convergence.

Backs SHALL clauses in
openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: LLMCoverageChecker produces one-shot CoverageResult

Thin by design — mirrors `LLMJudge`'s shape:
- Constructor takes `provider_factory(workspace_root) -> TrackedProvider`
  and materialises one workspace-scoped `TrackedProvider` up front so
  every audit record lands in the same `token_usage.jsonl` /
  `llm_calls.jsonl` / `sanitize_audit.jsonl` trio for the workspace
  (D-021 / D-022 / D-015).
- `check(state)` renders one prompt, makes one
  `provider.chat(messages, response_model=CoverageResult)` call, and
  returns `result.gaps` unchanged. It MUST NOT enter a ReAct sub-loop,
  MUST NOT invoke `ExplorerTools`, and MUST NOT mutate `state` (same
  discipline as Judge).
- `set_emitter(emitter)` forwards to the wrapped TrackedProvider so
  Coverage-side `usage_delta` / `llm_call` events land on the same SSE
  channel as the Explorer loop.

The `run_explorer` recursion body (`_COVERAGE_RECURSION_ENABLED=True`)
decides whether to recurse based on the returned gaps + budget + depth
— see `agent/explorer.py` for the coverage-round decision logic.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

from codebus_agent.providers.protocol import Message as ProviderMessage
from codebus_agent.providers.tracked import TrackedProvider

from .prompts.coverage import COVERAGE_SYSTEM, render_coverage_prompt
from .types import CoverageResult, Gap

if TYPE_CHECKING:
    from .emitter import SSEEmitter
    from .types import ExplorerState


__all__ = ["LLMCoverageChecker"]


class LLMCoverageChecker:
    """Structural satisfier of `agent.protocols.CoverageChecker`.

    Intentionally does NOT import `ExplorerTools` — one-shot gap
    detection never needs tool dispatch (see spec Requirement
    `LLMCoverageChecker produces one-shot CoverageResult`: "MUST NOT
    invoke any ExplorerTools method").
    """

    def __init__(
        self,
        provider_factory: Callable[[Path], TrackedProvider],
        workspace_root: Path,
    ) -> None:
        self._provider = provider_factory(Path(workspace_root))

    def set_emitter(self, emitter: "SSEEmitter | None") -> None:
        """Propagate `emitter` down to the wrapped TrackedProvider.

        The Explorer endpoint calls this once the per-task TaskHandle is
        created so coverage-side `usage_delta` / `llm_call` events land
        on the same SSE channel as the Explorer loop's own emits.
        """
        self._provider.set_emitter(emitter)

    async def check(self, state: "ExplorerState") -> list[Gap]:
        messages = [
            ProviderMessage(role="system", content=COVERAGE_SYSTEM),
            ProviderMessage(
                role="user",
                content=render_coverage_prompt(state),
            ),
        ]
        result = await self._provider.chat(
            messages, response_model=CoverageResult
        )
        assert isinstance(result, CoverageResult), (
            "TrackedProvider.chat(response_model=CoverageResult) must "
            "return a validated CoverageResult instance"
        )
        return list(result.gaps)
