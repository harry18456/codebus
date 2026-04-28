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
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any

from codebus_agent.providers.protocol import Message as ProviderMessage
from codebus_agent.providers.tracked import TrackedProvider
from codebus_agent.sanitizer import (
    MessageSource,
    SanitizerAuditLogger,
    SanitizerEngine,
)

from .budget import TokenBudgetProbe
from .emitter import NullEmitter, SSEEmitter
from .prompts.explorer import (
    EXPLORER_SYSTEM,
    render_explorer_prompt,
)
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
# `context-compression-token-budget`: fixed FIFO window on `state.messages`
# when forwarding the provider wire prompt. Applies only to Explorer's
# cross-iteration ReAct path — `state.messages` itself is NOT mutated.
_MESSAGE_ROLLING_WINDOW: int = 16
# `context-compression-token-budget`: threshold (fraction of configured
# budget) at which an Explorer run emits a one-time `budget_warning` SSE
# event per kind (`tokens` / `steps`). Tuneable constant — bump if Demo
# shows 80% feels too late.
_BUDGET_WARNING_PCT: float = 0.8


@dataclass
class _BudgetWarningState:
    """Per-run sticky flags so `budget_warning` fires at most once per kind.

    Built at the outermost `run_explorer` call and re-used across
    coverage-gap recursion frames (state is threaded through the tail
    call) so a user sees one warning per kind over the full session.
    """

    warned_tokens: bool = False
    warned_steps: bool = False
# `_enqueue_gap_investigation` renders at most this many gap descriptions
# into the user-message summary (Decision 6); remaining gaps collapse into
# `（及其他 N 項）` so the prompt stays bounded even on pathological rounds.
_COVERAGE_SUMMARY_MAX_GAPS: int = 3
_COVERAGE_SUMMARY_DESC_TRUNCATE: int = 60
_COVERAGE_QUEUE_PLACEHOLDER_TRUNCATE: int = 80


def _should_stop(
    state: ExplorerState,
    cancel_event: asyncio.Event | None,
    token_probe: TokenBudgetProbe | None = None,
) -> tuple[bool, str | None]:
    """Return `(stop?, stopped_reason)`.

    Four-branch precedence (Decision 3): cancel > token > steps > queue.
    `token_probe=None` skips the token branch so legacy in-process
    tests and golden-sample replay stay unaffected.
    """
    if cancel_event is not None and cancel_event.is_set():
        return True, "cancelled"
    if token_probe is not None and token_probe.total() >= state.budget_tokens_left:
        return True, "budget_tokens_exhausted"
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
    """Render prompt → single chat call → return (thought, tool_calls).

    Only the **last `_MESSAGE_ROLLING_WINDOW`** entries of `state.messages`
    are forwarded to the provider; earlier ones stay on state for the
    reasoning log. `render_explorer_prompt` still consumes the full
    state (visited / stations summary) so the LLM never loses the
    accumulated exploration context.
    """
    user_prompt = render_explorer_prompt(state, tool_specs)
    windowed = state.messages[-_MESSAGE_ROLLING_WINDOW:]
    messages = _to_provider_messages(windowed) + [
        ProviderMessage(role="system", content=EXPLORER_SYSTEM),
        ProviderMessage(role="user", content=user_prompt),
    ]
    action = await provider.chat(messages, response_model=ExplorerAction)
    assert isinstance(action, ExplorerAction), (
        "TrackedProvider.chat(response_model=ExplorerAction) must return "
        "a validated ExplorerAction instance"
    )
    return action.thought, action.tool_calls


async def _execute_one(
    call: ToolCall,
    tools: ExplorerTools,
    *,
    error_sanitize_fn: "Callable[[str], Awaitable[str]] | None" = None,
) -> ToolResult:
    """Route a tool call to the matching method on the tools impl.

    Missing methods and raised exceptions both collapse into
    ``ToolResult.error`` so the loop can record the failure and move on
    (spec scenario `Tool errors do not crash the loop`).

    When ``error_sanitize_fn`` is provided, the error string written
    into ``ToolResult.output`` is run through Pass 2 sanitize first
    (D2.19 — `Tool error string sanitized through Pass 2`). The
    callable owns the per-iteration ``MessageSource(message_id=...)``
    binding so this helper stays oblivious to ``state.step_count``.
    Backward-compat: ``None`` keeps the legacy raw-error behaviour for
    tests that don't wire a sanitizer.
    """
    method = getattr(tools, call.name, None)
    if method is None or not callable(method):
        msg = f"unknown tool {call.name!r}"
        raw_error_text = f"ERROR: {msg}"
        out = await error_sanitize_fn(raw_error_text) if error_sanitize_fn else raw_error_text
        return ToolResult(
            tool_call_id=call.id,
            tool_name=call.name,
            output=out,
            raw=None,
            error=msg,
        )
    try:
        output = await method(**call.arguments)
    except BaseException as exc:  # noqa: BLE001 — capture then record
        raw_error_text = f"ERROR: {exc}"
        out = await error_sanitize_fn(raw_error_text) if error_sanitize_fn else raw_error_text
        return ToolResult(
            tool_call_id=call.id,
            tool_name=call.name,
            output=out,
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
    calls: list[ToolCall],
    tools: ExplorerTools,
    *,
    error_sanitize_fn: Callable[[str], str] | None = None,
) -> list[ToolResult]:
    if not calls:
        return []
    return list(
        await asyncio.gather(
            *[
                _execute_one(c, tools, error_sanitize_fn=error_sanitize_fn)
                for c in calls
            ]
        )
    )


