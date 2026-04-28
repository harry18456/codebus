## ADDED Requirements

### Requirement: PIIProvider Protocol exposes detect-shaped interface

The sidecar SHALL define a `PIIProvider` Protocol that exposes exactly one method, `async def detect(text: str) -> list[PIISpan]`, returning a list of zero or more PII spans found in the input text. Per `docs/decisions.md` D-033, this Protocol SHALL be the single abstraction every PII detection backend (rule-based, LLM-based, hybrid) implements; no method on this Protocol returns a sanitized string or applies placeholders. Placeholder rendering and audit emission remain the responsibility of `SanitizerEngine`.

`PIISpan` SHALL be a frozen dataclass containing exactly five fields: `rule_id: str`, `kind: str`, `start: int`, `end: int`, `value: str`. The field semantics SHALL match the legacy `RuleMatch` dataclass currently declared in `sidecar/src/codebus_agent/sanitizer/rules.py`, allowing existing engine logic to migrate without span-shape changes.

The Protocol SHALL be `@runtime_checkable` so registry / wrapper code can use `isinstance(x, PIIProvider)` for marker-based dispatch.

#### Scenario: Protocol exposes only detect

- **WHEN** the `PIIProvider` Protocol is imported from `codebus_agent.providers`
- **THEN** it MUST declare exactly one method `detect(text: str) -> list[PIISpan]`
- **AND** the method MUST be declared `async`
- **AND** the Protocol MUST NOT declare any method named `chat`, `embed`, or `sanitize`

#### Scenario: PIISpan dataclass shape

- **WHEN** a `PIISpan` instance is constructed
- **THEN** it MUST expose `rule_id: str`, `kind: str`, `start: int`, `end: int`, `value: str` attributes
- **AND** the dataclass MUST be frozen (mutation MUST raise `dataclasses.FrozenInstanceError`)

#### Scenario: Protocol is runtime checkable

- **WHEN** a class implementing `async def detect(self, text: str) -> list[PIISpan]` is checked with `isinstance(instance, PIIProvider)`
- **THEN** the check MUST return `True` regardless of class hierarchy (structural subtyping)

---

### Requirement: RuleBasedPIIProvider wraps existing default_rules

The sidecar SHALL ship a `RuleBasedPIIProvider` class that implements the `PIIProvider` Protocol by delegating to the existing built-in rule table. The class SHALL accept `rules: list[Rule] | None = None` and `config: SanitizerConfig | None = None` at construction; when `rules` is omitted, it SHALL use `codebus_agent.sanitizer.rules.default_rules()`. The class SHALL NOT modify any rule pattern, kind label, or `rule_id` value defined in the existing rule table — this Requirement formalizes that the structural change is wrapping-only and rules content remains governed by the `sanitizer` capability's `Built-in rule set` Requirement.

#### Scenario: Default construction uses default_rules

- **WHEN** `RuleBasedPIIProvider()` is constructed without arguments
- **THEN** `await provider.detect("contact: alice@example.com")` MUST return exactly one `PIISpan` with `kind="email"` and `value="alice@example.com"`
- **AND** the span's `rule_id` MUST equal the same identifier produced by the matching rule in `codebus_agent.sanitizer.rules.default_rules()` (no rename, no synthetic id)

#### Scenario: Multiple matches returned in order

- **WHEN** `await provider.detect("a@b.com and c@d.com")` is called
- **THEN** the returned list MUST contain exactly two spans
- **AND** the spans MUST be ordered by `start` ascending

#### Scenario: Empty input returns empty list

- **WHEN** `await provider.detect("")` is called
- **THEN** the returned list MUST be empty (length 0)

#### Scenario: detect is async-callable from sync regex

- **WHEN** `RuleBasedPIIProvider.detect` runs against text containing PII patterns
- **THEN** it MUST yield results without performing any blocking I/O (the pure-regex implementation contains no `await` suspension point)
- **AND** awaiting the coroutine MUST resolve in a single event loop tick

---

### Requirement: MockPIIProvider supports test scripting

The sidecar SHALL ship a `MockPIIProvider` class for tests that implements `PIIProvider`. The class SHALL accept an optional `script: list[list[PIISpan]] | None = None` at construction. Each `detect` call SHALL consume one entry from `script` and return it; if `script` is None or exhausted, `detect` SHALL return an empty list.

The mock SHALL track its calls for assertion (recording each input text). The mock MUST NOT inspect the input text against any rule pattern — its return value is determined solely by the script.

#### Scenario: Script controls return value

