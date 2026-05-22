## MODIFIED Requirements

### Requirement: Endpoint Profile Schema

The `~/.codebus/config.yaml` file SHALL accept an `agent` block with two top-level keys: `active_provider` and `providers`. `active_provider` SHALL be a string naming the active provider; the supported values SHALL be `claude` and `codex` (the `codex` provider's block shape is defined by the `codex-config` capability). An `active_provider` value outside this set SHALL be rejected with `ConfigLoadError::YamlParse`. `providers` SHALL be a map keyed by provider name. The `providers.claude` block SHALL contain three keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure` (the active endpoint profile for the claude provider). The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each verb sub-block SHALL contain a `model` field (a `SystemModel` enum value) and an optional `effort` field (an arbitrary string). The `azure` block SHALL contain `base_url` (URL string), `keyring_service` (arbitrary string, default `codebus-azure`), and the same four verb sub-blocks (`goal`, `query`, `fix`, `verify`); in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string and `effort` SHALL be an optional arbitrary string.

The endpoint profile referenced by `providers.claude.active` MUST be fully populated for the load to succeed — all four verb sub-blocks (`goal`, `query`, `fix`, `verify`) are required when the profile is active. The non-active endpoint profile MAY be absent or partially populated; codebus SHALL NOT validate fields of the non-active profile. If the `agent` block is absent entirely, the system SHALL fall back to a built-in default equivalent to `active_provider: claude` with `providers.claude.active: system` and verb defaults `goal: opus-4-6` / `query: haiku-4-5` / `fix: sonnet-4-6` / `verify: opus-4-6` and per-verb default `effort` values `high` / `low` / `medium` / `high` respectively.

The `verify` sub-block SHALL govern the model and effort used by the independent content-verification spawn run by `quiz` and `goal` verbs after their main generation phase (see `Quiz Content Verification and Repair` in the `quiz` capability and `Goal Content Verification and Repair` in the `verb-library` capability). The `verify` sub-block SHALL NOT be referenced by any other spawn (quiz plan / quiz generate / quiz repair / goal main / goal repair / fix); those spawns continue to use their own verb's sub-block.

The `Verb` resolution enum SHALL include a `Verify` variant alongside the existing `Goal` / `Query` / `Fix` / `Chat` / `Quiz` variants. The resolution function SHALL map `Verb::Verify` directly to the `verify` sub-block of the active endpoint profile. Unlike `Chat` and `Quiz` which reuse the `Query` sub-block by design, `Verb::Verify` SHALL NOT fall back to any other verb's sub-block.

#### Scenario: System profile loads with all four verbs populated

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: claude` and a `agent.providers.claude` block with `active: system` and a `system` block with all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** the config loader SHALL return a config with the claude provider and system endpoint profile selected and the parsed verb settings

#### Scenario: Azure profile loads with required fields

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: claude` and a `agent.providers.claude` block with `active: azure` and an `azure` block with `base_url`, `keyring_service`, and all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** the config loader SHALL return a config with the azure endpoint profile selected and the parsed verb settings, `base_url`, and `keyring_service`

#### Scenario: Codex active_provider is accepted

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: codex` and a valid `agent.providers.codex` block
- **THEN** the config loader SHALL NOT reject the load on the basis of the provider name (the codex provider block is validated per the `codex-config` capability)

#### Scenario: Unsupported provider name rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: gemini`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the unsupported provider name

#### Scenario: Active azure but azure block missing base_url fails

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: azure` and an `azure` block lacking `base_url`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` and SHALL NOT silently fall back to defaults

#### Scenario: Active system but system block missing verify fails

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` and a `system` block with `goal`, `query`, `fix` populated but `verify` absent
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `system.verify` sub-block as the missing required field AND SHALL NOT silently fall back to defaults

#### Scenario: Active azure but azure block missing verify fails

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: azure` and an `azure` block with `goal`, `query`, `fix` populated but `verify` absent
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `azure.verify` sub-block as the missing required field

#### Scenario: Non-active profile may be partial

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` with a complete `system` block AND an `azure` block missing `keyring_service`
- **THEN** the config loader SHALL return a config with the system endpoint profile selected and SHALL NOT fail due to the incomplete azure profile

#### Scenario: Non-active profile may omit verify

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` with a complete `system` block (all four verbs populated) AND a partial `azure` block missing the `verify` sub-block
- **THEN** the config loader SHALL return a config with the system endpoint profile selected and SHALL NOT fail due to the incomplete azure profile (verify required only on the active profile)

#### Scenario: Invalid SystemModel value rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.system.goal.model: gpt-4`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the invalid enum value

#### Scenario: Verb::Verify resolves to system.verify sub-block

- **WHEN** the system endpoint profile is active with `system.verify: { model: opus-4-6, effort: high }` and `system.query: { model: haiku-4-5, effort: low }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: opus-4-6` and `effort: high` AND SHALL NOT fall back to the `query` sub-block

#### Scenario: Verb::Verify resolves to azure.verify sub-block

- **WHEN** the azure endpoint profile is active with `azure.verify: { model: claude-opus-deploy, effort: high }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: claude-opus-deploy` and `effort: high` (azure deployment names pass through verbatim)
