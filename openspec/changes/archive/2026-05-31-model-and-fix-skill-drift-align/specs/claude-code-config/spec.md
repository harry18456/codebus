## MODIFIED Requirements

### Requirement: Endpoint Profile Schema

The `~/.codebus/config.yaml` file SHALL accept an `agent` block with two top-level keys: `active_provider` and `providers`. `active_provider` SHALL be a string naming the active provider; the supported values SHALL be `claude` and `codex` (the `codex` provider's block shape is defined by the `codex-config` capability). An `active_provider` value outside this set SHALL be rejected with `ConfigLoadError::YamlParse`. `providers` SHALL be a map keyed by provider name. The `providers.claude` block SHALL contain three keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure` (the active endpoint profile for the claude provider). The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each verb sub-block SHALL contain a `model` field (a free-form string alias — see the `System Profile Model Aliases` requirement, NOT a closed enum) and an optional `effort` field (an effort value constrained by the Effort Closed-Set Validation paragraph below). The `azure` block SHALL contain `base_url` (URL string), `keyring_service` (arbitrary string, default `codebus-azure`), and the same four verb sub-blocks (`goal`, `query`, `fix`, `verify`); in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string and `effort` SHALL be an optional effort value constrained by the Effort Closed-Set Validation paragraph below.

The endpoint profile referenced by `providers.claude.active` MUST be fully populated for the load to succeed — all four verb sub-blocks (`goal`, `query`, `fix`, `verify`) are required when the profile is active. The non-active endpoint profile MAY be absent or partially populated; codebus SHALL NOT validate fields of the non-active profile. If the `agent` block is absent entirely, the system SHALL fall back to a built-in default equivalent to `active_provider: claude` with `providers.claude.active: system` and verb defaults `goal: opus-4-6` / `query: haiku-4-5` / `fix: sonnet-4-6` / `verify: opus-4-6` and per-verb default `effort` values `high` / `low` / `medium` / `high` respectively.

**Effort Closed-Set Validation**: The `effort` value of each verb sub-block in the ACTIVE endpoint profile (`goal`, `query`, `fix`, `verify`) SHALL be one of the closed set `low`, `medium`, `high`, `xhigh`, `max`. This set SHALL be exactly the value set the Claude CLI `--effort <level>` flag accepts (confirmed via `claude --help`: `low, medium, high, xhigh, max`); `auto` is NOT a member, because the Claude CLI rejects `--effort auto` and a configured `effort: auto` therefore fails the spawn. A value outside this closed set (including `auto`) SHALL be rejected at load with `ConfigLoadError::YamlParse` identifying the offending field path and the allowed set. This validation SHALL apply to the active profile only, consistent with the rule that codebus SHALL NOT validate fields of the non-active profile: an out-of-set `effort` in a cold-storage (non-active) profile SHALL NOT block the load. This closed set SHALL constrain the claude provider's `effort` field only; the `model` field SHALL remain unconstrained by this requirement (no `model` closed-set is introduced or changed here), and the codex provider's effort is governed by the `codex-config` capability. The Settings UI `SYSTEM_EFFORTS` dropdown SHALL surface exactly this same five-value set (no `auto`), so the GUI cannot offer a value the loader and CLI reject.

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

#### Scenario: Arbitrary system model string loads

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` and a complete `system` block whose `goal.model` is an arbitrary string such as `gpt-4` or a newly-released alias such as `opus-4-8`
- **THEN** the config loader SHALL accept the value and return a config with `system.goal.model` preserved verbatim (the system `model` field is a free string, not a closed enum, so no value is rejected on the basis of an enum membership check)

#### Scenario: Empty azure model rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: azure` and a complete `azure` block whose `goal.model` is the empty string
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `azure.goal.model` field as required and non-empty (the azure `model` rule is non-emptiness, not enum membership)

#### Scenario: Verb::Verify resolves to system.verify sub-block

- **WHEN** the system endpoint profile is active with `system.verify: { model: opus-4-6, effort: high }` and `system.query: { model: haiku-4-5, effort: low }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: opus-4-6` and `effort: high` AND SHALL NOT fall back to the `query` sub-block

#### Scenario: Verb::Verify resolves to azure.verify sub-block

- **WHEN** the azure endpoint profile is active with `azure.verify: { model: claude-opus-deploy, effort: high }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: claude-opus-deploy` and `effort: high` (azure deployment names pass through verbatim)

#### Scenario: Invalid effort in active system profile rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` and a complete `system` block whose `goal.effort` is `ultra`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `system.goal.effort` field as outside the allowed effort set `low / medium / high / xhigh / max`

#### Scenario: Invalid effort in active azure profile rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: azure` and a complete `azure` block whose `fix.effort` is `ultra`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `azure.fix.effort` field as outside the allowed effort set

#### Scenario: All five effort values load successfully

- **WHEN** `~/.codebus/config.yaml` contains an active `system` block whose four verbs carry efforts drawn from the closed set `low`, `medium`, `high`, `xhigh`, `max`
- **THEN** the config loader SHALL return a config with the system endpoint profile selected AND SHALL NOT reject any of the five values

#### Scenario: effort auto is rejected as an invalid effort value

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` and a complete `system` block whose `query.effort` is `auto`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `system.query.effort` field as outside the allowed effort set `low / medium / high / xhigh / max` (the Claude CLI does not accept `--effort auto`)

#### Scenario: Out-of-set effort in non-active profile does not block load

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` with a complete valid `system` block AND a fully-populated cold-storage `azure` block whose `goal.effort` is `ultra`
- **THEN** the config loader SHALL return a config with the system endpoint profile selected AND SHALL NOT fail due to the out-of-set effort in the non-active azure profile

<!-- @trace
source: model-and-fix-skill-drift-align
updated: 2026-05-31
code:
  - codebus-core/src/log/mod.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/agent/claude_cli.rs
  - codebus-core/src/agent/backend.rs
  - codebus-core/src/log/sink.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/config/endpoint.rs
  - docs/BACKLOG.md
tests:
  - codebus-app/src/lib/ipc.effort.test.ts
  - codebus-core/tests/endpoint_config_load.rs
  - codebus-app/src/components/settings/EndpointSection.test.tsx
-->

---
### Requirement: System Profile Model Aliases

The system-profile `model` field SHALL be a free-form string alias, NOT a closed enum: codebus SHALL NOT reject a system `model` value at config load on the basis of any closed set, and an arbitrary alias string SHALL load and be preserved verbatim. Codebus SHALL maintain a deterministic mapping `system_model_to_cli_flag` from the system `model` string to the value passed to the Claude CLI's `--model` flag. The mapping rule SHALL be uniform: a bare alias (`opus-4-7`, `haiku-4-5`, or any newly-released model) SHALL be prefixed with `claude-` (e.g. `opus-4-7` becomes `claude-opus-4-7`); a value already carrying the `claude-` prefix (or any full model id) SHALL pass through verbatim; an empty string SHALL map to an empty flag (the caller decides whether that is valid). Because the rule applies to any value, a newly-released Claude model SHALL require no codebus code change (forward-compatible). The mapping SHALL be applied immediately before spawning the child process.

#### Scenario: opus-4-6 alias resolves to claude-opus-4-6

- **WHEN** the system endpoint profile is active and the goal verb's `model` is `opus-4-6`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-opus-4-6`

#### Scenario: haiku-4-5 alias resolves to claude-haiku-4-5

- **WHEN** the system endpoint profile is active and the query verb's `model` is `haiku-4-5`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-haiku-4-5`

#### Scenario: Unversioned alias loads and gets the claude- prefix

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.system.query.model: haiku` (without version suffix)
- **THEN** the config loader SHALL accept the value AND the spawned `claude` child process SHALL receive the argument pair `--model claude-haiku` (the system `model` is a free string, so an unversioned alias is NOT rejected — it is prefixed like any other bare alias)

#### Scenario: Future model alias needs no code change

- **WHEN** the system endpoint profile is active and the goal verb's `model` is a newly-released alias such as `opus-4-8` not previously known to codebus
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-opus-4-8` without any codebus code change

##### Example: alias mapping table (illustrative, non-exhaustive)

The mapping rule applies to any value; the table below lists currently-common aliases only and is NOT a closed set.

| system `model` alias | `--model` flag value passed to claude CLI   |
| -------------------- | ------------------------------------------- |
| `opus-4-7`           | `claude-opus-4-7`                           |
| `opus-4-6`           | `claude-opus-4-6`                           |
| `haiku-4-5`          | `claude-haiku-4-5`                          |
| `sonnet-4-6`         | `claude-sonnet-4-6`                         |
| `claude-opus-4-7`    | `claude-opus-4-7` (passes through verbatim) |

<!-- @trace
source: model-and-fix-skill-drift-align
updated: 2026-05-31
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

When the azure profile is active, codebus SHALL pass each verb's `model` field verbatim to the Claude CLI's `--model` flag. Codebus SHALL NOT validate, translate, or rewrite the string under any circumstance. The string SHALL be treated as an Azure deployment name even when its value matches a system-style alias literal (e.g. `opus-4-6`).

#### Scenario: Azure deployment name passes through

- **WHEN** the azure profile is active and the goal verb's `model` is `claude-opus-4-6-2026V2`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model claude-opus-4-6-2026V2`

#### Scenario: Azure mode does not translate system-style alias

- **WHEN** the azure profile is active and the goal verb's `model` is `opus-4-6`
- **THEN** the spawned `claude` child process SHALL receive the argument pair `--model opus-4-6` AND codebus SHALL NOT translate the value to `claude-opus-4-6`
