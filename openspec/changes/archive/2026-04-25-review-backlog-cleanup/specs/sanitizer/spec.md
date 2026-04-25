## MODIFIED Requirements

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
