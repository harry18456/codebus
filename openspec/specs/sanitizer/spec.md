# sanitizer Specification

## Purpose

TBD - created by archiving change 'sanitizer-safety-chain'. Update Purpose after archive.

## Requirements

### Requirement: SanitizerEngine exposes pure `sanitize` interface

The sidecar SHALL define a `SanitizerEngine` class with a method `sanitize(text: str, source: SanitizeSource) -> SanitizedResult`, per `docs/decisions.md` D-011 and D-015. `SanitizedResult` MUST contain the sanitized text and a list of `AuditEntry` records describing each replacement. `SanitizeSource` MUST be a tagged union discriminating Pass 1 (file source) and Pass 2 (message source); the same class MUST be reusable by Pass 3 (Q&A `add_to_kb`) without signature change.

#### Scenario: Pass 1 sanitize returns replaced text and audit entries

- **WHEN** `SanitizerEngine.sanitize("contact: alice@example.com", source=FileSource(path="src/app.py"))` is called
- **THEN** the returned `SanitizedResult.text` MUST contain `<REDACTED:email#1>` in place of `alice@example.com`
- **AND** `SanitizedResult.entries` MUST contain exactly one `AuditEntry` with `kind="email"`, `placeholder_index=1`, `rule_id` naming the rule that matched, and `source` identifying the file

#### Scenario: Same value replaced with same placeholder within single call

- **WHEN** `SanitizerEngine.sanitize("a: alice@example.com, b: alice@example.com", source=FileSource(path="src/a.py"))` is called
- **THEN** both occurrences of `alice@example.com` MUST be replaced with the same placeholder string `<REDACTED:email#1>`
- **AND** `SanitizedResult.entries` MUST contain exactly one entry for that `(kind, original_value)` pair, not two

#### Scenario: Placeholder index resets across sanitize calls

- **WHEN** `sanitize` is called twice with different `FileSource` paths, both containing distinct emails
- **THEN** each call's returned `placeholder_index` MUST start at `1`, independent of any previous call

#### Scenario: Fail-closed on engine error

- **WHEN** `sanitize` encounters an unrecoverable internal error (for example, a `detect-secrets` plugin raising)
- **THEN** it MUST raise `SanitizerError` and MUST NOT return any partial text
- **AND** the raised error MUST include the source identifier and the originating exception chained as `__cause__`

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/engine.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Placeholder format is `<REDACTED:kind#index>`

The placeholder string emitted by `SanitizerEngine` SHALL match the regular expression `<REDACTED:(email|phone|id|secret|ip|internal-domain|jwt|private-key|credential|suspect)#\d+>`, per `docs/sanitizer.md §四`.

#### Scenario: Email placeholder format

- **WHEN** an email is detected and replaced
- **THEN** the placeholder MUST take the form `<REDACTED:email#<N>>` where `N` is a positive integer

#### Scenario: No reverse mapping stored

- **WHEN** any replacement is performed
- **THEN** the engine MUST NOT retain the original value in memory beyond the duration of the single `sanitize` call
- **AND** the engine MUST NOT expose any method returning the pre-sanitize value for a given placeholder

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/engine.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Built-in rule set covers Secret, PII, internal-identifier kinds

The `SanitizerEngine` SHALL ship with built-in rules covering all kinds listed in `docs/sanitizer.md §一`: Secret (via `detect-secrets` integration plus API key / JWT / PEM / SSH private key / DB connection string / `.env` KEY=value patterns), PII (email, Taiwan mobile, Taiwan national ID), internal identifiers (RFC1918 / RFC4193 / link-local IP, `.local` / `.internal` / `.corp` / `.lan` TLD).

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

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/rules.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: SanitizerAuditLogger appends each replacement to JSONL

The sidecar SHALL expose a `SanitizerAuditLogger` that writes one JSON object per replacement to `{workspace}/.codebus/sanitize_audit.jsonl`. The file SHALL be append-only and each line SHALL be a single JSON object terminated by `\n`.

