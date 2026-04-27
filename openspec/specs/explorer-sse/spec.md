# explorer-sse Specification

## Purpose

TBD - created by archiving change 'agent-sse-wiring'. Update Purpose after archive.

## Requirements

### Requirement: POST /explore endpoint spawns Explorer under task registry

The sidecar SHALL expose a `POST /explore` endpoint that accepts a JSON body with `workspace_root: str`, `task: str`, and optional `budget_steps: int` / `budget_tokens: int` fields, validates the workspace root (exists, is a directory), creates an `explore`-kind task via the existing `TaskRegistry.create("explore")` single-slot store, spawns the Explorer agent as a background coroutine under the existing `_run_background_task` wrapper, and responds `202 Accepted` with `{"task_id": "explore_<8-hex>"}`. When another task is already in flight, the endpoint MUST respond `409 Conflict` with `{"code": "TASK_IN_FLIGHT"}`.

The background coroutine SHALL construct a fully-wired Explorer — `ToolContext` bound to the validated workspace root (with `sanitizer=SanitizerEngine()` and `kb=app.state.kb`/`usage_tracker=app.state.kb_usage_tracker(ws)` when configured), `FolderTools(ctx, state)`, `LLMJudge` built via `app.state.llm_judge_provider`, `ReasoningLogger(workspace_root / ".codebus" / "reasoning_log.jsonl")`, and a `TaskHandleEmitter(handle)` — then invoke `run_explorer(...)` with the emitter. On a clean return the wrapper emits `done`; on failure the wrapper emits a sanitized `error` event (existing spec `Background task error containment` continues to apply).

The endpoint coroutine MUST `mkdir(parents=True, exist_ok=True)` the `<workspace_root>/.codebus/` subdirectory **before** instantiating `ReasoningLogger`. Per the `agent-core` Requirement `ReasoningLogger appends one JSONL line per Step to workspace path`, `ReasoningLogger` does NOT auto-mkdir its parent directory (unlike `UsageTracker` / `LLMCallLogger` / `KBGrowthLogger` which do); the caller is responsible for ensuring the `.codebus/` subdirectory exists before construction. This invariant aligns with `audit-path-unification` (archive 2026-04-25), which moved all six workspace-level audit JSONLs from `<workspace_root>/` root into `<workspace_root>/.codebus/`.

#### Scenario: Happy path returns 202 with task_id

- **WHEN** `POST /explore` is called with a valid JSON body and a workspace root that exists as a directory
- **THEN** the response status MUST be `202`
- **AND** the response JSON MUST contain a `task_id` matching `^explore_[0-9a-f]{8}$`

#### Scenario: Concurrent task rejected

- **WHEN** `POST /explore` is called while another task (`scan` / `kb` / `explore`) is still `running`
- **THEN** the response status MUST be `409`
- **AND** the response body MUST contain `{"code": "TASK_IN_FLIGHT"}`

#### Scenario: Missing workspace root rejected

- **WHEN** `POST /explore` is called with a `workspace_root` that does not exist or is not a directory
- **THEN** the response status MUST be `400` (or `404`, at implementation discretion, but MUST NOT create a task handle)
- **AND** `TaskRegistry.current_running()` MUST remain unchanged

#### Scenario: Bearer authentication enforced

- **WHEN** `POST /explore` is called without a valid bearer token
- **THEN** the response status MUST be `401`, identical to every other sidecar endpoint's behavior

#### Scenario: ReasoningLogger lands under .codebus subdirectory

- **WHEN** the endpoint coroutine constructs the `ReasoningLogger` for a successfully validated workspace root `<ws>`
- **THEN** the logger's `path` attribute MUST equal `<ws>/.codebus/reasoning_log.jsonl`
- **AND** the endpoint coroutine MUST have called `(<ws> / ".codebus").mkdir(parents=True, exist_ok=True)` before constructing the logger (caller-mkdir invariant — `ReasoningLogger` does NOT auto-mkdir)
- **AND** subsequent `Step` writes from `run_explorer` MUST land in `<ws>/.codebus/reasoning_log.jsonl`, NOT in `<ws>/reasoning_log.jsonl`


<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->

---
### Requirement: Explorer loop emits agent_thought / agent_action_result / judge_verdict events

The sidecar SHALL extend `run_explorer` to accept an optional `emitter: SSEEmitter | None = None` parameter. When a non-None emitter is injected, each ReAct iteration MUST emit exactly three events in order, matching `docs/sidecar-api.md §四` wire schemas:

