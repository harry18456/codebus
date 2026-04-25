# usage-tracking Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: UsageTracker writes token_usage.jsonl

The sidecar SHALL implement a `UsageTracker` that appends one JSON line per LLM call to `<workspace>/.codebus/token_usage.jsonl`, per `docs/decisions.md` D-021 and `docs/agent-core.md §十三`. The path lives under the `.codebus/` subdirectory of the workspace root (consistent with the workspace-level audit chain convention shared by `<workspace>/.codebus/sanitize_audit.jsonl` and `<workspace>/.codebus/tool_audit.jsonl`); the tracker's constructor MUST auto-create the parent `.codebus/` directory if absent so callers do not have to pre-mkdir. The `module` field on each line SHALL reflect the calling subsystem (e.g., `"kb_build"`, `"qa_agent"`); when `TrackedProvider` is constructed with `default_module`, that value SHALL be carried automatically into every record it writes, so callers do not duplicate the `record(...)` call themselves.

#### Scenario: One line per chat call

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `<workspace>/.codebus/token_usage.jsonl`

#### Scenario: Required fields present

- **WHEN** a line from `<workspace>/.codebus/token_usage.jsonl` is parsed
- **THEN** it MUST contain the keys `timestamp`, `provider`, `model`, `operation`, `input_tokens`, `output_tokens`, and `cost_usd` with non-null values

#### Scenario: Embed calls tracked

- **WHEN** an `LLMProvider.embed` call completes through the tracked wrapper
- **THEN** a line with `operation="embed"` and `output_tokens=0` MUST be appended to `<workspace>/.codebus/token_usage.jsonl`

#### Scenario: Module field reflects TrackedProvider's default_module

- **WHEN** a `TrackedProvider` is constructed with `default_module="kb_build"` and an `embed` call completes through it
- **THEN** the appended `<workspace>/.codebus/token_usage.jsonl` line MUST contain `"module": "kb_build"`, and no second line with the same `(timestamp, model, input_tokens)` tuple MUST be appended by any other layer (e.g., `KnowledgeBase` MUST NOT call `tracker.record()` itself)

#### Scenario: Default module absent yields empty string

- **WHEN** a `TrackedProvider` is constructed without `default_module` (or `default_module=None`) and a call completes through it
- **THEN** the appended line's `module` field MUST be the empty string `""`, preserving backward compatibility with M1 records that did not carry a module label

---
### Requirement: LLMCallLogger writes llm_calls.jsonl

The sidecar SHALL implement an `LLMCallLogger` that appends the full request and response payload (as seen by the provider) for every call, per `docs/decisions.md` D-022. The default workspace-scoped path SHALL be `<workspace>/.codebus/llm_calls.jsonl` — under the `.codebus/` subdirectory of the workspace root, consistent with `<workspace>/.codebus/sanitize_audit.jsonl` / `<workspace>/.codebus/tool_audit.jsonl` / `<workspace>/.codebus/token_usage.jsonl`. The logger's constructor MUST auto-create the parent `.codebus/` directory if absent so callers do not have to pre-mkdir.

#### Scenario: Request and response captured

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `<workspace>/.codebus/llm_calls.jsonl` containing `request` (the exact payload sent to the provider) and `response` (the exact payload received)

#### Scenario: Sanitizer-ready field reserved

- **WHEN** a line from `<workspace>/.codebus/llm_calls.jsonl` is parsed
- **THEN** it MUST contain a `sanitizer_pass2_applied` boolean field whose value reflects whether Sanitizer Pass 2 was applied to the request before dispatch (production code post-`sanitizer-safety-chain` always sets `true`; the field exists so future changes that gate Pass 2 on per-call conditions can vary it)

#### Scenario: Failure still logged

- **WHEN** an `LLMProvider.chat` call raises an exception through the tracked wrapper
- **THEN** a line with `response: null` and an `error` field describing the exception class and message MUST be appended to `<workspace>/.codebus/llm_calls.jsonl`

---
### Requirement: TrackedProvider wraps every provider

All LLM calls in the sidecar SHALL pass through a `TrackedProvider` decorator that delegates to an inner `LLMProvider` and invokes `UsageTracker` and `LLMCallLogger` before returning, per design decision D-local-4.

#### Scenario: Direct provider use forbidden

- **WHEN** the provider registry is queried
- **THEN** every registered provider instance MUST be wrapped in `TrackedProvider`

#### Scenario: Wrapper preserves protocol shape

- **WHEN** `TrackedProvider(MockProvider())` is checked against the `LLMProvider` protocol
- **THEN** static type analysis MUST accept the wrapper as an `LLMProvider` subtype

#### Scenario: Skipping wrapper emits test failure

- **WHEN** an integration test calls `LLMProvider.chat` without going through `TrackedProvider`
- **THEN** an enforcement check in the provider registry MUST raise at instantiation time, so the unwrapped path is not reachable from production code

<!-- @trace
source: m1-power-on
updated: 2026-04-19
-->

---
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
