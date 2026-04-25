## MODIFIED Requirements

### Requirement: LLMCallLogger writes llm_calls.jsonl

The sidecar SHALL implement an `LLMCallLogger` that appends the full request and response payload (as seen by the provider) for every call, per `docs/decisions.md` D-022.

#### Scenario: Request and response captured

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `llm_calls.jsonl` containing `request` (the exact payload sent to the provider) and `response` (the exact payload received)

#### Scenario: Sanitizer-ready field reserved

- **WHEN** a line from `llm_calls.jsonl` is parsed
- **THEN** it MUST contain a `sanitizer_pass2_applied` boolean field whose value reflects whether Sanitizer Pass 2 was applied to the request before dispatch (production code post-`sanitizer-safety-chain` always sets `true`; the field exists so future changes that gate Pass 2 on per-call conditions can vary it)

#### Scenario: Failure still logged

- **WHEN** an `LLMProvider.chat` call raises an exception through the tracked wrapper
- **THEN** a line with `response: null` and an `error` field describing the exception class and message MUST be appended to `llm_calls.jsonl`