The `sanitize_audit.jsonl` filename literal MUST appear in `sidecar/src/codebus_agent/_audit_paths.py` as the canonical `_SANITIZE_AUDIT_FILENAME` constant. All other modules in `sidecar/src/codebus_agent/` that reference the filename MUST import the constant from `_audit_paths` (or its backward-compat shim `codebus_agent.api._audit_paths`); they MUST NOT redeclare the literal string. This rule generalises the equivalent constraint that the `kb-growth` capability already imposes on `kb_growth.jsonl`, extending the single-source contract to all seven workspace-level audit JSONL filenames.

Each audit line's `pass` integer MUST agree with the line's `source` shape per the cross-cutting `pass_num to source-type invariant`: `pass=1` (Pass 1, file-reading-stage Sanitize) MUST carry a file-source (`source` JSON-serialized shape reflects `FileSource(path=..., pass_=...)`); `pass=2` (Pass 2, Provider pre-flight Sanitize) MUST carry a message-source (`source` JSON-serialized shape reflects `MessageSource(message_id=...)`); `pass=3` (Pass 3, Q&A `add_to_kb` Sanitize) MAY carry either file-source or message-source because the Q&A path can sanitize file-derived chunks or chat-channel content (`docs/decisions.md` D-016). This invariant is the single semantic anchor that lets Trust Layer R-01 panel group Pass 1 vs Pass 2 redactions by source type without inspecting the underlying source string format.

#### Scenario: Audit log line contains required fields

- **WHEN** a Pass 1 sanitize replaces one value in `src/app.py`
- **THEN** `sanitize_audit.jsonl` MUST have one appended line containing fields `ts` (ISO 8601 UTC), `schema_version` (integer), `rules_version` (string), `pass` (integer 1, 2, or 3), `session_id` (UUID string), `source` (string prefixed with `file:` or `message:`), `rule_id` (string), `kind` (string), `placeholder_index` (integer), and `extra` (object)
- **AND** the line MUST NOT contain the original pre-sanitize value nor the surrounding context text

#### Scenario: Schema version starts at 1

- **WHEN** any audit line is written by this change
- **THEN** its `schema_version` field MUST equal `1`

#### Scenario: Concurrent writes append atomically

- **WHEN** two Pass calls write audit entries from two threads in the same process
- **THEN** every written line MUST be a complete JSON object terminated by `\n` and MUST NOT be interleaved with another line's bytes

#### Scenario: Filename literal is single-sourced in canonical leaf module

- **WHEN** any test scans `sidecar/src/codebus_agent/` for the regex `['\"][\w_-]+\.jsonl['\"]` (any `.jsonl` quoted string literal)
- **THEN** every match MUST originate from `sidecar/src/codebus_agent/_audit_paths.py`
- **AND** no other module in the package tree MUST contain a `*.jsonl` quoted string literal
- **AND** the rule applies to all seven workspace-level audit filenames: `sanitize_audit.jsonl`, `tool_audit.jsonl`, `token_usage.jsonl`, `llm_calls.jsonl`, `reasoning_log.jsonl`, `generator_log.jsonl`, `kb_growth.jsonl`

#### Scenario: pass_num to source-type invariant

- **WHEN** any test scans `<workspace>/.codebus/sanitize_audit.jsonl` for every line written by sidecar production code
- **THEN** every line whose `pass` field equals `1` MUST carry a `source` whose serialized shape reflects `FileSource(path=..., pass_=...)` — NEVER a `MessageSource(...)` shape
- **AND** every line whose `pass` field equals `2` MUST carry a `source` whose serialized shape reflects `MessageSource(message_id=...)` — NEVER a `FileSource(...)` shape
- **AND** every line whose `pass` field equals `3` MAY carry either a `FileSource` or `MessageSource` shape (Q&A `add_to_kb` accepts both per D-016)

#### Scenario: Explorer tool error path runs Pass 2 sanitize

