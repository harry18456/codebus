"""LLMJudge — one-shot Relevance verdict producer.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: Judge evaluation runs as one-shot call per iteration

Thin by design: ``evaluate(state, results)`` renders one prompt, makes
one ``provider.chat(messages, response_model=JudgeVerdict)`` call, and
returns the parsed verdict. It MUST NOT enter a ReAct sub-loop, MUST
NOT invoke ``ExplorerTools``, and MUST NOT mutate ``state`` (see
spec scenario `Judge is stateless with respect to ExplorerState`).

The Judge holds a pre-materialized ``TrackedProvider`` constructed via
the caller-supplied factory (shape matches
``app.state.llm_judge_provider(workspace_root)``). That keeps audit
wiring uniform with the rest of the sidecar: every Judge call lands in
the same workspace-scoped ``token_usage.jsonl`` / ``llm_calls.jsonl``.
"""
from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

from codebus_agent.providers.protocol import Message as ProviderMessage
from codebus_agent.providers.tracked import TrackedProvider

from .prompts.judge import JUDGE_SYSTEM, render_judge_prompt
from .types import JudgeVerdict, ToolResult

if TYPE_CHECKING:
    from .types import ExplorerState


__all__ = ["LLMJudge"]


class LLMJudge:
    """Structural satisfier of ``agent.protocols.Judge``.

    Intentionally does NOT import ``ExplorerTools`` — one-shot verdicts
    never need tool dispatch (spec scenario
    `Judge does not invoke ExplorerTools`).
    """

    def __init__(
        self,
        provider_factory: Callable[[Path], TrackedProvider],
        workspace_root: Path,
    ) -> None:
        self._provider = provider_factory(Path(workspace_root))

    async def evaluate(
        self,
        state: "ExplorerState",
        results: list[ToolResult],
    ) -> JudgeVerdict:
        messages = [
            ProviderMessage(role="system", content=JUDGE_SYSTEM),
            ProviderMessage(
                role="user",
                content=render_judge_prompt(state.task, results),
            ),
        ]
        verdict = await self._provider.chat(
            messages, response_model=JudgeVerdict
        )
        assert isinstance(verdict, JudgeVerdict), (
            "TrackedProvider.chat(response_model=JudgeVerdict) must return "
            "a validated JudgeVerdict instance"
        )
        return verdict
