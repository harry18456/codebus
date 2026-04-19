# usage-tracking Specification

## Purpose

TBD - created by archiving change 'm1-power-on'. Update Purpose after archive.

## Requirements

### Requirement: UsageTracker writes token_usage.jsonl

The sidecar SHALL implement a `UsageTracker` that appends one JSON line per LLM call to `<workspace>/token_usage.jsonl`, per `docs/decisions.md` D-021 and `docs/agent-core.md §十三`.

#### Scenario: One line per chat call

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `token_usage.jsonl`

#### Scenario: Required fields present

- **WHEN** a line from `token_usage.jsonl` is parsed
- **THEN** it MUST contain the keys `timestamp`, `provider`, `model`, `operation`, `input_tokens`, `output_tokens`, and `cost_usd` with non-null values

#### Scenario: Embed calls tracked

- **WHEN** an `LLMProvider.embed` call completes through the tracked wrapper
- **THEN** a line with `operation="embed"` and `output_tokens=0` MUST be appended to `token_usage.jsonl`


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

---
### Requirement: LLMCallLogger writes llm_calls.jsonl

The sidecar SHALL implement an `LLMCallLogger` that appends the full request and response payload (as seen by the provider) for every call, per `docs/decisions.md` D-022.

#### Scenario: Request and response captured

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `llm_calls.jsonl` containing `request` (the exact payload sent to the provider) and `response` (the exact payload received)

#### Scenario: Sanitizer-ready field reserved

- **WHEN** a line from `llm_calls.jsonl` is parsed
- **THEN** it MUST contain a `sanitizer_pass2_applied` boolean field. During M1 this field MUST be `false` (Sanitizer Pass 2 is not yet implemented); later changes will flip it to `true`

#### Scenario: Failure still logged

- **WHEN** an `LLMProvider.chat` call raises an exception through the tracked wrapper
- **THEN** a line with `response: null` and an `error` field describing the exception class and message MUST be appended to `llm_calls.jsonl`


<!-- @trace
source: m1-power-on
updated: 2026-04-19
code:
  - web/dist
-->

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
code:
  - web/dist
-->