- **WHEN** the Explorer ReAct loop's `_execute_tools` catches an exception from a tool invocation and the resulting error string is about to be written into `ToolResult.output`
- **THEN** the production code path MUST invoke `SanitizerEngine.sanitize(error_text, source=MessageSource(message_id=f"explorer_step_{step_idx}_tool_error"))` before populating `ToolResult.output`
- **AND** any sanitize hits MUST produce one `<workspace>/.codebus/sanitize_audit.jsonl` line per hit with `pass=2` and `source` shape reflecting `MessageSource(message_id=...)`
- **AND** the `ToolResult.output` ultimately written into the `Step` log MUST be the post-sanitize string (containing `<REDACTED:>` placeholders for any redactions)


<!-- @trace
source: agent-defense-depth
updated: 2026-04-27
code:
  - docs/sidecar-api.md
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/api/kb.py
  - CLAUDE.md
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/kb/payload.py
tests:
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
-->

---
### Requirement: Rules version is recorded on every audit line

Each line written to `sanitize_audit.jsonl` SHALL include the `rules_version` field sourced from the effective Sanitizer config, per `docs/decisions.md` D-015 and `docs/authorization.md §六`.

The `rules_version` value SHALL originate from a single module-level constant `RULES_VERSION` exported from the `codebus_agent.sanitizer` package. Production callers that stamp the version into audit records (currently `api/__init__.py` and `api/scan.py` via `TrackedProvider` / `Scanner` factory paths) MUST import this constant rather than declare independent string literals. This collapses Invariant 9 (`Sanitizer rules 改動必 bump version`) into a single edit point: bumping the constant in `codebus_agent.sanitizer` MUST propagate to every audit writer without requiring synchronized edits across multiple files.

#### Scenario: Rules version propagates from config

- **WHEN** the Sanitizer config declares `rules_version: "2026-04-20-1"`
- **AND** a Pass 1 sanitize produces an audit entry
- **THEN** the written JSONL line MUST contain `"rules_version": "2026-04-20-1"`

#### Scenario: Missing rules version rejected at config load

- **WHEN** a `sanitizer.local.yaml` is loaded without a `rules_version` field
- **THEN** the config loader MUST raise a validation error that names the missing field

#### Scenario: Single source of truth for rules_version constant

- **WHEN** the `codebus_agent.sanitizer` package is imported
- **THEN** it MUST expose a module-level `RULES_VERSION: str` constant
- **AND** every other production callsite that needs the rules version (notably `codebus_agent.api.__init__` and `codebus_agent.api.scan`) MUST import this constant rather than declare an independent string literal
- **AND** an integration test MUST assert that the constant resolved from `codebus_agent.sanitizer` is the same Python object (identity check, not equality) as the `_RULES_VERSION` symbols read from `codebus_agent.api.__init__` and `codebus_agent.api.scan`, so that future drift between three independent string literals is caught at test time


<!-- @trace
source: review-backlog-cleanup
updated: 2026-04-25
code:
  - sidecar/src/codebus_agent/scanner/service.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/api/__init__.py
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/providers/pricing.py
  - docs/decisions.md
  - sidecar/src/codebus_agent/providers/__init__.py
  - sidecar/src/codebus_agent/api/scan.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - docs/reviews/2026-04-25-stage-4.md
  - CLAUDE.md
  - sidecar/src/codebus_agent/generator/runner.py
  - sidecar/src/codebus_agent/providers/tracked.py
tests:
  - sidecar/tests/providers/test_tracked_chat_cost.py
  - sidecar/tests/sanitizer/test_rules_version_constant.py
  - sidecar/tests/providers/test_pricing.py
-->

---
### Requirement: SanitizerConfig loads from workspace-then-global YAML

The sidecar SHALL expose a `SanitizerConfig.load(workspace_root: Path) -> SanitizerConfig` loader that reads `{workspace_root}/sanitizer.local.yaml` when present, otherwise falls back to `~/.codebus/sanitizer.local.yaml`, otherwise uses built-in defaults, per `docs/sanitizer.md §五`. Workspace-level config SHALL replace (not deep-merge) the global config.