1. **After Think** — `{"type": "agent_thought", "step": N, "thought": "<text>", "action": [{"tool": "<name>", "args": {...}}]}` where `action` enumerates the `ExplorerAction.tool_calls` (empty list when the action carries no calls).
2. **After Act** — one `{"type": "agent_action_result", "step": N, "tool": "<name>", "observation": "<first 500 chars>", "tokens_used": <int>}` per `ToolResult`. `observation` MUST be truncated to ≤ 500 characters to prevent channel-flood; failed tools (`ToolResult.error is not None`) MUST also emit but with the truncated error message surfaced in `observation`. P0 implementation MAY emit `tokens_used: 0` as a placeholder until per-tool token attribution lands (currently `ToolResult` does not carry a `tokens_used` field; once it does, the emitter MUST forward the per-tool count). Consumers MUST treat any non-negative integer (including `0`) as valid; `0` does NOT mean "no tokens used", it signals "attribution not yet wired".
3. **After Judge** — `{"type": "judge_verdict", "step": N, "relevance": <float>, "reason": "<text>"}`.

When `emitter is None` the loop behavior MUST be identical to its prior form — no SSE side effects, no AttributeError, no performance regression beyond the negligible `None` check. This optionality preserves backward compatibility with every existing `run_explorer` caller (in-process tests, golden-sample replay, future Q&A integration) that does not wire SSE.

The loop MUST also emit one `{"type": "progress", "phase": "exploring", "current": step_count, "total": initial_budget_steps}` event per iteration so the frontend progress bar stays in sync with the Agent console stream.

#### Scenario: Three event types fire per iteration in order

- **WHEN** `run_explorer(..., emitter=test_emitter)` runs a single iteration with a non-empty `tool_calls` list
- **THEN** `test_emitter.emit` MUST be called with `type="agent_thought"`, then `type="agent_action_result"` (one per tool call), then `type="judge_verdict"`, in that sequence
- **AND** every event MUST carry the same `step` value equal to `state.step_count` at iteration start

#### Scenario: Missing emitter preserves legacy behavior

- **WHEN** `run_explorer(...)` is called without an `emitter` argument (default `None`)
- **THEN** no SSE emission MUST occur and the loop's return value MUST be identical to the pre-SSE form
- **AND** all existing Explorer loop tests MUST pass unchanged

#### Scenario: Observation truncation bounds channel payload

- **WHEN** a tool returns a 10_000-character output
- **THEN** the emitted `agent_action_result.observation` field MUST be at most 500 characters plus a truncation indicator
- **AND** the full output MUST still land in `reasoning_log.jsonl` verbatim

#### Scenario: tokens_used field accepts P0 placeholder zero

- **WHEN** `agent_action_result` events are inspected from a P0-stage Explorer run (no `tokens_used` field on `ToolResult` yet)
- **THEN** the `tokens_used` field on each emitted event MUST be a non-negative integer
- **AND** the value `0` MUST be treated as a valid P0 placeholder rather than a malformed event by every consumer (frontend Agent console, golden replay harness, integration tests)

---
### Requirement: TrackedProvider emits usage_delta on every completed call

