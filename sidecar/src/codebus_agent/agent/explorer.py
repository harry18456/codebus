"""Explorer ReAct main loop.

Backs SHALL clauses in
openspec/changes/explorer-react-loop-p0/specs/agent-core/spec.md
  Requirement: ReAct loop executes think-act-observe-judge-log-update each iteration
  Requirement: Explorer Think step validates ExplorerAction via Instructor
  Requirement: Explorer loop stops on budget exhaustion, empty queue, or cancel signal

openspec/changes/coverage-gap-recurse/specs/agent-core/spec.md
  Requirement: Coverage-gap recursion runs after main loop convergence
openspec/changes/coverage-gap-recurse/specs/explorer-sse/spec.md
  Requirement: Coverage round emits coverage_gaps SSE event

Single async entrypoint ``run_explorer`` drives the Think / Act / Observe
/ Judge / Log / Update six-step loop (`docs/agent-core.md §四`). Private
helpers (``_think`` / ``_execute_tools`` / ``_append_observations`` /
``_update_state`` / ``_should_stop``) stay at module level so tests and
future subclasses can swap individual substeps without leaking loop
plumbing into callers.

After the main while loop converges, ``run_explorer`` fires exactly one
coverage-gap round (``coverage.check``) and — when all three
preconditions hold (gaps non-empty, budget > 0, depth within cap) —
tail-recurses into a new ``run_explorer`` call with ``_depth + 1``.
The innermost convergence's ``stopped_reason`` propagates unchanged
through the recursive return chain so callers always see the deepest
frame's terminal branch.

Deferred to later changes (not this one):

- **Context compression / token-aware budget** — ``_update_state``
  decrements ``state.budget_steps_left`` only; token-aware accounting
  and rolling-window compression land in step 21.
"""
from __future__ import annotations

import asyncio
from datetime import datetime, timezone
from typing import Any

from codebus_agent.providers.protocol import Message as ProviderMessage
from codebus_agent.providers.tracked import TrackedProvider

from .emitter import NullEmitter, SSEEmitter
from .prompts.explorer import (
    EXPLORER_PROMPT_VERSION,
    EXPLORER_SYSTEM,
    render_explorer_prompt,
)
from .prompts.judge import JUDGE_PROMPT_VERSION
from .protocols import CoverageChecker, ExplorerTools, Judge
from .reasoning_logger import ReasoningLogger
from .types import (
    ExplorerAction,
    ExplorerResult,
    ExplorerState,
    Gap,
    JudgeVerdict,
    Message,
    Station,
    Step,
    ToolCall,
    ToolResult,
)


__all__ = ["run_explorer"]


_MIN_STATIONS_FOR_CONVERGENCE: int = 3
# `coverage-gap-recurse` flipped this to True and filled the recursion body.
# Kept as a module-level kill-switch for rollback / debugging — flipping it
# back to False makes `run_explorer` skip the coverage round entirely without
# affecting the Judge / Explorer main loop.
_COVERAGE_RECURSION_ENABLED: bool = True
# Max recursion depth cap — proposal pins "3 層上限 = 主 loop + 最多 2 次
# gap 補查". The recursion precondition is `_depth + 1 < _COVERAGE_MAX_DEPTH`
# so legal depths are 0 / 1 / 2; at depth=2 the next frame would equal the
# cap and recursion halts (see spec scenario `Max depth halts further
# recursion`).
_COVERAGE_MAX_DEPTH: int = 3
_OBSERVATION_TRUNCATE_LIMIT: int = 500  # chars for agent_action_result.observation
_TRUNCATE_MARKER: str = "… [truncated]"
# `_enqueue_gap_investigation` renders at most this many gap descriptions
# into the user-message summary (Decision 6); remaining gaps collapse into
# `（及其他 N 項）` so the prompt stays bounded even on pathological rounds.
_COVERAGE_SUMMARY_MAX_GAPS: int = 3
_COVERAGE_SUMMARY_DESC_TRUNCATE: int = 60
_COVERAGE_QUEUE_PLACEHOLDER_TRUNCATE: int = 80


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