#### Scenario: Workspace config replaces global

- **WHEN** both `{workspace}/sanitizer.local.yaml` and `~/.codebus/sanitizer.local.yaml` exist with different `path_allowlist`
- **THEN** `SanitizerConfig.load` MUST return a config whose `path_allowlist` equals the workspace value, with no entries merged from the global file

#### Scenario: Fallback to global when workspace absent

- **WHEN** only `~/.codebus/sanitizer.local.yaml` exists
- **THEN** `SanitizerConfig.load` MUST return a config parsed from the global file

#### Scenario: Built-in defaults used when neither file present

- **WHEN** neither the workspace nor the global YAML file exists
- **THEN** `SanitizerConfig.load` MUST return a config whose `rules_version` and rule set come from compiled-in defaults

#### Scenario: Unknown config field rejected

- **WHEN** `sanitizer.local.yaml` contains a field not declared in the Pydantic model
- **THEN** config loading MUST raise a validation error naming the unknown field

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/config.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Config declares allowlist structure without requiring non-empty contents

The `SanitizerConfig` Pydantic model SHALL declare three allowlist fields — `path_allowlist: list[str]`, `filename_allowlist: list[str]`, and `pattern_allowlist: list[PatternAllowlistEntry]` — per `docs/sanitizer.md §四`. Each field SHALL default to an empty list. `PatternAllowlistEntry` SHALL contain `pattern: str` and `reason: str` fields, both required.

#### Scenario: Empty allowlists accepted

- **WHEN** `SanitizerConfig` is constructed with all three allowlists omitted
- **THEN** validation MUST succeed and each field MUST equal `[]`

#### Scenario: Pattern allowlist entry requires reason

- **WHEN** `pattern_allowlist` is constructed with `[{"pattern": "^FAKE_KEY_"}]` lacking `reason`
- **THEN** Pydantic validation MUST raise a ValidationError naming `reason`

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/config.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Allowlist hits still audited but not redacted

When a detected value matches an entry in `path_allowlist`, `filename_allowlist`, or `pattern_allowlist`, the `SanitizerEngine` SHALL leave the value unchanged in the output text, SHALL still emit an audit entry, and SHALL set `extra.allowlisted` to `true` on that entry, per `docs/sanitizer.md §四` "白名單".

#### Scenario: Pattern allowlist hit leaves text unchanged

- **WHEN** `pattern_allowlist` contains `[{"pattern": "^FAKE_KEY_", "reason": "test fixture"}]`
- **AND** `sanitize` is called with text `"FAKE_KEY_1234"` classified as a secret
- **THEN** the returned text MUST equal the input text exactly
- **AND** `SanitizedResult.entries` MUST contain exactly one entry with `extra.allowlisted == true`

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/config.py
-->


<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Placeholder index scope is single sanitize call

The `placeholder_index` values returned by a single `sanitize` call SHALL be independent of any other call's indices, per `docs/sanitizer.md §四`.

#### Scenario: Index does not leak across calls

- **WHEN** `sanitize(text_a, source_a)` returns entries with indices `[1, 2]`
- **AND** a subsequent `sanitize(text_b, source_b)` is invoked with a different source
- **THEN** the second call's indices MUST start from `1` and MUST NOT continue from `2`

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sanitizer/engine.py
-->
</content>
</invoke>

<!-- @trace
source: sanitizer-safety-chain
updated: 2026-04-21
code:
  - sidecar/src/codebus_agent/sandbox.py
  - sidecar/src/codebus_agent/providers/tracked.py
  - sidecar/uv.lock
  - sidecar/src/codebus_agent/sanitizer/engine.py
  - sidecar/src/codebus_agent/sanitizer/audit.py
  - sidecar/src/codebus_agent/sanitizer/__init__.py
  - sidecar/src/codebus_agent/sanitizer/config.py
  - sidecar/pyproject.toml
  - sidecar/src/codebus_agent/sanitizer/rules.py