The sidecar SHALL extend `TrackedProvider.__init__` to accept an optional `emitter: SSEEmitter | None = None` parameter. When a non-None emitter is injected, every successful `chat` / `embed` invocation MUST emit one `{"type": "usage_delta", "phase": <str|null>, "module": <default_module>, "step": <int|null>, "prompt_tokens": <int>, "completion_tokens": <int>, "cost_usd": <float>, "session_total_cost_usd": <float>, "session_total_tokens": <int>}` event after the wrapper writes its `token_usage.jsonl` line but before returning. Failed calls MUST NOT emit `usage_delta` (the existing `llm_calls.jsonl` failure record stands; cost accounting for retries is surfaced by the retrying caller's eventual success).

`phase` and `step` SHALL be read from module-level `contextvars.ContextVar` (`current_phase()` / `current_step()`) so Explorer loop can scope them per iteration without threading them through every call site. When the context vars are unset (e.g., KB build path), the fields MUST appear as JSON `null`.

`session_total_cost_usd` MUST reflect the TrackedProvider instance's in-memory running cost aggregate across all `chat` and `embed` calls made on that instance. `session_total_tokens` MUST reflect the same instance's in-memory sum of `session_prompt_tokens + session_completion_tokens` after the current call has been tallied, so each `usage_delta` carries both the per-call delta and the post-call running total.

#### Scenario: Emitter fires after token_usage.jsonl write

- **WHEN** `TrackedProvider(inner, ..., emitter=test_emitter).chat(msgs, response_model=M)` completes successfully
- **THEN** `test_emitter.emit` MUST be called exactly once with `type="usage_delta"` AFTER the `token_usage.jsonl` line is written
- **AND** the event's `module` field MUST equal the provider's `default_module`
- **AND** the event MUST carry a non-negative `session_total_tokens` integer whose value equals the post-call running sum of `prompt_tokens + completion_tokens` across every successful chat/embed on this TrackedProvider instance

#### Scenario: Failed call suppresses usage_delta

- **WHEN** the inner provider's `chat` raises an exception
- **THEN** `test_emitter.emit` MUST NOT receive a `usage_delta` event for this call
- **AND** the existing `llm_calls.jsonl` failure record MUST still land (pre-existing contract)
- **AND** `session_total_tokens` MUST NOT advance on the TrackedProvider instance (failed calls do not count)

#### Scenario: Omitting emitter preserves existing behavior

- **WHEN** `TrackedProvider` is constructed without the `emitter` kwarg (M2 existing call sites)
- **THEN** every existing test in `sidecar/tests/providers/` MUST pass unchanged
- **AND** no SSE emit MUST occur

#### Scenario: Session total tokens accumulate across successive calls

- **WHEN** a single TrackedProvider instance makes three successful chat calls in sequence with token usages `(p1, c1)`, `(p2, c2)`, `(p3, c3)`
- **THEN** the third `usage_delta` event's `session_total_tokens` field MUST equal `p1 + c1 + p2 + c2 + p3 + c3`
- **AND** a fourth call that raises an exception MUST leave `session_total_tokens` unchanged relative to the third event's value

---
### Requirement: LLMCallLogger emits llm_call event carrying preview

The sidecar SHALL extend `LLMCallLogger` to accept an optional `emitter: SSEEmitter | None = None` at construction. When a non-None emitter is injected, every `log(...)` call (covering both successful and failed wire payloads) MUST emit one `{"type": "llm_call", "request_id": <str>, "module": <str>, "step_id": <int|null>, "role": <str>, "provider": <str>, "model": <str>, "call_type": <str>, "latency_ms": <int|null>, "tokens": {"prompt": <int>, "completion": <int>}, "cost_usd": <float|null>, "preview": <str>}` event. The `preview` field MUST be truncated to 200 characters drawn from the first user message's content so large prompts do not flood the SSE channel; the detail endpoint (future addition) serves full payloads on demand.

Emission MUST land the full `llm_calls.jsonl` line regardless of whether the emitter succeeds or fails; disk write is authoritative, SSE is best-effort wire.

#### Scenario: Successful call emits llm_call event

- **WHEN** `LLMCallLogger(..., emitter=test_emitter).log(request=..., response=..., ...)` runs
- **THEN** `test_emitter.emit` MUST be called with `type="llm_call"`
- **AND** the event's `preview` field MUST be a string of length at most 200 characters
- **AND** the corresponding `llm_calls.jsonl` line MUST still be written (wire-log parity)

#### Scenario: Failed call still emits llm_call event

- **WHEN** `LLMCallLogger(..., emitter=test_emitter).log_failure(request=..., exception=..., ...)` runs
- **THEN** `test_emitter.emit` MUST receive an `llm_call` event even though the call failed
- **AND** the event's `latency_ms` MAY be null but the `request_id` / `model` / `module` fields MUST be present

#### Scenario: Omitted emitter preserves file-only behavior

- **WHEN** `LLMCallLogger` is constructed without the `emitter` kwarg
- **THEN** every existing `llm_calls.jsonl` test MUST pass unchanged
- **AND** no SSE emit MUST occur

---
### Requirement: SSEEmitter is an opt-in runtime-checkable Protocol

The sidecar SHALL define a `codebus_agent.agent.emitter.SSEEmitter` structural Protocol with a single method `emit(event: dict) -> None` and `@runtime_checkable` so tests can assert conformance. The module SHALL provide two concrete implementations:

1. `NullEmitter` — a no-op emitter that silently accepts every event; the default when no SSE channel exists (in-process testing, golden-sample replay).
2. `TaskHandleEmitter` — wraps a `TaskHandle` instance and delegates `emit(event)` to `handle.emit(event)`, routing events through the existing `sse-progress-skeleton` fan-out machinery.

Callers that need to emit without checking `emitter is None` SHALL construct a `NullEmitter()` and pass it unconditionally; this lets the hot path drop the nullability check at the cost of one extra construction.

#### Scenario: NullEmitter satisfies Protocol

- **WHEN** a test passes `NullEmitter()` as the `emitter` argument to `run_explorer` / `TrackedProvider` / `LLMCallLogger`
- **THEN** every call site MUST behave identically to passing `None`
- **AND** `isinstance(NullEmitter(), SSEEmitter)` MUST return True

#### Scenario: TaskHandleEmitter fans out to subscribers

- **WHEN** `TaskHandleEmitter(handle).emit({"type": "progress", ...})` runs
- **THEN** every subscriber queue registered on `handle` MUST receive the event
- **AND** the existing `TaskHandle.emit` fan-out contract MUST be respected

---
### Requirement: Coverage round emits coverage_gaps SSE event

The sidecar SHALL emit exactly one `coverage_gaps` SSE event per coverage round through the `run_explorer` emitter. The event MUST fire after `coverage.check(state)` returns and before recursion is decided (including the decision not to recurse). When `emitter is None` the event MUST NOT be emitted, preserving the legacy file-only behaviour for in-process tests and golden-sample replay.

The event envelope MUST match this wire schema:

```json
{
  "type": "coverage_gaps",
  "round": <int>,
  "gaps": [
    {"description": "<str>", "suggested_target": "<str|null>"}
  ],
  "will_recurse": <bool>,
  "skip_reason": "<str|null>"
}
```

Field semantics:

- `round` MUST equal the `_depth` value at which `coverage.check` was invoked (0-indexed; the first coverage round after the outermost `run_explorer` is `round=0`).
- `gaps` MUST be a JSON array of `Gap.model_dump()` outputs (each a `{description, suggested_target}` object). An empty array signals no gap found.
- `will_recurse` MUST be `true` if and only if all three recursion preconditions hold: `len(gaps) > 0`, `state.budget_steps_left > 0`, and `_depth < _COVERAGE_MAX_DEPTH`.
- `skip_reason` MUST be `null` when `will_recurse=true`. When `will_recurse=false`, it MUST be exactly one of `"no_gaps"` (gaps empty), `"budget_exhausted"` (budget at or below 0 with gaps present), or `"max_depth_reached"` (`_depth` at or above `_COVERAGE_MAX_DEPTH` with gaps and budget otherwise sufficient). When more than one blocking condition holds simultaneously, `skip_reason` MUST follow this precedence: `no_gaps` > `max_depth_reached` > `budget_exhausted`.

#### Scenario: coverage_gaps event fires after check returns gaps and before recursion

- **WHEN** `run_explorer(..., emitter=test_emitter, _depth=0)` completes its main loop, `coverage.check(state)` returns two `Gap` entries, `state.budget_steps_left == 4`, and `_COVERAGE_MAX_DEPTH == 3`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `round=0`, `gaps` a two-element list, `will_recurse=true`, and `skip_reason=null`
- **AND** the event MUST be emitted before the recursive `run_explorer(..., _depth=1)` call begins

#### Scenario: Empty gaps still emit with skip_reason="no_gaps"

- **WHEN** `run_explorer(..., emitter=test_emitter)` completes its main loop and `coverage.check(state)` returns `[]`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps=[]`, `will_recurse=false`, and `skip_reason="no_gaps"`
- **AND** no recursion MUST occur

#### Scenario: Budget-exhausted round emits skip_reason="budget_exhausted"

- **WHEN** `run_explorer(..., emitter=test_emitter)` completes with `state.budget_steps_left == 0` and `coverage.check(state)` returns one `Gap`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps` a one-element list, `will_recurse=false`, and `skip_reason="budget_exhausted"`

#### Scenario: Max-depth round emits skip_reason="max_depth_reached"

- **WHEN** `run_explorer(..., emitter=test_emitter, _depth=2)` completes with `state.budget_steps_left == 10`, `_COVERAGE_MAX_DEPTH == 3`, and `coverage.check(state)` returns one `Gap`
- **THEN** `test_emitter.emit` MUST receive exactly one event with `type="coverage_gaps"`, `gaps` a one-element list, `will_recurse=false`, and `skip_reason="max_depth_reached"`

#### Scenario: Missing emitter preserves legacy behavior

- **WHEN** `run_explorer(...)` runs with `emitter=None` and a coverage round fires
- **THEN** no SSE emission MUST occur for the coverage round
- **AND** the recursive-call decision (recurse / return) MUST be unchanged relative to the emitter-set case

---
### Requirement: Explorer emits budget_warning SSE event at 80% threshold

The sidecar SHALL emit a `budget_warning` SSE event through the `run_explorer` emitter at most once per budget kind (`"tokens"` or `"steps"`) per invocation of `run_explorer`, when consumption first reaches or exceeds 80% of the configured budget for that kind. The threshold constant `_BUDGET_WARNING_PCT: float = 0.8` MUST live module-level in `codebus_agent.agent.explorer`.

The event envelope MUST match this wire schema:

```json
{
  "type": "budget_warning",
  "kind": "tokens" | "steps",
  "current": <int>,
  "budget": <int>,
  "pct": <float>
}
```

Field semantics:

- `kind` MUST be exactly `"tokens"` when triggered by `token_probe.total()` crossing the threshold against `state.budget_tokens_left`, and exactly `"steps"` when triggered by `(initial_budget_steps - state.budget_steps_left)` crossing the threshold against `initial_budget_steps`.
- `current` MUST be the consumed value at emit time (`token_probe.total()` for tokens, `initial_budget_steps - state.budget_steps_left` for steps).
- `budget` MUST be the configured budget value (`state.budget_tokens_left` as captured at call entry for tokens, `initial_budget_steps` snapshot for steps).
- `pct` MUST be `current / budget` as a float rounded to at most six decimal places.

The emitter SHALL be evaluated once per iteration, immediately after `state.step_count` / `state.budget_steps_left` are updated and before the `progress` event is emitted. When `emitter is None` the warning MUST NOT be emitted. When `token_probe is None` the `"tokens"` branch MUST NOT fire.

Each kind MUST be suppressed after a single successful emit within the same `run_explorer` invocation (including across coverage-gap recursion frames that share the same run). The loop MUST NOT emit duplicate warnings for the same kind even when consumption continues to climb.

#### Scenario: First iteration crossing step threshold emits warning

- **WHEN** `run_explorer` runs with `initial_budget_steps = 5` and iteration 4 completes (so consumed = 4, 4/5 = 0.8)
- **THEN** the emitter MUST receive exactly one event with `type="budget_warning"`, `kind="steps"`, `current=4`, `budget=5`, `pct=0.8`
- **AND** the event MUST be emitted before the iteration's `progress` event
- **AND** subsequent iterations MUST NOT emit a second `steps` warning on this run

#### Scenario: Token budget crosses threshold before step budget

- **WHEN** `run_explorer` runs with a scripted `token_probe` that reports `total()` crossing `0.8 * state.budget_tokens_left` on iteration 2, and step budget is never crossed
- **THEN** the emitter MUST receive exactly one event with `type="budget_warning"`, `kind="tokens"` with matching `current` / `budget` / `pct` fields
- **AND** no `budget_warning` event with `kind="steps"` MUST fire for this run

#### Scenario: Both thresholds cross in the same run emit once per kind

- **WHEN** a run's `token_probe` crosses 80% in iteration 3 and its step consumption crosses 80% in iteration 7
- **THEN** the emitter MUST receive exactly one `kind="tokens"` event and exactly one `kind="steps"` event across the entire run (including any coverage-gap recursion frames)
- **AND** neither warning MUST fire a second time even if consumption continues to climb

#### Scenario: Missing emitter suppresses all warnings

- **WHEN** `run_explorer` is called with `emitter=None` and budget thresholds are crossed
- **THEN** no `budget_warning` event MUST be emitted anywhere
- **AND** the loop terminal behaviour MUST remain identical to the emitter-set case

#### Scenario: Missing token probe suppresses tokens warning only

- **WHEN** `run_explorer` is called with `emitter=test_emitter` and `token_probe=None`
- **THEN** no `budget_warning` event with `kind="tokens"` MUST be emitted regardless of `state.budget_tokens_left` value
- **AND** the `kind="steps"` warning MUST still fire when the step threshold is crossed