def _coverage_skip_reason(
    gaps: list[Gap], *, budget_ok: bool, depth_ok: bool
) -> str | None:
    """Compute the `skip_reason` value for a coverage round.

    Returns `None` when recursion SHOULD fire (all three preconditions
    hold); otherwise returns one of `"no_gaps"` / `"max_depth_reached"`
    / `"budget_exhausted"` per the spec's precedence order
    (`no_gaps > max_depth_reached > budget_exhausted` — see explorer-sse
    `Coverage round emits coverage_gaps SSE event`).
    """
    if not gaps:
        return "no_gaps"
    if not depth_ok:
        return "max_depth_reached"
    if not budget_ok:
        return "budget_exhausted"
    return None


def _enqueue_gap_investigation(
    state: ExplorerState, gaps: list[Gap]
) -> None:
    """Double-push: pending_queue += one target per gap, messages += 1 user.

    Design Decision 6 (see openspec/changes/coverage-gap-recurse/design.md):
    - `pending_queue` keeps the Explorer loop alive on re-entry (otherwise
      `_should_stop`'s `queue_empty` branch would fire immediately).
    - `messages` carries the gap summary so the next `_think` prompt
      surfaces the Coverage intent as a user-role context message.
    """
    for gap in gaps:
        target = gap.suggested_target or (
            f"gap:{gap.description[:_COVERAGE_QUEUE_PLACEHOLDER_TRUNCATE]}"
        )
        state.pending_queue.append(target)

    head = gaps[:_COVERAGE_SUMMARY_MAX_GAPS]
    summary = "、".join(
        g.description[:_COVERAGE_SUMMARY_DESC_TRUNCATE] for g in head
    )
    remainder = len(gaps) - _COVERAGE_SUMMARY_MAX_GAPS
    if remainder > 0:
        summary += f"（及其他 {remainder} 項）"

    state.messages.append(
        Message(
            role="user",
            content=f"Coverage 回報 {len(gaps)} 個 gap：{summary}。請優先補查。",
        )
    )


