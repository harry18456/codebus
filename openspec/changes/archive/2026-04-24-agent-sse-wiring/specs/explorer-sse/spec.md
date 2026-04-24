## ADDED Requirements

### Requirement: POST /explore endpoint spawns Explorer under task registry

The sidecar SHALL expose a `POST /explore` endpoint that accepts a JSON body with `workspace_root: str`, `task: str`, and optional `budget_steps: int` / `budget_tokens: int` fields, validates the workspace root (exists, is a directory), creates an `explore`-kind task via the existing `TaskRegistry.create("explore")` single-slot store, spawns the Explorer agent as a background coroutine under the existing `_run_background_task` wrapper, and responds `202 Accepted` with `{"task_id": "explore_<8-hex>"}`. When another task is already in flight, the endpoint MUST respond `409 Conflict` with `{"code": "TASK_IN_FLIGHT"}`.

The background coroutine SHALL construct a fully-wired Explorer — `ToolContext` bound to the validated workspace root (with `sanitizer=SanitizerEngine()` and `kb=app.state.kb`/`usage_tracker=app.state.kb_usage_tracker(ws)` when configured), `FolderTools(ctx, state)`, `LLMJudge` built via `app.state.llm_judge_provider`, `ReasoningLogger(workspace_root / "reasoning_log.jsonl")`, and a `TaskHandleEmitter(handle)` — then invoke `run_explorer(...)` with the emitter. On a clean return the wrapper emits `done`; on failure the wrapper emits a sanitized `error` event (existing spec `Background task error containment` continues to apply).

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


### Requirement: Explorer loop emits agent_thought / agent_action_result / judge_verdict events

The sidecar SHALL extend `run_explorer` to accept an optional `emitter: SSEEmitter | None = None` parameter. When a non-None emitter is injected, each ReAct iteration MUST emit exactly three events in order, matching `docs/sidecar-api.md §四` wire schemas:

1. **After Think** — `{"type": "agent_thought", "step": N, "thought": "<text>", "action": [{"tool": "<name>", "args": {...}}]}` where `action` enumerates the `ExplorerAction.tool_calls` (empty list when the action carries no calls).
2. **After Act** — one `{"type": "agent_action_result", "step": N, "tool": "<name>", "observation": "<first 500 chars>", "tokens_used": <int>}` per `ToolResult`. `observation` MUST be truncated to ≤ 500 characters to prevent channel-flood; failed tools (`ToolResult.error is not None`) MUST also emit but with the truncated error message surfaced in `observation`.
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


### Requirement: TrackedProvider emits usage_delta on every completed call

The sidecar SHALL extend `TrackedProvider.__init__` to accept an optional `emitter: SSEEmitter | None = None` parameter. When a non-None emitter is injected, every successful `chat` / `embed` invocation MUST emit one `{"type": "usage_delta", "phase": <str|null>, "module": <default_module>, "step": <int|null>, "prompt_tokens": <int>, "completion_tokens": <int>, "cost_usd": <float>, "session_total_cost_usd": <float>}` event after the wrapper writes its `token_usage.jsonl` line but before returning. Failed calls MUST NOT emit `usage_delta` (the existing `llm_calls.jsonl` failure record stands; cost accounting for retries is surfaced by the retrying caller's eventual success).

`phase` and `step` SHALL be read from module-level `contextvars.ContextVar` (`current_phase()` / `current_step()`) so Explorer loop can scope them per iteration without threading them through every call site. When the context vars are unset (e.g., KB build path), the fields MUST appear as JSON `null`.

`session_total_cost_usd` MUST come from `UsageTracker.session_total()` (existing aggregate) so each `usage_delta` carries both the delta and the running total.

#### Scenario: Emitter fires after token_usage.jsonl write

- **WHEN** `TrackedProvider(inner, ..., emitter=test_emitter).chat(msgs, response_model=M)` completes successfully
- **THEN** `test_emitter.emit` MUST be called exactly once with `type="usage_delta"` AFTER the `token_usage.jsonl` line is written
- **AND** the event's `module` field MUST equal the provider's `default_module`

#### Scenario: Failed call suppresses usage_delta

- **WHEN** the inner provider's `chat` raises an exception
- **THEN** `test_emitter.emit` MUST NOT receive a `usage_delta` event for this call
- **AND** the existing `llm_calls.jsonl` failure record MUST still land (pre-existing contract)

#### Scenario: Omitting emitter preserves existing behavior

- **WHEN** `TrackedProvider` is constructed without the `emitter` kwarg (M2 existing call sites)
- **THEN** every existing test in `sidecar/tests/providers/` MUST pass unchanged
- **AND** no SSE emit MUST occur


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