tests:
  - sidecar/tests/providers/test_registry_guard_roles.py
  - sidecar/tests/providers/test_tracked_provider.py
  - sidecar/tests/sanitizer/fixtures/internal_ids_sample.txt
  - sidecar/tests/sandbox/test_tool_audit.py
  - sidecar/tests/sanitizer/fixtures/pii_sample.txt
  - sidecar/tests/sanitizer/test_config.py
  - sidecar/tests/sanitizer/test_audit.py
  - sidecar/tests/providers/test_no_outbound_per_role.py
  - sidecar/tests/providers/test_registry_role_dispatch.py
  - sidecar/tests/sanitizer/__init__.py
  - sidecar/tests/providers/test_tracked_role_audit.py
  - sidecar/tests/providers/test_tracked_pass2.py
  - sidecar/tests/sanitizer/test_allowlist.py
  - sidecar/tests/sanitizer/test_rules.py
  - sidecar/tests/test_sanitizer_safety_chain_integration.py
  - sidecar/tests/sanitizer/fixtures/secret_sample.txt
  - sidecar/tests/test_phase9_jsonl_acceptance.py
  - sidecar/tests/providers/test_registry.py
  - sidecar/tests/sanitizer/test_engine.py
-->

---
### Requirement: Pass 3 add_to_kb sanitize emits structured audit entry

The Q&A `add_to_kb` write path SHALL invoke `SanitizerEngine.sanitize(text, source=FileSource(path=chunk.source, pass_="qa_add_to_kb"))` for every chunk before any KB upsert or `kb_growth.jsonl` write, per `docs/decisions.md` D-015 and `docs/qa-agent.md §三`. Each `AuditEntry` produced MUST be appended to the workspace-scoped `SanitizerAuditLogger` with `pass_num=3` (Python keyword argument), completing the three-pass audit chain (Pass 1 = scanner ingestion, Pass 2 = TrackedProvider pre-flight, Pass 3 = Q&A add_to_kb).

