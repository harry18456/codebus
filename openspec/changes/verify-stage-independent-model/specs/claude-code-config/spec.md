## MODIFIED Requirements

### Requirement: Endpoint Profile Schema

The `~/.codebus/config.yaml` file SHALL accept a `claude_code` block with three top-level keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure`. The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each verb sub-block SHALL contain a `model` field (a `SystemModel` enum value) and an optional `effort` field (an arbitrary string). The `azure` block SHALL contain `base_url` (URL string), `keyring_service` (arbitrary string, default `codebus-azure`), and the same four verb sub-blocks (`goal`, `query`, `fix`, `verify`); in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string and `effort` SHALL be an optional arbitrary string.

The profile referenced by `active` MUST be fully populated for the load to succeed — all four verb sub-blocks (`goal`, `query`, `fix`, `verify`) are required when the profile is active. The non-active profile MAY be absent or partially populated; codebus SHALL NOT validate fields of the non-active profile. If the `claude_code` block is absent entirely, the system SHALL fall back to a built-in default profile equivalent to `active: system` with verb defaults `goal: opus-4-6` / `query: haiku-4-5` / `fix: sonnet-4-6` / `verify: opus-4-6` and per-verb default `effort` values `high` / `low` / `medium` / `high` respectively.

The `verify` sub-block SHALL govern the model and effort used by the independent content-verification spawn run by `quiz` and `goal` verbs after their main generation phase (see `Quiz Content Verification and Repair` in the `quiz` capability and `Goal Content Verification and Repair` in the `verb-library` capability). The `verify` sub-block SHALL NOT be referenced by any other spawn (quiz plan / quiz generate / quiz repair / goal main / goal repair / fix); those spawns continue to use their own verb's sub-block.

The `Verb` resolution enum SHALL include a `Verify` variant alongside the existing `Goal` / `Query` / `Fix` / `Chat` / `Quiz` variants. The resolution function SHALL map `Verb::Verify` directly to the `verify` sub-block of the active profile. Unlike `Chat` and `Quiz` which reuse the `Query` sub-block by design, `Verb::Verify` SHALL NOT fall back to any other verb's sub-block.

#### Scenario: System profile loads with all four verbs populated

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: system` and a `claude_code.system` block with all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** `load_claude_code_config` SHALL return a config with the system profile selected and the parsed verb settings

#### Scenario: Azure profile loads with required fields

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: azure` and a `claude_code.azure` block with `base_url`, `keyring_service`, and all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** `load_claude_code_config` SHALL return a config with the azure profile selected and the parsed verb settings, `base_url`, and `keyring_service`

#### Scenario: Active azure but azure block missing base_url fails

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: azure` and a `claude_code.azure` block lacking `base_url`
- **THEN** `load_claude_code_config` SHALL return `ConfigLoadError::YamlParse` and SHALL NOT silently fall back to defaults

#### Scenario: Active system but system block missing verify fails

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: system` and a `claude_code.system` block with `goal`, `query`, `fix` populated but `verify` absent
- **THEN** `load_claude_code_config` SHALL return `ConfigLoadError::YamlParse` identifying `claude_code.system.verify` as the missing required field AND SHALL NOT silently fall back to defaults

#### Scenario: Active azure but azure block missing verify fails

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: azure` and a `claude_code.azure` block with `goal`, `query`, `fix` populated but `verify` absent
- **THEN** `load_claude_code_config` SHALL return `ConfigLoadError::YamlParse` identifying `claude_code.azure.verify` as the missing required field

#### Scenario: Non-active profile may be partial

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: system` with a complete `system` block AND an `azure` block missing `keyring_service`
- **THEN** `load_claude_code_config` SHALL return a config with the system profile selected and SHALL NOT fail due to the incomplete azure profile

#### Scenario: Non-active profile may omit verify

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.active: system` with a complete `system` block (all four verbs populated) AND a partial `azure` block missing the `verify` sub-block
- **THEN** `load_claude_code_config` SHALL return a config with the system profile selected and SHALL NOT fail due to the incomplete azure profile (verify required only on the active profile)

#### Scenario: Invalid SystemModel value rejected

- **WHEN** `~/.codebus/config.yaml` contains `claude_code.system.goal.model: gpt-4`
- **THEN** `load_claude_code_config` SHALL return `ConfigLoadError::YamlParse` identifying the invalid enum value

#### Scenario: Verb::Verify resolves to system.verify sub-block

- **WHEN** the system profile is active with `system.verify: { model: opus-4-6, effort: high }` and `system.query: { model: haiku-4-5, effort: low }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: opus-4-6` and `effort: high` AND SHALL NOT fall back to the `query` sub-block

#### Scenario: Verb::Verify resolves to azure.verify sub-block

- **WHEN** the azure profile is active with `azure.verify: { model: claude-opus-deploy, effort: high }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: claude-opus-deploy` and `effort: high` (azure deployment names pass through verbatim)

##### Example: schema shape

```yaml
claude_code:
  active: azure
  system:
    goal:   { model: opus-4-6,   effort: high   }
    query:  { model: haiku-4-5,  effort: low    }
    fix:    { model: sonnet-4-6, effort: medium }
    verify: { model: opus-4-6,   effort: high   }
  azure:
    base_url: https://example.cognitiveservices.azure.com/anthropic
    keyring_service: codebus-azure
    goal:   { model: claude-opus-4-6-2026V2,   effort: high   }
    query:  { model: claude-haiku-4-5-2026V2,  effort: low    }
    fix:    { model: claude-sonnet-4-6-2026V2, effort: medium }
    verify: { model: claude-opus-4-6-2026V2,   effort: high   }
```
