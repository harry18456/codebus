## MODIFIED Requirements

### Requirement: Load global config tolerantly

The system SHALL load `~/.codebus/config.yaml` if present, ignore unknown fields silently (forward-compat for future schema growth), warn but not abort on parse errors, and warn on unknown values for known fields.

The config schema SHALL recognize the following top-level keys, each optional and tolerantly parsed:

- `emoji`: emoji mode preference (one of `auto` | `on` | `off`)
- `llm`: LLM provider configuration block (provider kind plus provider-specific sub-fields)
- `pii`: PII scanner configuration block (scanner kind, on-hit behavior, extra patterns)
- `lint`: lint rule configuration block (rule overrides, disabled rules, custom rules path)
- `render`: output renderer configuration block (renderer format)
- `log`: log sink configuration block (sink kind, retention)

For each of the five plugin section keys (`llm`, `pii`, `lint`, `render`, `log`), the loader SHALL:

- Parse the section if present and value is a YAML mapping; produce an empty section if absent or null
- Within each section, recognize provider/scanner/rule/renderer/sink kind via a `provider` / `scanner` / `format` / `sink` discriminator field as appropriate to the section
- Silently ignore sub-fields under a section that the loader does not recognize (forward-compat for future plugin additions)
- Warn but not abort when a discriminator field has an unknown value, treating that section as unset (factory falls through to default)
- Warn but not abort when a sub-field has a type-incompatible value (e.g., `timeout_secs: "thirty"` where number expected), treating that sub-field as unset

#### Scenario: Missing config returns empty config

- **WHEN** `~/.codebus/config.yaml` does not exist
- **THEN** the system returns an empty config object without error

#### Scenario: Invalid YAML warns and falls back to empty

- **WHEN** `~/.codebus/config.yaml` contains malformed YAML
- **THEN** the system writes a warning to stderr and returns an empty config object

#### Scenario: Unknown emoji value warns and is ignored

- **WHEN** `~/.codebus/config.yaml` contains `emoji: maybe`
- **THEN** the system writes a warning, and `emoji` is treated as unset (falling through to next priority level)

#### Scenario: Future top-level fields are silently ignored

- **WHEN** `~/.codebus/config.yaml` contains a top-level field not in the recognized set (`emoji`, `llm`, `pii`, `lint`, `render`, `log`)
- **THEN** the system silently ignores the field without warning

#### Scenario: LLM section selects provider via discriminator

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, binary_path: /usr/local/bin/claude }`
- **THEN** the loader returns an LLM config with provider kind `claude_cli` and the `binary_path` sub-field populated

#### Scenario: Unknown LLM provider warns and is treated as unset

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: gibberish, api_key: x }`
- **THEN** the system writes a warning identifying the unknown provider, and the LLM section is treated as unset (factory uses default `claude_cli` provider)

#### Scenario: Unknown sub-field within a known section is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, future_unknown_field: 1 }`
- **THEN** the loader honors `provider: claude_cli` and silently ignores `future_unknown_field`

#### Scenario: PII section selects scanner via discriminator

- **WHEN** `~/.codebus/config.yaml` contains `pii: { scanner: regex_basic, on_hit: warn, patterns_extra: ["INTERNAL-\\d{6}"] }`
- **THEN** the loader returns a PII config with scanner kind `regex_basic`, on-hit `warn`, and one extra pattern

#### Scenario: Lint section overrides recognized

- **WHEN** `~/.codebus/config.yaml` contains `lint: { disabled_rules: [oversize-page] }`
- **THEN** the loader returns a lint config with the rule `oversize-page` listed in disabled rules

#### Scenario: Render section selects renderer

- **WHEN** `~/.codebus/config.yaml` contains `render: { format: terminal }`
- **THEN** the loader returns a render config with format `terminal`

#### Scenario: Log section selects sink

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl, retention_days: 30 }`
- **THEN** the loader returns a log config with sink kind `jsonl` and retention 30 days

#### Scenario: Empty plugin section parses as defaults

- **WHEN** `~/.codebus/config.yaml` contains `pii: {}`
- **THEN** the loader returns a PII config with all fields at their defaults (scanner unset, factory uses `null` scanner)

#### Scenario: Type-mismatched sub-field warns and is treated as unset

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, timeout_secs: "thirty" }` (string where number expected)
- **THEN** the system writes a warning, the `timeout_secs` sub-field is treated as unset, and the rest of the LLM section is honored
