## MODIFIED Requirements

### Requirement: Load global config tolerantly

The system SHALL load `~/.codebus/config.yaml` if present, ignore unknown fields silently (forward-compat for future schema growth), warn but not abort on parse errors, and warn on unknown values for known fields.

The config schema SHALL recognize the following top-level keys, each optional and tolerantly parsed:

- `emoji`: emoji mode preference (one of `auto` | `on` | `off`)
- `llm`: LLM provider configuration block. Discriminator field `provider` selects one of `claude_cli` | `anthropic_api` | `openai` | `ollama_local`; the remaining sub-fields under the `llm` mapping are the variant-specific fields valid for the selected provider. For `claude_cli` the recognized sub-fields are `binary_path`, `model`, and `effort`.
- `pii`: PII scanner configuration block. Discriminator field `scanner` selects one of `null` | `regex_basic` | `presidio` | `aws`; the remaining sub-fields are variant-specific fields valid for the selected scanner (every variant accepts `on_hit`).
- `lint`: lint rule configuration block (rule overrides, disabled rules, custom rules path). Not a tagged-variant section.
- `render`: output renderer configuration block. Discriminator field `format` selects one of `terminal` | `json_lines` | `tauri`; the remaining sub-fields are variant-specific.
- `log`: log sink configuration block. Discriminator field `sink` selects one of `null` | `jsonl` | `otel`; the remaining sub-fields are variant-specific. For the `jsonl` sink, the recognized sub-field is `dir`, which is optional — when omitted, the system SHALL fall back to `<repo>/.codebus/logs/` for the active vault.

For each of the four tagged-variant plugin section keys (`llm`, `pii`, `render`, `log`), the loader SHALL:

- Parse the section if present and value is a YAML mapping; produce an empty section (treated as the variant's default) if absent or null
- Within each section, recognize the variant via its discriminator field (`provider` / `scanner` / `format` / `sink`)
- Silently ignore sub-fields under a section that the chosen variant does not define (forward-compat for future field additions and tolerance for fields valid in a sibling variant)
- Warn but not abort when a discriminator field has an unknown value, treating that section as unset (factory falls through to default)
- Warn but not abort when a sub-field has a type-incompatible value (e.g., `timeout_secs: "thirty"` where number expected), treating that sub-field as unset (other valid sub-fields in the same section are preserved)

For the `lint` section, the loader SHALL parse the (non-discriminated) struct as before, ignoring unknown sub-fields and warning on type-incompatible sub-fields.

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
- **THEN** the loader returns an LLM config of variant `claude_cli` with the `binary_path` sub-field populated

#### Scenario: Unknown LLM provider warns and is treated as unset

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: gibberish, api_key: x }`
- **THEN** the system writes a warning identifying the unknown provider, and the LLM section is treated as unset (factory uses default `claude_cli` provider)

#### Scenario: Unknown sub-field within a known section is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, future_unknown_field: 1 }`
- **THEN** the loader honors `provider: claude_cli` and silently ignores `future_unknown_field`

#### Scenario: Sub-field valid in a sibling variant is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, api_key: secret }` (where `api_key` is valid for the `anthropic_api` and `openai` variants but not for `claude_cli`)
- **THEN** the loader honors `provider: claude_cli` and silently ignores `api_key`, matching the behavior for any unknown sub-field under the chosen variant

#### Scenario: ClaudeCli model and effort are parsed when present

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli, model: sonnet, effort: high }`
- **THEN** the loader returns an LLM config of variant `claude_cli` with `model` set to `"sonnet"` and `effort` set to `"high"` (both stored as opaque strings; the loader does not validate the values against the Claude CLI's accepted aliases)

#### Scenario: ClaudeCli model and effort default to None when absent

- **WHEN** `~/.codebus/config.yaml` contains `llm: { provider: claude_cli }` with no `model` or `effort` sub-field
- **THEN** the loader returns an LLM config of variant `claude_cli` with both `model` and `effort` as `None`, so the agent is spawned with the Claude CLI's default model and effort

#### Scenario: PII section selects scanner via discriminator

- **WHEN** `~/.codebus/config.yaml` contains `pii: { scanner: regex_basic, on_hit: warn, patterns_extra: ["INTERNAL-\\d{6}"] }`
- **THEN** the loader returns a PII config of variant `regex_basic` with on-hit `warn` and one extra pattern

#### Scenario: Lint section overrides recognized

- **WHEN** `~/.codebus/config.yaml` contains `lint: { disabled_rules: [oversize-page] }`
- **THEN** the loader returns a lint config with the rule `oversize-page` listed in disabled rules

#### Scenario: Render section selects renderer

- **WHEN** `~/.codebus/config.yaml` contains `render: { format: terminal }`
- **THEN** the loader returns a render config of variant `terminal`

#### Scenario: Log section selects jsonl sink with explicit dir

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl, dir: /var/log/codebus }`
- **THEN** the loader returns a log config of variant `jsonl` with directory `/var/log/codebus`

#### Scenario: Log section selects jsonl sink without dir defaulting to vault

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl }` with no `dir` sub-field
- **THEN** the loader returns a log config of variant `jsonl` with `dir: None`; downstream the run flow SHALL fall back to `<repo>/.codebus/logs/` of the active vault

#### Scenario: Log section retention_days is silently ignored

- **WHEN** `~/.codebus/config.yaml` contains `log: { sink: jsonl, dir: /var/log/codebus, retention_days: 30 }`
- **THEN** the loader returns a log config of variant `jsonl` with `dir: /var/log/codebus` and silently ignores `retention_days` (the field has been removed from the schema; this matches the behavior for any unknown sub-field under the chosen variant)

#### Scenario: Empty plugin section parses as defaults

- **WHEN** `~/.codebus/config.yaml` contains `pii: {}`
- **THEN** the loader returns a PII config at the default variant (`null` scanner with on-hit `warn`)
