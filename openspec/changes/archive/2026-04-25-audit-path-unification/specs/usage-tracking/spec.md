## MODIFIED Requirements

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
