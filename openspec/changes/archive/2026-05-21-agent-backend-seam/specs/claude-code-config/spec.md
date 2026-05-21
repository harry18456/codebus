## MODIFIED Requirements

### Requirement: Endpoint Profile Schema

The `~/.codebus/config.yaml` file SHALL accept an `agent` block with two top-level keys: `active_provider` and `providers`. `active_provider` SHALL be a string naming the active provider; within this capability's scope the only supported value SHALL be `claude`. `providers` SHALL be a map keyed by provider name. The `providers.claude` block SHALL contain three keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure` (the active endpoint profile for the claude provider). The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each verb sub-block SHALL contain a `model` field (a `SystemModel` enum value) and an optional `effort` field (an arbitrary string). The `azure` block SHALL contain `base_url` (URL string), `keyring_service` (arbitrary string, default `codebus-azure`), and the same four verb sub-blocks (`goal`, `query`, `fix`, `verify`); in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string and `effort` SHALL be an optional arbitrary string.

The endpoint profile referenced by `providers.claude.active` MUST be fully populated for the load to succeed — all four verb sub-blocks (`goal`, `query`, `fix`, `verify`) are required when the profile is active. The non-active endpoint profile MAY be absent or partially populated; codebus SHALL NOT validate fields of the non-active profile. If the `agent` block is absent entirely, the system SHALL fall back to a built-in default equivalent to `active_provider: claude` with `providers.claude.active: system` and verb defaults `goal: opus-4-6` / `query: haiku-4-5` / `fix: sonnet-4-6` / `verify: opus-4-6` and per-verb default `effort` values `high` / `low` / `medium` / `high` respectively.

The `verify` sub-block SHALL govern the model and effort used by the independent content-verification spawn run by `quiz` and `goal` verbs after their main generation phase (see `Quiz Content Verification and Repair` in the `quiz` capability and `Goal Content Verification and Repair` in the `verb-library` capability). The `verify` sub-block SHALL NOT be referenced by any other spawn (quiz plan / quiz generate / quiz repair / goal main / goal repair / fix); those spawns continue to use their own verb's sub-block.

The `Verb` resolution enum SHALL include a `Verify` variant alongside the existing `Goal` / `Query` / `Fix` / `Chat` / `Quiz` variants. The resolution function SHALL map `Verb::Verify` directly to the `verify` sub-block of the active endpoint profile. Unlike `Chat` and `Quiz` which reuse the `Query` sub-block by design, `Verb::Verify` SHALL NOT fall back to any other verb's sub-block.

#### Scenario: System profile loads with all four verbs populated

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: claude` and a `agent.providers.claude` block with `active: system` and a `system` block with all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** the config loader SHALL return a config with the claude provider and system endpoint profile selected and the parsed verb settings

#### Scenario: Azure profile loads with required fields

- **WHEN** `~/.codebus/config.yaml` contains `agent.active_provider: claude` and a `agent.providers.claude` block with `active: azure` and an `azure` block with `base_url`, `keyring_service`, and all four verbs (`goal`, `query`, `fix`, `verify`) populated
- **THEN** the config loader SHALL return a config with the azure endpoint profile selected and the parsed verb settings, `base_url`, and `keyring_service`

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

##### Example: schema shape

```yaml
agent:
  active_provider: claude
  providers:
    claude:
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

### Requirement: System Profile Model Aliases

The `SystemModel` type SHALL be a closed enum with exactly four variants serialised in kebab-case, each carrying an explicit version suffix: `opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6`. Unversioned aliases (`opus`, `haiku`, `sonnet`) SHALL be rejected at deserialisation. Codebus SHALL maintain a deterministic mapping `to_cli_flag` from each variant to the string passed to the Claude CLI's `--model` flag. The mapping SHALL be: `opus-4-7` → `claude-opus-4-7`, `opus-4-6` → `claude-opus-4-6`, `haiku-4-5` → `claude-haiku-4-5`, `sonnet-4-6` → `claude-sonnet-4-6`. The mapping SHALL be applied immediately before spawning the child process.

#### Scenario: opus-4-6 alias resolves to claude-opus-4-6

- **WHEN** the system endpoint profile is active and the goal verb's `model` is `opus-4-6`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-opus-4-6`

#### Scenario: haiku-4-5 alias resolves to claude-haiku-4-5

- **WHEN** the system endpoint profile is active and the query verb's `model` is `haiku-4-5`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-haiku-4-5`

#### Scenario: Unversioned alias rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.system.query.model: haiku` (without version suffix)
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the invalid enum value

##### Example: alias mapping table

| `SystemModel` value | `--model` flag value passed to claude CLI |
| ------------------- | ----------------------------------------- |
| `opus-4-7`          | `claude-opus-4-7`                         |
| `opus-4-6`          | `claude-opus-4-6`                         |
| `haiku-4-5`         | `claude-haiku-4-5`                        |
| `sonnet-4-6`        | `claude-sonnet-4-6`                       |

## REMOVED Requirements

### Requirement: Legacy Config Schema Warning Without Rewrite

**Reason**: 專案尚未 release，採全新統一 `agent.providers.*` schema，無遷移、無向後相容需求。legacy `claude_code.*` 頂層 verb key 的偵測與遷移警告整段移除——讀到舊 schema 不再印警告，視為無 `agent` 區塊而落回 provider 預設。

**Migration**: 開發者本機重跑 `codebus init` 產生新 `agent.*` 格式 config，或手動把 `claude_code:` 區塊改寫為 `agent.providers.claude:`（內層 `active`/`system`/`azure` 結構不變）。

#### Scenario: Legacy top-level claude_code no longer warns

- **WHEN** `~/.codebus/config.yaml` contains a top-level `claude_code` block (legacy schema) and no `agent` block
- **THEN** the config loader SHALL NOT print a migration warning AND SHALL treat the configuration as if the `agent` block were absent, falling back to the built-in provider defaults (`active_provider: claude`, `providers.claude.active: system`)
