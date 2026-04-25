## MODIFIED Requirements

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


## ADDED Requirements

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
