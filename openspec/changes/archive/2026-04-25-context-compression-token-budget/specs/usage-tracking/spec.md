## ADDED Requirements

### Requirement: TrackedProvider exposes session token counters

The sidecar SHALL extend `codebus_agent.providers.tracked.TrackedProvider` with three in-memory running counters tracking token consumption for the lifetime of a single TrackedProvider instance:

- `session_prompt_tokens: int` — monotonic running sum of `prompt_tokens` reported to `UsageTracker.record` across every successful `chat` and `embed` call on this instance.
- `session_completion_tokens: int` — monotonic running sum of `completion_tokens` reported to `UsageTracker.record` across every successful `chat` and `embed` call on this instance.
- `session_total_tokens: int` — read-only property returning `session_prompt_tokens + session_completion_tokens`.

All three MUST start at `0` at construction time. All three MUST increment only on successful completion of `chat` / `embed` (the same path that advances `session_total_cost_usd`). Calls that raise an exception MUST leave the counters unchanged — mirroring the `session_total_cost_usd` contract in design decision D-022 and the existing `usage_delta on success only` scenario.

The counters MUST live on the TrackedProvider instance (not on a shared module-level accumulator) so per-workspace and per-task providers maintain independent token budgets without contamination between tasks.

Token values MUST come from the same prompt_tokens / completion_tokens integers that `UsageTracker.record` receives (i.e., the estimated or provider-reported token counts established by D-021); the counters MUST NOT introduce a separate token estimator.

These counters MUST be accessible by external callers (e.g., `codebus_agent.agent.budget.AggregatedTokenProbe`) so Explorer ReAct loop can aggregate totals across multiple TrackedProvider instances (reasoning / judge / coverage roles) without reading back `token_usage.jsonl` from disk.

#### Scenario: Counters start at zero

- **WHEN** a `TrackedProvider(inner, ..., emitter=None)` is freshly constructed
- **THEN** `session_prompt_tokens`, `session_completion_tokens`, and `session_total_tokens` MUST all equal `0`

#### Scenario: Successful chat advances both counters

- **WHEN** a successful `chat(msgs, response_model=M)` call reports `prompt_tokens=42` and `completion_tokens=7` to `UsageTracker.record`
- **THEN** `session_prompt_tokens` MUST advance by exactly `42`
- **AND** `session_completion_tokens` MUST advance by exactly `7`
- **AND** `session_total_tokens` MUST equal the new `session_prompt_tokens + session_completion_tokens`

#### Scenario: Failed chat leaves counters unchanged

- **WHEN** a `chat` call raises an exception (e.g., `OpenAIContextLengthError`) before returning
- **THEN** `session_prompt_tokens`, `session_completion_tokens`, and `session_total_tokens` MUST retain their pre-call values
- **AND** `llm_calls.jsonl` MUST still record the failure wire payload per the pre-existing contract

#### Scenario: Embed path contributes to prompt counter only

- **WHEN** a successful `embed(texts)` call reports `embed_tokens=15` and `completion_tokens=0` to `UsageTracker.record`
- **THEN** `session_prompt_tokens` MUST advance by exactly `15`
- **AND** `session_completion_tokens` MUST NOT change for that call
- **AND** `session_total_tokens` MUST reflect the updated sum

#### Scenario: Counters are per-instance not shared

- **WHEN** two `TrackedProvider` instances are constructed from the same factory (e.g., two `LLMJudge` tasks in different workspaces)
- **THEN** successful `chat` on one instance MUST NOT mutate any counter on the other instance
- **AND** each instance MUST maintain its own independent token running total
