## MODIFIED Requirements

### Requirement: UsageTracker writes token_usage.jsonl

The sidecar SHALL implement a `UsageTracker` that appends one JSON line per LLM call to `<workspace>/.codebus/token_usage.jsonl`, per `docs/decisions.md` D-021 and `docs/agent-core.md §十三`. The path lives under the `.codebus/` subdirectory of the workspace root (consistent with the workspace-level audit chain convention shared by `<workspace>/.codebus/sanitize_audit.jsonl` and `<workspace>/.codebus/tool_audit.jsonl`); the tracker's constructor MUST auto-create the parent `.codebus/` directory if absent so callers do not have to pre-mkdir. The `module` field on each line SHALL reflect the calling subsystem (e.g., `"kb_build"`, `"qa_agent"`); when `TrackedProvider` is constructed with `default_module`, that value SHALL be carried automatically into every record it writes, so callers do not duplicate the `record(...)` call themselves.

The `cost_usd` field on each line SHALL be derived for `chat` operations from a model→pricing table lookup (`codebus_agent.providers.pricing.estimate_chat_cost_usd(model, prompt_tokens, completion_tokens)`) rather than a hard-coded `0.0`. When the model identifier is present in the pricing table, the recorded cost MUST be the sum of `prompt_tokens × input_per_1m_usd / 1_000_000` and `completion_tokens × output_per_1m_usd / 1_000_000`. When the model identifier is absent from the pricing table, the recorded cost MUST default to `0.0` and a warning-level log entry naming the unknown model MUST be emitted via the standard logger so operators can extend the table. `embed` operations preserve their existing cost path (the inner provider's `Usage.cost_usd` falling back to `0.0`); the pricing table is `chat`-specific in this change because OpenAI Python SDK does not return `cost_usd` for chat completions today.

This same `cost_usd` value MUST be used both in the `token_usage.jsonl` line and in the `usage_delta` SSE event emitted by `TrackedProvider`, so on-disk audit and on-wire telemetry agree.

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

#### Scenario: Known chat model writes non-zero cost_usd

- **WHEN** a `TrackedProvider` whose inner provider reports a model identifier listed in `codebus_agent.providers.pricing` (e.g., `gpt-4o-mini-chat-v1` mapping to `gpt-4o-mini`) completes a `chat` call with non-zero `prompt_tokens` and `completion_tokens`
- **THEN** the appended `<workspace>/.codebus/token_usage.jsonl` line MUST contain a `cost_usd` value strictly greater than `0.0`
- **AND** the value MUST equal `prompt_tokens × input_per_1m_usd / 1_000_000 + completion_tokens × output_per_1m_usd / 1_000_000` rounded to a finite floating-point number
- **AND** the `usage_delta` SSE event emitted for the same call MUST carry the same `cost_usd` value (audit and wire agree)

#### Scenario: Unknown chat model logs warning and writes zero cost_usd

- **WHEN** a `TrackedProvider` whose inner provider reports a model identifier NOT listed in `codebus_agent.providers.pricing` completes a `chat` call
- **THEN** the appended `<workspace>/.codebus/token_usage.jsonl` line MUST contain `"cost_usd": 0.0`
- **AND** a warning-level log entry naming the unknown model identifier MUST be emitted via Python's standard logging so operators can extend the pricing table without losing audit fidelity