def _make_error_sanitize_fn(
    *,
    sanitizer: SanitizerEngine | None,
    audit: SanitizerAuditLogger | None,
    session_id: str,
    rules_version: str,
    step_idx: int,
) -> "Callable[[str], Awaitable[str]] | None":
    """Build the per-iteration error-sanitize callable for D2.19.

    Returns ``None`` when ``sanitizer`` is absent so the loop falls
    back to raw error text (backward compat for legacy tests). When
    present, the returned callable runs Pass 2 sanitize tagged
    ``MessageSource(message_id=f"explorer_step_{step_idx}_tool_error")``
    and appends each hit to ``sanitize_audit.jsonl`` with ``pass_num=2``.
    """
    if sanitizer is None:
        return None

    async def _sanitize_error(error_text: str) -> str:
        result = await sanitizer.sanitize(
            error_text,
            source=MessageSource(
                message_id=f"explorer_step_{step_idx}_tool_error"
            ),
        )
        if audit is not None:
            for entry in result.entries:
                audit.append(
                    entry=entry,
                    pass_num=2,
                    rules_version=rules_version,
                    session_id=session_id,
                )
        return result.text

    return _sanitize_error


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


def _maybe_emit_budget_warning(
    emitter: SSEEmitter | None,
    warning_state: _BudgetWarningState,
    state: ExplorerState,
    initial_budget_steps: int,
    token_probe: TokenBudgetProbe | None,
) -> None:
    """Fan out a `budget_warning` event once per kind per run.

    Called after the Update step (budget_steps_left decremented) and
    before the iteration's `progress` emit. No-op when `emitter is None`.
    Tokens branch skipped when `token_probe is None`.
    """
    if emitter is None:
        return

    # tokens branch
    if (
        token_probe is not None
        and not warning_state.warned_tokens
        and state.budget_tokens_left > 0
    ):
        current = token_probe.total()
        budget = state.budget_tokens_left
        if current / budget >= _BUDGET_WARNING_PCT:
            emitter.emit(
                {
                    "type": "budget_warning",
                    "kind": "tokens",
                    "current": int(current),
                    "budget": int(budget),
                    "pct": round(current / budget, 6),
                }
            )
            warning_state.warned_tokens = True

    # steps branch
    if (
        not warning_state.warned_steps
        and initial_budget_steps > 0
    ):
        consumed = initial_budget_steps - state.budget_steps_left
        if consumed / initial_budget_steps >= _BUDGET_WARNING_PCT:
            emitter.emit(
                {
                    "type": "budget_warning",
                    "kind": "steps",
                    "current": int(consumed),
                    "budget": int(initial_budget_steps),
                    "pct": round(consumed / initial_budget_steps, 6),
                }
            )
            warning_state.warned_steps = True


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
    token_probe: TokenBudgetProbe | None = None,
    sanitizer: SanitizerEngine | None = None,
    sanitizer_audit: SanitizerAuditLogger | None = None,
    session_id: str = "",
    rules_version: str = "",
    _depth: int = 0,
    _warning_state: _BudgetWarningState | None = None,
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
    # Sticky warning state — constructed at the outermost frame and
    # reused across coverage-gap recursion so `budget_warning` fires at
    # most once per kind per Explorer session.
    warning_state = _warning_state or _BudgetWarningState()

    while True:
        should_stop, stopped_reason = _should_stop(
            state, cancel_event, token_probe
        )
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

        # 2. Act (parallel tool dispatch; errors captured into ToolResult.error).
        # D2.19: when a sanitizer is wired, the tool error path runs Pass 2
        # sanitize before populating `ToolResult.output` so user input
        # echoed in exception messages cannot reach the next iteration's
        # LLM context unsanitized. The factory binds the per-iteration
        # `MessageSource(message_id=...)` so `_execute_one` stays oblivious.
        error_sanitize_fn = _make_error_sanitize_fn(
            sanitizer=sanitizer,
            audit=sanitizer_audit,
            session_id=session_id,
            rules_version=rules_version,
            step_idx=iter_step,
        )
        results = await _execute_tools(
            tool_calls, tools, error_sanitize_fn=error_sanitize_fn
        )
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

        # Budget warning — per-kind once, fires after Update and before
        # `progress` so consumers see the warning alongside the progress
        # tick that pushed consumption past the threshold.
        _maybe_emit_budget_warning(
            emitter,
            warning_state,
            state,
            initial_budget_steps,
            token_probe,
        )

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
                token_probe=token_probe,
                sanitizer=sanitizer,
                sanitizer_audit=sanitizer_audit,
                session_id=session_id,
                rules_version=rules_version,
                _depth=_depth + 1,
                _warning_state=warning_state,
            )

    return ExplorerResult(
        stations=list(state.stations),
        log_path=str(logger.path),
        stopped_reason=stopped_reason or "budget_exhausted",
    )
