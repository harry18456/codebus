## ADDED Requirements

### Requirement: TrackedProvider tags usage records with default_module

The `TrackedProvider` SHALL accept an optional `default_module: str | None = None` argument at construction. When set, every `UsageTracker.record(...)` call made by `TrackedProvider.chat` and `TrackedProvider.embed` SHALL include `module=self._default_module`. The parameter is the SOLE mechanism by which subsystem labels (e.g., `"kb_build"`, `"qa_agent"`) reach `token_usage.jsonl` — callers MUST NOT bypass `TrackedProvider` to make their own `tracker.record()` call, ensuring "exactly one line per LLM call" per the `usage-tracking` capability.

#### Scenario: Default module reaches usage record

- **WHEN** a `TrackedProvider` is constructed with `default_module="kb_build"` and a `chat` or `embed` call succeeds
- **THEN** the corresponding `token_usage.jsonl` line MUST contain `"module": "kb_build"`

#### Scenario: Omitting default_module preserves M1 behavior

- **WHEN** a `TrackedProvider` is constructed without `default_module` (the M1 construction signature)
- **THEN** the wrapper MUST NOT raise, and the `token_usage.jsonl` line's `module` field MUST be the empty string `""` (matching M1's behavior before this Requirement landed)

#### Scenario: Failure path still records with default_module

- **WHEN** a `TrackedProvider` constructed with `default_module="kb_build"` wraps a provider whose `embed()` raises
- **THEN** the `llm_calls.jsonl` failure line MUST still be written (per existing `LLMCallLogger writes llm_calls.jsonl` Requirement), and any usage line written for the failed call MUST also carry `module="kb_build"` so accounting reflects retry costs against the right subsystem
