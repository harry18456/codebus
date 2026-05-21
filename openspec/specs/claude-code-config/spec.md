# claude-code-config Specification

## Purpose

The endpoint profile configuration and scoped environment injection that codebus uses when spawning the Claude CLI child process — the `~/.codebus/config.yaml` `claude_code` block's `active` selector plus `system` / `azure` profile shape, the `SystemModel` enum and its `--model` flag mapping, azure profile model-string passthrough, OS keyring storage (`CODEBUS_AZURE_KEY` env fallback when keyring backend is unavailable), `Command::env`-only scoped env injection at spawn time (no parent-shell mutation), legacy schema warning without on-disk rewrite, and the `codebus config` subcommand for keyring management. Does NOT cover the verb-level spawn flags (`--tools`, `--allowedTools`, `--permission-mode`, stream-json flags) which live in `cli`'s per-verb Subcommand Behavior requirements, nor the agent stream parsing pipeline (lives in `agent-stream-rendering`).

## Requirements

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


<!-- @trace
source: agent-backend-seam
updated: 2026-05-21
code:
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/mod.rs
  - docs/2026-05-21-multi-provider-design-discussion.md
  - codebus-core/src/agent/spawn_spec.rs
  - codebus-core/src/verb/query.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-app/src/store/settings.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/verb/chat.rs
  - docs/v3-roadmap.md
tests:
  - codebus-core/tests/endpoint_config_load.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/azure_key_pre_spawn.rs
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
  - codebus-cli/tests/config_subcommand.rs
  - codebus-cli/tests/scoped_env_injection.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-cli/tests/goal_content_verify_cli.rs
-->

---
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


<!-- @trace
source: agent-backend-seam
updated: 2026-05-21
code:
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/mod.rs
  - docs/2026-05-21-multi-provider-design-discussion.md
  - codebus-core/src/agent/spawn_spec.rs
  - codebus-core/src/verb/query.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/agent/claude_cli.rs
  - codebus-app/src/store/settings.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/config/global_starter.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/wiki/fix/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/verb/chat.rs
  - docs/v3-roadmap.md
tests:
  - codebus-core/tests/endpoint_config_load.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/azure_key_pre_spawn.rs
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
  - codebus-cli/tests/config_subcommand.rs
  - codebus-cli/tests/scoped_env_injection.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-cli/tests/goal_content_verify_cli.rs
-->

---
### Requirement: Azure Profile Model String Passthrough

When the azure profile is active, codebus SHALL pass each verb's `model` field verbatim to the Claude CLI's `--model` flag. Codebus SHALL NOT validate, translate, or rewrite the string under any circumstance. The string SHALL be treated as an Azure deployment name even when its value matches a `SystemModel` enum literal.

#### Scenario: Azure deployment name passes through

- **WHEN** the azure profile is active and the goal verb's `model` is `claude-opus-4-6-2026V2`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-opus-4-6-2026V2`

#### Scenario: Azure mode does not translate system-style alias

- **WHEN** the azure profile is active and the goal verb's `model` is `opus-4-6`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model opus-4-6` AND codebus SHALL NOT translate the value to `claude-opus-4-6`

---
### Requirement: OS Keyring Integration With Env Fallback

Codebus SHALL store the Azure API key in the operating-system keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service or KWallet). The keyring entry SHALL be addressed by `(service, account)` where `service` is the value of `azure.keyring_service` (default `codebus-azure`) and `account` is the fixed literal `default`. The `account` value SHALL NOT be user-configurable in this change.

Before spawning any child process while the azure profile is active, codebus SHALL resolve the API key using the following fallback chain in order:

1. Read the password from the keyring entry `(azure.keyring_service, "default")`.
2. If the keyring backend is unavailable OR the entry does not exist, read the environment variable `CODEBUS_AZURE_KEY`.
3. If both sources are absent, codebus SHALL return an `EndpointKeyMissing` error AND SHALL NOT spawn the child process.

#### Scenario: Keyring read succeeds and key is injected

- **WHEN** the azure profile is active, the keyring entry exists with value `sk-test`, and `codebus query` is invoked
- **THEN** the spawned `claude` child process environment SHALL contain `ANTHROPIC_API_KEY=sk-test` AND the parent shell environment SHALL NOT be modified

#### Scenario: Keyring unavailable falls back to env

