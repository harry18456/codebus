## ADDED Requirements

### Requirement: AuditRole enumerates legal role values in llm_calls.jsonl

The `role` field written to `<workspace>/.codebus/llm_calls.jsonl` by `LLMCallLogger` SHALL take one of exactly five string values: `"reasoning"`, `"judge"`, `"chat"`, `"embed"`, or `"pii_detection"`. Per `docs/decisions.md` D-033, the first four values are emitted by `TrackedProvider` in LLM mode (mapped from `ProviderRole.value`); the fifth value `"pii_detection"` is reserved for emission by future LLM-based PII providers operating through `TrackedProvider` in PII mode.

This change introduces the `"pii_detection"` value as a legal schema value but does NOT ship any code path that writes it — the production PIIProvider implementations in this change (`RuleBasedPIIProvider`, `MockPIIProvider`) perform no LLM calls and produce no `llm_calls.jsonl` lines. The schema reservation exists so a subsequent Spectra change introducing `LocalLLMPIIProvider` or `OpenAIPIIDetectionProvider` can emit `role: "pii_detection"` without re-opening the AuditRole closed set.

When `role == "pii_detection"`, the `sanitizer_pass2_applied` field on the same line MUST be `false`. When `role` is any of the four LLM-mode values (`"reasoning"`, `"judge"`, `"chat"`, `"embed"`), `sanitizer_pass2_applied` MUST be `true` (per the existing `TrackedProvider applies Sanitizer Pass 2 before dispatch` Requirement in the `llm-provider` capability). This pairing makes `role` and `sanitizer_pass2_applied` jointly carry the trust-boundary distinction surfaced by the Trust Layer R-01 / O-04 panels.

Adding a sixth legal `role` value MUST be done by a Spectra change that simultaneously updates this Requirement, the consuming code in `LLMCallLogger`, and any UI panel that filters on `role`. This constraint generalizes the closed-set governance pattern already applied to the `module` lane labels in the `UsageTracker writes token_usage.jsonl` Requirement.

#### Scenario: Closed set of role values

- **WHEN** any test scans every line of `<workspace>/.codebus/llm_calls.jsonl` produced by sidecar production code across all task kinds (`scan` / `kb` / `explore` / `generate` / `qa`)
- **THEN** every line's `role` field value MUST be one of `{"reasoning", "judge", "chat", "embed", "pii_detection"}`
- **AND** no other string value MUST appear in the `role` field
- **AND** the test MUST also fail if a line lacks the `role` field entirely (the field is mandatory)

#### Scenario: pii_detection role pairs with sanitizer_pass2_applied false

- **WHEN** any production-emitted `llm_calls.jsonl` line has `"role": "pii_detection"`
- **THEN** the same line MUST have `"sanitizer_pass2_applied": false`
- **AND** an integration test MUST assert this pairing across every line that consumes the future PII LLM provider code path (skipped for this change since no such provider ships)

#### Scenario: LLM-mode roles pair with sanitizer_pass2_applied true

- **WHEN** any production-emitted `llm_calls.jsonl` line has `"role"` value in `{"reasoning", "judge", "chat", "embed"}`
- **THEN** the same line MUST have `"sanitizer_pass2_applied": true`
- **AND** any deviation MUST cause a test failure identifying the D-015 invariant violation for this line

#### Scenario: This change emits no pii_detection lines

- **WHEN** all tests for this change pass and `RuleBasedPIIProvider` / `MockPIIProvider` are exercised by their respective unit tests
- **THEN** zero lines with `"role": "pii_detection"` MUST be appended to any `llm_calls.jsonl` file during these tests
- **AND** the AuditRole closed set MUST remain enforced as the schema contract for any future PII Spectra change to honour