**Naming convention — Python param vs JSONL key are intentionally different**: `pass_num` is the **Python keyword argument name** on `SanitizerAuditLogger.append(*, entry, pass_num, rules_version, session_id)`; the corresponding **JSONL key written to `<workspace>/.codebus/sanitize_audit.jsonl` is bare `pass`** (the integer value 1, 2, or 3 stays the same). The mismatch is deliberate: the Python name avoids shadowing the `pass` reserved keyword, while the JSONL key stays terse for readability in the audit panel. Implementers MUST NOT introduce `pass_num` as a JSONL key (e.g. by renaming the line dict's `"pass"` key) — every consumer (Trust Layer R-01 / O-05 panels, golden replay diff, audit join queries) reads the bare `pass` key directly.

`pass_num` (Python) / `pass` (JSONL) is the runtime label that downstream consumers use to attribute redactions to a sanitize stage. The `source` field on Pass 3 audit lines MUST be the structured form `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}` matching the existing structured shape used by Pass 1 scanner (`{"pass": "scanner", "path": ...}`). Note that the `pass` *value* inside the `source` object is a free-form string label (e.g. `"qa_add_to_kb"` / `"scanner"` / `"explorer_read_file"` / `"find_callers"` / `"grep_search"`) distinct from the integer-valued top-level `pass` key on the same JSONL line — both keys legitimately coexist with different semantics: the top-level integer marks which of the three Sanitize passes fired, the nested string marks which call-site within that pass produced the entry.

The `SanitizeSource` discriminated union (`FileSource | MessageSource`) SHALL NOT be extended for Pass 3; the existing `FileSource.pass_` string field is the explicit extension point already promised by the foundational `Sanitizer SHALL provide a stateless engine` Requirement (which states the same class is "reusable by Pass 3 without signature change"). Adding a third union variant is forbidden by this Requirement so the audit schema remains stable across Pass 1 / Pass 3 ingestion sites.

#### Scenario: add_to_kb chunk with secret hits writes pass=3 audit line

- **WHEN** Q&A `add_to_kb` is invoked with a chunk containing a string matched by the built-in secret rule set
- **THEN** the appended `<workspace>/.codebus/sanitize_audit.jsonl` line MUST contain the JSONL key `"pass"` with integer value `3` (NOT a key named `"pass_num"` — the Python param name does not propagate to the wire schema)
- **AND** the line's `source` field MUST be the JSON object `{"pass": "qa_add_to_kb", "path": "<chunk.source>"}` (the inner `pass` string label is distinct from the top-level integer `pass` key on the same line — both legitimately coexist)
- **AND** the placeholder index MUST start at `1` for that sanitize call (Pass 3 calls share the same per-call index reset semantics as Pass 1 / Pass 2)

#### Scenario: SanitizeSource union not extended

- **WHEN** the codebase is inspected for `SanitizeSource = ` assignments in `codebus_agent.sanitizer`
- **THEN** the right-hand side MUST remain exactly `FileSource | MessageSource` — Pass 3 MUST NOT introduce a new variant such as `Pass3Source` or `QASource`

#### Scenario: Empty post-sanitize chunk still records hit lines

- **WHEN** `add_to_kb` sanitizes a chunk whose entire text gets replaced (post-sanitize text strips to empty)
- **THEN** every triggered redaction MUST still produce a `pass=3` line in `sanitize_audit.jsonl` (using the JSONL key `"pass"` with integer value `3`, not `"pass_num"`)
- **AND** the call MUST proceed to skip the KB upsert and `kb_growth.jsonl` write per the Q&A capability's empty-chunk handling — but the sanitize audit lines MUST NOT be retroactively suppressed

#### Scenario: JSONL key is bare `pass`, never `pass_num`

- **WHEN** any test reads a line from `<workspace>/.codebus/sanitize_audit.jsonl` produced by `SanitizerAuditLogger.append`
- **THEN** the parsed JSON object MUST contain a key named exactly `"pass"` with an integer value in `{1, 2, 3}`
- **AND** the parsed JSON object MUST NOT contain a key named `"pass_num"` at the top level (the Python keyword argument name MUST NOT leak to the wire schema)
- **AND** this rule applies uniformly to all three passes (Pass 1 scanner / Pass 2 provider pre-flight / Pass 3 add_to_kb) — none of them MUST emit `"pass_num"` as a JSONL top-level key

<!-- @trace
source: spec-cleanup-stage-5-batch-b
updated: 2026-04-27
code:
  - sidecar/src/codebus_agent/agent/tools/folder_tools.py
  - sidecar/src/codebus_agent/agent/tools/kb_search.py
  - sidecar/src/codebus_agent/kb/knowledge_base.py
  - sidecar/src/codebus_agent/agent/explorer.py
  - sidecar/src/codebus_agent/agent/station_id.py
  - sidecar/src/codebus_agent/agent/tools/add_to_kb.py
  - docs/sidecar-api.md
  - CLAUDE.md
  - docs/reviews/2026-04-26-stage-5.md
  - sidecar/src/codebus_agent/kb/payload.py
  - sidecar/src/codebus_agent/api/kb.py
  - sidecar/src/codebus_agent/api/qa.py
  - sidecar/src/codebus_agent/kb/growth_logger.py
  - sidecar/src/codebus_agent/api/scan.py
tests:
  - sidecar/tests/api/test_scan_stream.py
  - sidecar/tests/agent/test_station_id_constant.py
  - sidecar/tests/agent/tools/test_grep_fallback_sanitize.py
  - sidecar/tests/api/test_kb_build.py
  - sidecar/tests/test_no_jsonl_literal_drift.py
  - sidecar/tests/agent/test_explorer_error_sanitize.py
  - sidecar/tests/agent/test_qa_constants_single_source.py
  - sidecar/tests/api/test_kb_build_status_code.py
  - sidecar/tests/api/test_kb_build_production.py
  - sidecar/tests/agent/tools/test_pass1_source_type.py
  - sidecar/tests/sanitizer/test_pass_source_invariant.py
-->
