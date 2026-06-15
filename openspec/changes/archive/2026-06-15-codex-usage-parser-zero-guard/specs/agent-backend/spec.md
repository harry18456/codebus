## MODIFIED Requirements

### Requirement: Provider-Declared Token Usage Semantics

Each `AgentBackend` implementation SHALL declare how its emitted `Usage` token events combine across one invocation, via an opt-in trait method `token_usage_semantics(&self) -> TokenUsageSemantics` with a default return of `TokenUsageSemantics::Delta`. `TokenUsageSemantics` SHALL be a closed enum with exactly two variants: `Delta` (each `Usage` event reports the tokens attributable to that event alone; the per-invocation total is the field-wise sum of all events) and `Cumulative` (each `Usage` event reports a running total for the invocation so far; the per-invocation total is the latest non-empty event, NOT a sum). The Claude backend SHALL use the default `Delta` (the Claude CLI emits one `result` usage event per `-p` run). The codex backend SHALL override to `Cumulative` (the codex `turn.completed.usage` field carries a cumulative total, not a per-turn delta).

`agent::invoke` SHALL read the backend's declared semantics once and combine each `Usage` event into the accumulated `TokenUsage` accordingly: under `Delta` it SHALL field-wise sum (the existing `accumulate_token_usage` behavior); under `Cumulative` it SHALL replace the accumulated value with the latest non-empty event and SHALL ignore an empty event whose normalized token counts are all zero. A `Cumulative` event is empty when `input_tokens == 0`, `output_tokens == 0`, `cache_read_tokens.unwrap_or(0) == 0`, `cache_write_tokens.unwrap_or(0) == 0`, and `reasoning_tokens.unwrap_or(0) == 0`; `TokenUsage.extras` SHALL NOT affect this emptiness check. This dispatch SHALL remain provider-agnostic per the `Invocation Loop Drives Backend Trait` requirement - the loop SHALL branch on the `TokenUsageSemantics` value only and SHALL NOT reference any provider binary name, provider argv flag, or provider stream-json field name. The resulting accumulated `TokenUsage` is the value recorded as `RunLog.tokens` per the `run-log` capability; for a `Cumulative` backend this value is the run's final cumulative total and SHALL NOT be double-counted across multiple `Usage` events.

This requirement SHALL NOT alter the serialized shape of `StreamEvent` (events.jsonl) or `TokenUsage` (runs.jsonl): `TokenUsageSemantics` is a transient combination directive used only inside `invoke` and SHALL NOT be serialized into either jsonl format.

#### Scenario: Delta backend sums usage events

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Delta` AND the stream yields two `Usage` events with `input_tokens` 100 then 25
- **THEN** the accumulated `RunLog.tokens.input_tokens` SHALL equal 125 (field-wise sum)

#### Scenario: Cumulative backend takes the latest non-empty usage snapshot

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Cumulative` AND the stream yields two non-empty `Usage` events with `input_tokens` 100 then 250
- **THEN** the accumulated `RunLog.tokens.input_tokens` SHALL equal 250 (latest cumulative snapshot) AND SHALL NOT equal 350 (the sum)

#### Scenario: Cumulative backend ignores an empty usage snapshot after a non-empty snapshot

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Cumulative` AND the stream yields a non-empty `Usage` event with `input_tokens` 100 and `output_tokens` 40 followed by a `Usage` event whose normalized token counts are all zero
- **THEN** the accumulated `RunLog.tokens.input_tokens` SHALL equal 100 AND `RunLog.tokens.output_tokens` SHALL equal 40

#### Scenario: Cumulative backend with only empty usage snapshots remains zero

- **WHEN** `invoke` runs against a backend whose `token_usage_semantics()` returns `Cumulative` AND every emitted `Usage` event has all normalized token counts equal to zero
- **THEN** the accumulated `RunLog.tokens` SHALL remain the default zero `TokenUsage`

#### Scenario: Codex backend declares cumulative, Claude backend declares delta

- **WHEN** `token_usage_semantics()` is queried on the codex backend and on the claude backend
- **THEN** the codex backend SHALL return `Cumulative` AND the claude backend SHALL return `Delta`

#### Scenario: Semantics dispatch references no provider identity

- **WHEN** the `invoke` loop combines `Usage` events using the declared semantics
- **THEN** the dispatch SHALL branch only on the `TokenUsageSemantics` enum value AND SHALL NOT reference the `claude` or `codex` binary name, provider-specific argv flags, or provider-specific stream-json field names