- **WHEN** `MockPIIProvider(script=[[PIISpan(rule_id="x", kind="email", start=0, end=4, value="abcd")]])` is constructed
- **AND** `await mock.detect("anything")` is called
- **THEN** the returned list MUST contain exactly one span equal to the scripted `PIISpan`
- **AND** the input text "anything" MUST be ignored (no pattern matching)

#### Scenario: No script returns empty

- **WHEN** `MockPIIProvider()` is constructed without `script`
- **AND** `await mock.detect("contact: alice@example.com")` is called
- **THEN** the returned list MUST be empty

#### Scenario: Script exhaustion returns empty

- **WHEN** `MockPIIProvider(script=[[]])` is constructed (one scripted call returning empty)
- **AND** `await mock.detect("first")` is called once
- **AND** `await mock.detect("second")` is called a second time
- **THEN** both calls MUST return empty lists

#### Scenario: Mock records call inputs

- **WHEN** `mock = MockPIIProvider()` is constructed
- **AND** `await mock.detect("foo")` and `await mock.detect("bar")` are called in order
- **THEN** `mock.calls` MUST equal `["foo", "bar"]` (or equivalent ordered accessor exposing both inputs)

---

### Requirement: TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES

The `TrackedProvider` class SHALL expose a class-level constant `PII_ALLOWED_INNER_TYPES: ClassVar[frozenset[type]]` enumerating the concrete classes that are legal as PII inner providers. This constant SHALL be disjoint from `ALLOWED_INNER_TYPES` (which gates LLM / Embedding inners). Adding a new PII provider implementation MUST be done through a Spectra change that simultaneously updates this Requirement and `PII_ALLOWED_INNER_TYPES` — drift between the spec and code is the failure mode this constraint prevents (mirrors the existing `Outbound LLM traffic gated by TrackedProvider whitelist` Requirement in the `llm-provider` capability).

After this change lands, `PII_ALLOWED_INNER_TYPES` MUST equal exactly `{RuleBasedPIIProvider, MockPIIProvider}`. Future additions (e.g., `LocalLLMPIIProvider`, `OpenAIPIIDetectionProvider`) MUST extend this set in their own respective Spectra changes.

#### Scenario: PII_ALLOWED_INNER_TYPES exposes initial allowlist

- **WHEN** `TrackedProvider.PII_ALLOWED_INNER_TYPES` is inspected after this change lands
- **THEN** it MUST equal exactly `{RuleBasedPIIProvider, MockPIIProvider}` (set equality, no superset)

#### Scenario: PII and LLM allowlists are disjoint

- **WHEN** the intersection `TrackedProvider.ALLOWED_INNER_TYPES & TrackedProvider.PII_ALLOWED_INNER_TYPES` is computed
- **THEN** the intersection MUST be empty (no class can be registered in both lanes)

#### Scenario: Non-allowlisted PII inner rejected at construction

- **WHEN** code attempts `TrackedProvider(SomeUnregisteredPIIProvider(), ...)` where `SomeUnregisteredPIIProvider` implements `PIIProvider` but is NOT in `PII_ALLOWED_INNER_TYPES`
- **THEN** construction MUST raise `TypeError` naming the disallowed inner class
- **AND** the error message MUST instruct the developer to extend `PII_ALLOWED_INNER_TYPES` via a Spectra change

#### Scenario: Source-grep test pins allowlist to spec

- **WHEN** an integration test scans `sidecar/src/codebus_agent/providers/tracked.py` for the `PII_ALLOWED_INNER_TYPES` literal contents
- **THEN** the discovered class set MUST equal the set named in this Requirement (`{RuleBasedPIIProvider, MockPIIProvider}` post-this-change)
- **AND** any additional or missing entry MUST cause the test to fail with a message identifying the drift

---

### Requirement: TrackedProvider auto-bypasses Pass 2 for PII inner

When `TrackedProvider` is constructed with an inner instance whose concrete type is in `PII_ALLOWED_INNER_TYPES`, the wrapper SHALL operate in "PII mode": it SHALL NOT invoke `SanitizerEngine.sanitize` on any input passed to the inner's `detect` method, regardless of whether a `sanitizer` argument was supplied at construction time. Per `docs/decisions.md` D-033, this is the sole legitimate exception to the D-015 invariant "every Provider input passes through Sanitizer Pass 2"; the exception is gated by `PII_ALLOWED_INNER_TYPES` membership and CANNOT be triggered by any external flag, parameter, or runtime branch.

