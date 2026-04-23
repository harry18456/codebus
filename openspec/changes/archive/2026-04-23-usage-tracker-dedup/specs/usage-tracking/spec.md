## MODIFIED Requirements

### Requirement: UsageTracker writes token_usage.jsonl

The sidecar SHALL implement a `UsageTracker` that appends one JSON line per LLM call to `<workspace>/token_usage.jsonl`, per `docs/decisions.md` D-021 and `docs/agent-core.md §十三`. The `module` field on each line SHALL reflect the calling subsystem (e.g., `"kb_build"`, `"qa_agent"`); when `TrackedProvider` is constructed with `default_module`, that value SHALL be carried automatically into every record it writes, so callers do not duplicate the `record(...)` call themselves.

#### Scenario: One line per chat call

- **WHEN** an `LLMProvider.chat` call completes through the tracked wrapper
- **THEN** exactly one new line MUST be appended to `token_usage.jsonl`

#### Scenario: Required fields present

- **WHEN** a line from `token_usage.jsonl` is parsed
- **THEN** it MUST contain the keys `timestamp`, `provider`, `model`, `operation`, `input_tokens`, `output_tokens`, and `cost_usd` with non-null values

#### Scenario: Embed calls tracked

- **WHEN** an `LLMProvider.embed` call completes through the tracked wrapper
- **THEN** a line with `operation="embed"` and `output_tokens=0` MUST be appended to `token_usage.jsonl`

#### Scenario: Module field reflects TrackedProvider's default_module

- **WHEN** a `TrackedProvider` is constructed with `default_module="kb_build"` and an `embed` call completes through it
- **THEN** the appended `token_usage.jsonl` line MUST contain `"module": "kb_build"`, and no second line with the same `(timestamp, model, input_tokens)` tuple MUST be appended by any other layer (e.g., `KnowledgeBase` MUST NOT call `tracker.record()` itself)

#### Scenario: Default module absent yields empty string

- **WHEN** a `TrackedProvider` is constructed without `default_module` (or `default_module=None`) and a call completes through it
- **THEN** the appended line's `module` field MUST be the empty string `""`, preserving backward compatibility with M1 records that did not carry a module label