- **WHEN** the azure profile is active, the keyring backend is unavailable, the environment variable `CODEBUS_AZURE_KEY=sk-fallback` is set, and `codebus query` is invoked
- **THEN** the spawned `claude` child process environment SHALL contain `ANTHROPIC_API_KEY=sk-fallback`

#### Scenario: Neither source available aborts before spawn

- **WHEN** the azure profile is active, the keyring entry does not exist, and `CODEBUS_AZURE_KEY` is unset
- **THEN** `codebus query` SHALL exit with non-zero status, stderr SHALL contain an `EndpointKeyMissing` error message naming the keyring service AND the `CODEBUS_AZURE_KEY` env var, AND the `claude` child process SHALL NOT be spawned

---
### Requirement: Scoped Environment Injection At Spawn

The `agent::claude_cli::invoke` function SHALL spawn the `claude` child process with environment variables injected exclusively via the `Command::env` / `Command::envs` Rust API. Codebus SHALL NOT modify the parent process's environment (no `std::env::set_var` calls) at any point in the spawn pipeline. When the system profile is active, codebus SHALL inject zero additional environment variables. When the azure profile is active, codebus SHALL inject exactly three environment variables on the child process: `ANTHROPIC_BASE_URL` (from `azure.base_url`), `ANTHROPIC_API_KEY` (from the keyring fallback chain), and `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` set to the literal string `1`.

#### Scenario: System profile injects no env

- **WHEN** the system profile is active and `codebus query` is invoked
- **THEN** the spawned `claude` child process SHALL inherit the parent environment unchanged AND no additional environment variables SHALL be set via `Command::env`

#### Scenario: Azure profile injects exactly three env vars

- **WHEN** the azure profile is active with `base_url=https://example.cognitiveservices.azure.com/anthropic`, the keyring key resolves to `sk-test`, and `codebus query` is invoked
- **THEN** the spawned `claude` child process environment SHALL contain `ANTHROPIC_BASE_URL=https://example.cognitiveservices.azure.com/anthropic`, `ANTHROPIC_API_KEY=sk-test`, AND `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1` AND no other env vars SHALL be added by codebus

#### Scenario: Parent shell env is not modified

- **WHEN** the azure profile is active and `codebus query` runs to completion
- **THEN** the parent process's `ANTHROPIC_API_KEY` environment variable SHALL be observable from the parent shell as either unset OR retain its pre-invocation value

---
### Requirement: Config Subcommand For Keyring Management

The `codebus` binary SHALL provide a `config` subcommand that exposes three actions: `set-key <profile>`, `get-key <profile>`, and `delete-key <profile>`. The `<profile>` argument SHALL accept the literal value `azure` and SHALL reject all other values with a non-zero exit code. The `get-key` action SHALL accept an optional `--show` flag.

`codebus config set-key azure` SHALL read a key from stdin without echoing, write the value to the keyring entry `(azure.keyring_service, "default")`, and exit zero on success. If the keyring backend is unavailable, the command SHALL exit non-zero with a stderr message instructing the user to set `CODEBUS_AZURE_KEY` instead.

`codebus config get-key azure` SHALL print `set` if the keyring entry exists AND `unset` otherwise. When `--show` is passed AND the entry exists, the command SHALL print the key value verbatim.

`codebus config delete-key azure` SHALL remove the keyring entry if present AND SHALL exit zero whether or not the entry existed (idempotent).

#### Scenario: set-key stores the key

- **WHEN** the user runs `codebus config set-key azure` and enters `sk-test` on stdin
- **THEN** the keyring entry `(codebus-azure, default)` SHALL contain `sk-test` AND stdout SHALL contain `key stored` AND the command SHALL exit zero

#### Scenario: get-key reports unset without revealing absence detail

- **WHEN** the user runs `codebus config get-key azure` AND no keyring entry exists
- **THEN** stdout SHALL print `unset` AND the command SHALL exit zero

#### Scenario: get-key with --show prints the value

- **WHEN** the user runs `codebus config get-key azure --show` AND the keyring entry holds `sk-test`
- **THEN** stdout SHALL print `sk-test` AND the command SHALL exit zero

#### Scenario: delete-key is idempotent

- **WHEN** the user runs `codebus config delete-key azure` AND no keyring entry exists
- **THEN** the command SHALL exit zero

#### Scenario: Unknown profile argument rejected

- **WHEN** the user runs `codebus config set-key bedrock`
- **THEN** the command SHALL exit non-zero AND stderr SHALL contain a clap error message identifying `bedrock` as an invalid profile value