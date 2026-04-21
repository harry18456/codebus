## ADDED Requirements

### Requirement: TrackedProvider applies Sanitizer Pass 2 before dispatch

The `TrackedProvider` SHALL invoke `SanitizerEngine.sanitize` on every outbound `chat` and `embed` payload before delegating to the wrapped provider. The sanitized payload SHALL be what the wrapped provider receives and SHALL be what `LLMCallLogger` records to `llm_calls.jsonl`, per `docs/decisions.md` D-015 and D-022.

#### Scenario: Chat payload sanitized before wrapped provider sees it

- **WHEN** `TrackedProvider(MockProvider(), role=ProviderRole.CHAT)` is constructed with a sanitizer injected
- **AND** `chat(messages=[{"role": "user", "content": "alice@example.com"}], response_model=...)` is called
- **THEN** the wrapped `MockProvider.chat` MUST receive a messages list whose user message `content` equals `"<REDACTED:email#1>"`
- **AND** the `llm_calls.jsonl` line for this call MUST record the same sanitized content, not the original email

#### Scenario: Embed texts sanitized before wrapped provider sees them

- **WHEN** `TrackedProvider.embed(texts=["contact 0912-345-678"])` is called
- **THEN** the wrapped provider's `embed` MUST be invoked with texts where `0912-345-678` has been replaced by `<REDACTED:phone#<N>>`

#### Scenario: sanitizer_pass2_applied field set to true

- **WHEN** any `TrackedProvider` call completes (successful or raising from the wrapped provider)
- **THEN** the corresponding `llm_calls.jsonl` line MUST contain `"sanitizer_pass2_applied": true`
- **AND** the field type MUST remain boolean (no breaking change from M1 schema)

#### Scenario: Sanitizer failure aborts dispatch

- **WHEN** the injected `SanitizerEngine.sanitize` raises `SanitizerError` during `chat`
- **THEN** the wrapped provider's `chat` MUST NOT be invoked
- **AND** no entry MUST be written to `llm_calls.jsonl` for this call
- **AND** the `TrackedProvider.chat` call MUST propagate the `SanitizerError` to its caller

### Requirement: TrackedProvider writes audit entries to sanitize_audit.jsonl

For every Pass 2 sanitize invocation performed by `TrackedProvider`, each resulting `AuditEntry` SHALL be appended to `{workspace}/.codebus/sanitize_audit.jsonl` by the injected `SanitizerAuditLogger`, with `pass` field equal to `2`.

#### Scenario: Pass 2 audit entry written

- **WHEN** `TrackedProvider.chat` sanitizes a message that contains an email
- **THEN** `sanitize_audit.jsonl` MUST have exactly one appended line with `"pass": 2`
- **AND** that line MUST include the same fields required by the `sanitizer` capability spec (`ts`, `schema_version`, `rules_version`, `session_id`, `source`, `rule_id`, `kind`, `placeholder_index`, `extra`)

#### Scenario: Source field identifies message scope

- **WHEN** a Pass 2 audit entry is written
- **THEN** its `source` field MUST start with the prefix `message:` followed by a stable identifier for the in-flight call (for example `message:chat_req_<uuid>`)
