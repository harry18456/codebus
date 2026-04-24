"""Explorer ReAct main loop.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReAct loop executes think-act-observe-judge-log-update each iteration
  Requirement: Explorer Think step validates ExplorerAction via Instructor
  Requirement: Explorer loop stops on budget exhaustion, empty queue, or cancel signal

Single async entrypoint ``run_explorer`` drives the Think / Act / Observe
/ Judge / Log / Update six-step loop (`docs/agent-core.md §四`). Private
helpers (``_think`` / ``_execute_tools`` / ``_append_observations`` /
``_update_state`` / ``_should_stop``) stay at module level so tests and
future subclasses can swap individual substeps without leaking loop
plumbing into callers.

P0 deliberately sidesteps several pieces of the full spec:

- **Coverage-gap recursion** — the recursion-hook branch at the end of
  ``run_explorer`` is gated by ``_COVERAGE_RECURSION_ENABLED = False``
  (per spec scenario `Coverage recursion hook remains dormant in P0`).
  The follow-up ``coverage-gap-recurse`` change flips the flag AND
  fills the recursion body.
- **Context compression / token-aware budget** — ``_update_state``
  decrements ``state.budget_steps_left`` only; token-aware accounting
  and rolling-window compression are deferred to step 21.
- **SSE emit** — ``ReasoningLogger.write`` writes to disk only; SSE
  lands in step 22 (``agent-sse-wiring``).
- **HTTP endpoint** — the sidecar does not expose ``POST /explore``
  here; Explorer is reachable only from Python for now.
"""
from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from typing import Any

from codebus_agent.providers.protocol import Message as ProviderMessage
from codebus_agent.providers.tracked import TrackedProvider

from .prompts.explorer import EXPLORER_SYSTEM, render_explorer_prompt
from .protocols import CoverageChecker, ExplorerTools, Judge
from .reasoning_logger import ReasoningLogger
from .types import (
    ExplorerAction,
    ExplorerResult,
    ExplorerState,
    JudgeVerdict,
    Message,
    Station,
    Step,
    ToolCall,
    ToolResult,
)


__all__ = ["run_explorer"]


_MIN_STATIONS_FOR_CONVERGENCE: int = 3
_COVERAGE_RECURSION_ENABLED: bool = False  # flipped by `coverage-gap-recurse`


def _should_stop(
    state: ExplorerState, cancel_event: asyncio.Event | None
) -> tuple[bool, str | None]:
    """Return (stop?, stopped_reason)."""
    if cancel_event is not None and cancel_event.is_set():
        return True, "cancelled"
    if state.budget_steps_left <= 0:
        return True, "budget_exhausted"
    if (
        not state.pending_queue
        and len(state.stations) >= _MIN_STATIONS_FOR_CONVERGENCE
    ):
        return True, "queue_empty"
    return False, None


def _to_provider_messages(messages: list[Message]) -> list[ProviderMessage]:
    """Agent-layer Message (Pydantic) → provider-layer Message (dataclass)."""
    return [
        ProviderMessage(role=m.role, content=m.content, tool_call_id=m.tool_call_id)
        for m in messages
    ]


async def _think(
    state: ExplorerState,
    provider: TrackedProvider,
    tool_specs: list[dict],
) -> tuple[str, list[ToolCall]]:
    """Render prompt → single chat call → return (thought, tool_calls)."""
    user_prompt = render_explorer_prompt(state, tool_specs)
    messages = _to_provider_messages(state.messages) + [
        ProviderMessage(role="system", content=EXPLORER_SYSTEM),
        ProviderMessage(role="user", content=user_prompt),
    ]
    action = await provider.chat(messages, response_model=ExplorerAction)
    assert isinstance(action, ExplorerAction), (
        "TrackedProvider.chat(response_model=ExplorerAction) must return "
        "a validated ExplorerAction instance"
    )
    return action.thought, action.tool_calls


async def _execute_one(call: ToolCall, tools: ExplorerTools) -> ToolResult:
    """Route a tool call to the matching method on the tools impl.

    Missing methods and raised exceptions both collapse into
    ``ToolResult.error`` so the loop can record the failure and move on
    (spec scenario `Tool errors do not crash the loop`).
    """
    method = getattr(tools, call.name, None)
    if method is None or not callable(method):
        msg = f"unknown tool {call.name!r}"
        return ToolResult(
            tool_call_id=call.id,
            tool_name=call.name,
            output=f"ERROR: {msg}",
            raw=None,
            error=msg,
        )
    try:
        output = await method(**call.arguments)
    except BaseException as exc:  # noqa: BLE001 — capture then record
        return ToolResult(
            tool_call_id=call.id,
            tool_name=call.name,
            output=f"ERROR: {exc}",
            raw=None,
            error=str(exc),
        )
    return ToolResult(
        tool_call_id=call.id,
        tool_name=call.name,
        output=str(output) if output is not None else "",
        raw=output,
        error=None,
    )


