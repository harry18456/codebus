## MODIFIED Requirements

### Requirement: SanitizerEngine exposes pure `sanitize` interface

The sidecar SHALL define a `SanitizerEngine` class with an async method `async def sanitize(text: str, source: SanitizeSource) -> SanitizedResult`, per `docs/decisions.md` D-011, D-015, and D-033. `SanitizedResult` MUST contain the sanitized text and a list of `AuditEntry` records describing each replacement. `SanitizeSource` MUST be a tagged union discriminating Pass 1 (file source) and Pass 2 (message source); the same class MUST be reusable by Pass 3 (Q&A `add_to_kb`) without signature change.

The Engine SHALL accept a `pii_provider: PIIProvider` argument at construction (replacing the previous `rules: list[Rule] | None = None` argument). The Engine SHALL delegate span discovery to `await pii_provider.detect(text)` and retain ownership of:

- Placeholder rendering per the `Placeholder format is <REDACTED:kind#index>` Requirement
- Per-call placeholder index management per the `Placeholder index scope is single sanitize call` Requirement
- Allowlist application per the `Allowlist hits still audited but not redacted` Requirement
- Audit entry construction (Engine returns `AuditEntry` records; the audit logger writes them to `sanitize_audit.jsonl`)

Switching from rule-based to LLM-based PII detection SHALL be performed by injecting a different `PIIProvider` implementation; the Engine itself MUST contain no rule logic and MUST NOT import `Rule` / `RegexRule` / `DetectSecretsRule` directly.

The migration from sync to async sanitize is BREAKING for any sync caller; the three production Pass call sites (Pass 1 scanner, Pass 2 TrackedProvider pre-flight, Pass 3 Q&A `add_to_kb`) all reside in async contexts and MUST be migrated to `await sanitize(...)` in this change.

#### Scenario: Pass 1 sanitize returns replaced text and audit entries

- **WHEN** `await SanitizerEngine(...).sanitize("contact: alice@example.com", source=FileSource(path="src/app.py"))` is awaited
- **THEN** the returned `SanitizedResult.text` MUST contain `<REDACTED:email#1>` in place of `alice@example.com`
- **AND** `SanitizedResult.entries` MUST contain exactly one `AuditEntry` with `kind="email"`, `placeholder_index=1`, `rule_id` naming the rule that matched, and `source` identifying the file

#### Scenario: Same value replaced with same placeholder within single call

- **WHEN** `await SanitizerEngine(...).sanitize("a: alice@example.com, b: alice@example.com", source=FileSource(path="src/a.py"))` is awaited
- **THEN** both occurrences of `alice@example.com` MUST be replaced with the same placeholder string `<REDACTED:email#1>`
- **AND** `SanitizedResult.entries` MUST contain exactly one entry for that `(kind, original_value)` pair, not two

#### Scenario: Placeholder index resets across sanitize calls

- **WHEN** `sanitize` is awaited twice with different `FileSource` paths, both containing distinct emails
- **THEN** each call's returned `placeholder_index` MUST start at `1`, independent of any previous call

#### Scenario: Fail-closed on engine error

- **WHEN** the injected `pii_provider.detect()` raises an unrecoverable internal error (for example, a `detect-secrets` plugin raising)
- **THEN** `sanitize` MUST raise `SanitizerError` and MUST NOT return any partial text
- **AND** the raised error MUST include the source identifier and the originating exception chained as `__cause__`

#### Scenario: Engine constructor accepts PIIProvider, not rules

- **WHEN** `SanitizerEngine(rules=[...])` is attempted (passing the legacy `rules` keyword)
- **THEN** construction MUST raise `TypeError` indicating the `rules` argument has been removed
- **AND** the error message MUST instruct the caller to inject a `PIIProvider` (e.g., `RuleBasedPIIProvider()` for the default behavior)
- **AND** `SanitizerEngine(pii_provider=RuleBasedPIIProvider())` MUST succeed and produce the same redaction behavior as the pre-D-033 default

#### Scenario: Engine has no direct rule imports

- **WHEN** static analysis inspects `sidecar/src/codebus_agent/sanitizer/engine.py`
- **THEN** the module MUST NOT import `Rule`, `RegexRule`, `DetectSecretsRule`, `default_rules`, or any other rule-table symbol from `codebus_agent.sanitizer.rules`
- **AND** all rule-related symbols MUST be reachable only via the injected `PIIProvider` instance

---

### Requirement: Built-in rule set covers Secret, PII, internal-identifier kinds

The default `RuleBasedPIIProvider` (instantiated by `make_default_engine()` and used wherever no other `PIIProvider` is injected) SHALL ship with built-in rules covering all kinds listed in `docs/sanitizer.md §一`: Secret (via `detect-secrets` integration plus API key / JWT / PEM / SSH private key / DB connection string / `.env` KEY=value patterns), PII (email, Taiwan mobile, Taiwan national ID), internal identifiers (RFC1918 / RFC4193 / link-local IP, `.local` / `.internal` / `.corp` / `.lan` TLD).

This Requirement preserves the pre-D-033 behavior verbatim — the rule patterns, `kind` labels, and `rule_id` strings produced by `default_rules()` MUST remain unchanged from the sanitizer-safety-chain implementation. Only the class that holds the rules has moved from `SanitizerEngine` to `RuleBasedPIIProvider` (as specified by the `pii-provider` capability's `RuleBasedPIIProvider wraps existing default_rules` Requirement). The `default_rules()` function in `codebus_agent.sanitizer.rules` SHALL remain its single source of truth; `RuleBasedPIIProvider.__init__` calls it when no rules are explicitly injected.

#### Scenario: Taiwan mobile number detected

- **WHEN** `sanitize` is called with text containing `0912-345-678`
- **THEN** the text MUST be replaced with `<REDACTED:phone#<N>>` and the audit entry's `kind` MUST equal `phone`

#### Scenario: Taiwan national ID detected

- **WHEN** `sanitize` is called with text containing `A123456789`
- **THEN** the text MUST be replaced with `<REDACTED:id#<N>>` and the audit entry's `kind` MUST equal `id`

#### Scenario: RFC1918 IP detected

- **WHEN** `sanitize` is called with text containing `10.0.3.42`
- **THEN** the text MUST be replaced with `<REDACTED:ip#<N>>` and the audit entry's `kind` MUST equal `ip`

#### Scenario: Internal TLD detected

- **WHEN** `sanitize` is called with text containing `db01.corp`
- **THEN** the text MUST be replaced with `<REDACTED:internal-domain#<N>>` and the audit entry's `kind` MUST equal `internal-domain`

#### Scenario: detect-secrets flags high-entropy API key

- **WHEN** `sanitize` is called with text containing a string that `detect-secrets` classifies as a secret (for example an AWS access key pattern)
- **THEN** the string MUST be replaced with `<REDACTED:secret#<N>>` or a more specific kind (`jwt`, `private-key`, `credential`) when the matching rule identifies the subtype

#### Scenario: rule_id stability across structural change

- **WHEN** any audit entry produced by `await SanitizerEngine(pii_provider=RuleBasedPIIProvider()).sanitize(...)` is inspected
- **THEN** the `rule_id` field MUST equal the same string the pre-D-033 `SanitizerEngine(rules=default_rules())` would have produced for the same matching rule
- **AND** an integration test MUST cross-reference at least one `rule_id` against a fixture string captured before this change to verify zero drift
