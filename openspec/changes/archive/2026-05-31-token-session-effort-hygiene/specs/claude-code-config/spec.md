## MODIFIED Requirements

### Requirement: Endpoint Profile Schema

The `~/.codebus/config.yaml` file SHALL accept an `agent` block with two top-level keys: `active_provider` and `providers`. `active_provider` SHALL be a string naming the active provider; the supported values SHALL be `claude` and `codex` (the `codex` provider's block shape is defined by the `codex-config` capability). An `active_provider` value outside this set SHALL be rejected with `ConfigLoadError::YamlParse`. `providers` SHALL be a map keyed by provider name. The `providers.claude` block SHALL contain three keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure` (the active endpoint profile for the claude provider). The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each verb sub-block SHALL contain a `model` field (a `SystemModel` enum value) and an optional `effort` field (an effort value constrained by the Effort Closed-Set Validation paragraph below). The `azure` block SHALL contain `base_url` (URL string), `keyring_service` (arbitrary string, default `codebus-azure`), and the same four verb sub-blocks (`goal`, `query`, `fix`, `verify`); in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string and `effort` SHALL be an optional effort value constrained by the Effort Closed-Set Validation paragraph below.

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

#### Scenario: Invalid SystemModel value rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.system.goal.model: gpt-4`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the invalid enum value

#### Scenario: Verb::Verify resolves to system.verify sub-block

- **WHEN** the system endpoint profile is active with `system.verify: { model: opus-4-6, effort: high }` and `system.query: { model: haiku-4-5, effort: low }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: opus-4-6` and `effort: high` AND SHALL NOT fall back to the `query` sub-block

#### Scenario: Verb::Verify resolves to azure.verify sub-block

- **WHEN** the azure endpoint profile is active with `azure.verify: { model: claude-opus-deploy, effort: high }`
- **THEN** `resolve(Verb::Verify)` SHALL return `model: claude-opus-deploy` and `effort: high` (azure deployment names pass through verbatim)

#### Scenario: Invalid effort in active system profile rejected

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.claude.active: system` and a complete `system` block whose `goal.effort` is `ultra`
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the `system.goal.effort` field as outside the allowed effort set `low / medium / high / xhigh / max / auto`

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