In PII mode, calls to `chat()` or `embed()` on the wrapper MUST raise `RuntimeError` (the wrapper exposes only the inner's `detect()` semantics in this mode). In LLM mode (inner type in `ALLOWED_INNER_TYPES`), calls to `detect()` MUST raise `RuntimeError`. The mode SHALL be determined exactly once at `__init__` time and SHALL NOT change during the wrapper's lifetime.

#### Scenario: PII mode bypasses Pass 2

- **WHEN** `TrackedProvider(RuleBasedPIIProvider(), sanitizer=engine, ...)` is constructed
- **AND** `await wrapper.detect("contact: alice@example.com")` is called
- **THEN** the wrapped `RuleBasedPIIProvider.detect` MUST receive the original text `"contact: alice@example.com"` (NOT a redacted version)
- **AND** `sanitizer.sanitize` MUST NOT be invoked during this call
- **AND** no entry MUST be appended to `sanitize_audit.jsonl` for this call (Pass 2 audit lane is the LLM mode lane only)

#### Scenario: No external flag can trigger bypass in LLM mode

- **WHEN** `TrackedProvider(OpenAIChatProvider("gpt-4o-mini"), sanitizer=engine, ...)` is constructed (LLM mode)
- **THEN** the constructor MUST NOT accept any keyword argument named `skip_sanitizer`, `bypass_pass2`, or any other flag with equivalent semantics
- **AND** every `chat` call MUST invoke `sanitizer.sanitize` regardless of construction parameters (the existing `TrackedProvider applies Sanitizer Pass 2 before dispatch` Requirement in `llm-provider` capability remains in force)

#### Scenario: Mode is determined once at construction

- **WHEN** a `TrackedProvider` instance is constructed
- **THEN** an internal mode marker (`"llm"` or `"pii"`) MUST be set exactly once based on `type(inner)` membership in `ALLOWED_INNER_TYPES` vs `PII_ALLOWED_INNER_TYPES`
- **AND** this mode marker MUST NOT be reassigned at any later point in the wrapper's lifetime

#### Scenario: Wrong-mode method calls raise

- **WHEN** `wrapper.chat(...)` or `wrapper.embed(...)` is called on a wrapper in PII mode
- **THEN** the call MUST raise `RuntimeError` whose message identifies the wrapper's mode and the called method as incompatible
- **AND** symmetrically, `wrapper.detect(...)` on a wrapper in LLM mode MUST raise `RuntimeError`

---

### Requirement: Future LLM-based PII providers extend allowlist additively

Future PII detection backends that issue outbound LLM calls (e.g., `LocalLLMPIIProvider` connecting to a local Ollama / LM Studio model, or `OpenAIPIIDetectionProvider` connecting to a hosted PII detection endpoint) SHALL extend `PII_ALLOWED_INNER_TYPES` via a separate Spectra change, NOT via this change. This Requirement formalizes the open extension contract:

- A future `LocalLLMPIIProvider` MUST validate its `base_url` constructor argument at construction time, raising if `base_url` does not target localhost / loopback / RFC1918 / RFC4193 address space — the validation criteria SHALL be specified in that change's spec, not this one.
- Audit emission for outbound LLM calls performed by such providers MUST use `role: "pii_detection"` and `sanitizer_pass2_applied: false` in `llm_calls.jsonl` lines, per the `usage-tracking` capability's `AuditRole enumerates legal role values` Requirement (added by this change).
- The change introducing each new PII provider MUST update `PII_ALLOWED_INNER_TYPES` in `tracked.py` AND the `TrackedProvider gates PII inner classes via PII_ALLOWED_INNER_TYPES` Requirement above (which currently pins `{RuleBasedPIIProvider, MockPIIProvider}`).

This Requirement contains no scenarios because it governs future-change behavior, not this change's runtime behavior. The constraint is preserved as a Requirement (rather than a docs/decisions.md ADR alone) so the analyzer surfaces drift if a future change adds an LLM-based PII provider without simultaneously touching `PII_ALLOWED_INNER_TYPES` and the audit role spec.

#### Scenario: Spec contains explicit forward-looking constraint

- **WHEN** the `pii-provider` capability spec is read after this change archives
- **THEN** it MUST contain this `Future LLM-based PII providers extend allowlist additively` Requirement verbatim
- **AND** the Requirement text MUST name `LocalLLMPIIProvider` and `OpenAIPIIDetectionProvider` as the two anticipated future implementations
- **AND** the Requirement MUST reference `role: "pii_detection"` and `sanitizer_pass2_applied: false` as the audit field values future implementations MUST emit