def _truncate_observation(result: ToolResult) -> str:
    """Pick the ≤500-char payload carried on `agent_action_result.observation`.

    Failed tools surface `error` (already captured into `output` as
    `ERROR: <msg>` by `_execute_one`, so reading `output` is enough); the
    truncation marker lets the UI show "more" when a snippet is clipped.
    """
    source = result.output if result.output else (result.error or "")
    if len(source) <= _OBSERVATION_TRUNCATE_LIMIT:
        return source
    return source[:_OBSERVATION_TRUNCATE_LIMIT] + _TRUNCATE_MARKER


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
    emitter: SSEEmitter | None = None,
    _depth: int = 0,
) -> ExplorerResult:
    """Drive the Think → Act → Observe → Judge → Log → Update loop.

    ``tool_specs`` precedence (per spec `ExplorerTools, Judge, and
    CoverageChecker are structural Protocols`): caller-supplied kwarg
    wins, else call ``tools.tool_specs()`` if present, else fall back to
    an empty list. The optional Protocol method lets ``FolderTools`` (and
    future ``TopicTools``) advertise their surface without caller plumbing.

    ``emitter`` (agent-sse-wiring): when set, each iteration fans out
    `agent_thought` → `agent_action_result` → `judge_verdict` → `progress`
    events to the caller's SSE channel. `None` keeps the legacy file-only
    behaviour; the hot path substitutes a `NullEmitter()` so the inner
    loop never branches on nullability.

    ``_depth`` (coverage-gap-recurse): keyword-only recursion counter.
    The leading underscore marks it as an implementation detail — callers
    (HTTP layer, golden-sample replay) MUST leave it at the 0 default.
    Bumped by exactly one when the coverage round recurses into a new
    `run_explorer` frame; capped at `_COVERAGE_MAX_DEPTH - 1` so at most
    two gap rounds follow the outer loop.
    """
    if tool_specs is None:
        tool_specs_fn = getattr(tools, "tool_specs", None)
        if callable(tool_specs_fn):
            tool_specs = tool_specs_fn()
        else:
            tool_specs = []

    _emitter: SSEEmitter = emitter or NullEmitter()
    # Snapshot the budget for `progress.total` so the UI bar denominator
    # stays fixed even as `state.budget_steps_left` decrements each iter.
    initial_budget_steps = state.budget_steps_left

    while True:
        should_stop, stopped_reason = _should_stop(state, cancel_event)
        if should_stop:
            break

        iter_step = state.step_count

        # 1. Think
        thought, tool_calls = await _think(state, provider, tool_specs)
        _emitter.emit(
            {
                "type": "agent_thought",
                "step": iter_step,
                "thought": thought,
                "action": [c.model_dump() for c in tool_calls],
            }
        )

        # 2. Act (parallel tool dispatch; errors captured into ToolResult.error)
        results = await _execute_tools(tool_calls, tools)
        for r in results:
            _emitter.emit(
                {
                    "type": "agent_action_result",
                    "step": iter_step,
                    "tool": r.tool_name,
                    "observation": _truncate_observation(r),
                    # P0: tokens_used is not yet threaded through ToolResult.
                    # Real token accounting lands with step-21 token-budget work.
                    "tokens_used": 0,
                }
            )

        # 3. Observe — feed tool outputs forward into next Think
        _append_observations(state, tool_calls, results)

        # 4. Judge (one-shot per iteration; MUST NOT mutate state)
        verdict = await judge.evaluate(state, results)
        _emitter.emit(
            {
                "type": "judge_verdict",
                "step": iter_step,
                "relevance": verdict.relevance,
                "reason": verdict.reason,
            }
        )

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

        # Progress tick — emitted after the Update step so `current` equals
        # the post-increment step_count (1-indexed against total).
        _emitter.emit(
            {
                "type": "progress",
                "phase": "exploring",
                "current": state.step_count,
                "total": initial_budget_steps,
            }
        )

    # Coverage-gap recursion — `coverage-gap-recurse` flipped
    # `_COVERAGE_RECURSION_ENABLED` True and filled this branch.
    # `coverage.check` fires exactly once per frame, regardless of
    # whether recursion proceeds. The module-level flag stays as a
    # rollback / debugging kill-switch; when False the coverage round
    # and any downstream SSE emit / Step write / recursion are skipped
    # entirely.
    if _COVERAGE_RECURSION_ENABLED:
        gaps = await coverage.check(state)
        budget_ok = state.budget_steps_left > 0
        depth_ok = _depth + 1 < _COVERAGE_MAX_DEPTH
        skip_reason = _coverage_skip_reason(
            gaps, budget_ok=budget_ok, depth_ok=depth_ok
        )
        will_recurse = skip_reason is None

        # Emit the coverage_gaps SSE event unconditionally (empty / skipped
        # rounds still carry narrative value for the frontend). Must fire
        # AFTER coverage.check returns and BEFORE the Step write + any
        # recursive `run_explorer` call so consumers observe the event
        # in temporal order relative to the recursive frame's own emits.
        _emitter.emit(
            {
                "type": "coverage_gaps",
                "round": _depth,
                "gaps": [g.model_dump() for g in gaps],
                "will_recurse": will_recurse,
                "skip_reason": skip_reason,
            }
        )

        # Decision 8: empty gaps round is a no-op for the reasoning log
        # (still emitted on SSE). Non-empty rounds — whether they recurse
        # or not — write exactly one Step line so replay can tell "Agent
        # saw gaps but was blocked by budget / depth" from "Agent saw no
        # gaps at all".
        if gaps:
            logger.write(
                Step(
                    step=state.step_count,
                    ts=datetime.now(timezone.utc),
                    thought=(
                        f"[coverage] round-{_depth + 1} gaps={len(gaps)} "
                        f"will_recurse={will_recurse}"
                    ),
                    tool_calls=[],
                    tool_results=[],
                    judge_verdict=None,
                    tokens_used=0,
                    explorer_prompt_version=EXPLORER_PROMPT_VERSION,
                    judge_prompt_version=JUDGE_PROMPT_VERSION,
                )
            )

        if will_recurse:
            _enqueue_gap_investigation(state, gaps)
            # Tail recursion reuses every collaborator — same `state`,
            # same `logger`, same `provider` / `judge` / `coverage`
            # instances, same emitter. Budget + stations + visited
            # accumulate across frames by design (Decision 2).
            return await run_explorer(
                state=state,
                provider=provider,
                tools=tools,
                judge=judge,
                coverage=coverage,
                logger=logger,
                cancel_event=cancel_event,
                tool_specs=tool_specs,
                emitter=emitter,
                _depth=_depth + 1,
            )

    return ExplorerResult(
        stations=list(state.stations),
        log_path=str(logger.path),
        stopped_reason=stopped_reason or "budget_exhausted",
    )