async def _execute_tools(
    calls: list[ToolCall], tools: ExplorerTools
) -> list[ToolResult]:
    if not calls:
        return []
    return list(
        await asyncio.gather(*[_execute_one(c, tools) for c in calls])
    )


def _append_observations(
    state: ExplorerState,
    calls: list[ToolCall],
    results: list[ToolResult],
) -> None:
    """Fold tool results into state.messages as role=`tool` messages."""
    results_by_id = {r.tool_call_id: r for r in results}
    for call in calls:
        result = results_by_id.get(call.id)
        if result is None:
            continue
        state.messages.append(
            Message(
                role="tool",
                content=result.output,
                tool_call_id=call.id,
                tool_name=call.name,
            )
        )


def _update_state(
    state: ExplorerState,
    calls: list[ToolCall],
    results: list[ToolResult],
    verdict: JudgeVerdict | None,
) -> None:
    """P0 Update step — verdict-driven station / queue / visited_files fold."""
    # Extend visited_files with any read_file results (simple fold; P0
    # heuristic — richer logic lands with the real tool impls).
    for r in results:
        if r.tool_name == "read_file" and not r.error:
            # The ``read_file`` tool hasn't been implemented yet; the
            # follow-up change will supply a ``path`` argument convention
            # we can lift into `visited_files` here.
            arg_path = None
            for call in calls:
                if call.id == r.tool_call_id:
                    arg_path = call.arguments.get("path")
                    break
            if isinstance(arg_path, str):
                state.visited_files.add(arg_path)

    if verdict is not None:
        if verdict.should_add_station:
            # Tagging the station with the most recent tool target is the
            # natural P0 signal; richer scoring is Judge-prompt work.
            seed_path = ""
            for call in calls:
                p = call.arguments.get("path") or call.arguments.get("target")
                if isinstance(p, str):
                    seed_path = p
                    break
            state.stations.append(
                Station(
                    path=seed_path,
                    role="explorer-p0",
                    relevance=verdict.relevance,
                    why=verdict.reason,
                    depends_on=[],
                )
            )
        if verdict.should_follow_imports:
            for r in results:
                if r.raw is not None and not r.error:
                    state.pending_queue.append(r.tool_name)


async def run_explorer(
    *,
    state: ExplorerState,
    provider: TrackedProvider,
    tools: ExplorerTools,
    judge: Judge,
    coverage: CoverageChecker,
    logger: ReasoningLogger,
    cancel_event: asyncio.Event | None = None,
    tool_specs: list[dict] | None = None,
) -> ExplorerResult:
    """Drive the Think → Act → Observe → Judge → Log → Update loop.

    ``tool_specs`` precedence (per spec `ExplorerTools, Judge, and
    CoverageChecker are structural Protocols`): caller-supplied kwarg
    wins, else call ``tools.tool_specs()`` if present, else fall back to
    an empty list. The optional Protocol method lets ``FolderTools`` (and
    future ``TopicTools``) advertise their surface without caller plumbing.
    """
    if tool_specs is None:
        tool_specs_fn = getattr(tools, "tool_specs", None)
        if callable(tool_specs_fn):
            tool_specs = tool_specs_fn()
        else:
            tool_specs = []

    while True:
        should_stop, stopped_reason = _should_stop(state, cancel_event)
        if should_stop:
            break

        # 1. Think
        thought, tool_calls = await _think(state, provider, tool_specs)

        # 2. Act (parallel tool dispatch; errors captured into ToolResult.error)
        results = await _execute_tools(tool_calls, tools)

        # 3. Observe — feed tool outputs forward into next Think
        _append_observations(state, tool_calls, results)

        # 4. Judge (one-shot per iteration; MUST NOT mutate state)
        verdict = await judge.evaluate(state, results)

        # 5. Log — append one JSONL line to reasoning_log.jsonl
        logger.write(
            Step(
                step=state.step_count,
                ts=datetime.now(timezone.utc),
                thought=thought,
                tool_calls=list(tool_calls),
                tool_results=list(results),
                judge_verdict=verdict,
                tokens_used=0,
            )
        )

        # 6. Update state — fold verdict into stations / queue / visited
        _update_state(state, tool_calls, results, verdict)
        state.step_count += 1
        state.budget_steps_left -= 1

    # Coverage-gap recursion hook — gated off in P0 per spec scenario
    # `Coverage recursion hook remains dormant in P0`. The follow-up
    # ``coverage-gap-recurse`` change flips the flag + fills the body.
    if _COVERAGE_RECURSION_ENABLED:  # pragma: no cover - intentionally dormant
        _ = coverage  # silences unused-parameter lint in the dormant branch
        raise RuntimeError(
            "coverage recursion is disabled in P0 — lands in coverage-gap-recurse"
        )

    return ExplorerResult(
        stations=list(state.stations),
        log_path=str(logger.path),
        stopped_reason=stopped_reason or "budget_exhausted",
    )
