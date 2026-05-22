# codex-config Specification

## Purpose

TBD - created by archiving change 'codex-backend'. Update Purpose after archive.

## Requirements

### Requirement: Codex Provider Config Schema

The `~/.codebus/config.yaml` `agent.providers` map SHALL accept a `codex` provider block independent of the `claude` provider block, so codex and claude endpoint settings are configured separately even when they share an API key. The `providers.codex` block SHALL contain three keys: `active`, `system`, and `azure`. The `active` key SHALL be a string with exactly one of two values: `system` or `azure` (the active endpoint profile for the codex provider).

The `system` block SHALL contain four verb sub-blocks named `goal`, `query`, `fix`, and `verify`; each sub-block SHALL contain a `model` field that is an arbitrary non-empty string (a codex model name such as `gpt-5.5`; codex model names are NOT a closed enum, so the loader SHALL NOT reject unknown model strings) and an optional `effort` field (an arbitrary string forwarded as `model_reasoning_effort`).

The `azure` block SHALL contain `base_url` (URL string of the Azure OpenAI resource, e.g. ending in `/openai`), `api_version` (string, e.g. `2025-04-01-preview`), `keyring_service` (arbitrary string, default `codebus-azure` so the Azure key MAY be shared with the claude provider while keeping config separate), and the same four verb sub-blocks; in the `azure` block each verb's `model` field SHALL be an arbitrary non-empty string (the Azure deployment name, e.g. `gpt-5.4`, passed verbatim) and `effort` SHALL be an optional arbitrary string.

The endpoint profile referenced by `providers.codex.active` MUST be fully populated for the load to succeed — all four verb sub-blocks (`goal`, `query`, `fix`, `verify`) are required when the profile is active. The non-active endpoint profile MAY be absent or partially populated; codebus SHALL NOT validate fields of the non-active profile. The Azure API key SHALL be read from the keyring service named by `keyring_service` using the same keyring-then-env fallback chain as the claude provider.

#### Scenario: Codex system profile loads with arbitrary model strings

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.codex` with `active: system` and a `system` block whose four verbs carry `model: gpt-5.5` (and other codex model strings) plus `effort` values
- **THEN** the config loader SHALL return the codex provider with the system profile and the verb settings parsed verbatim, without rejecting the non-enum model strings

#### Scenario: Codex azure profile loads with Responses API fields

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.codex` with `active: azure` and an `azure` block with `base_url`, `api_version`, `keyring_service`, and all four verbs (each carrying a deployment-name `model`) populated
- **THEN** the config loader SHALL return the codex provider with the azure profile, exposing `base_url`, `api_version`, `keyring_service`, and the verbatim deployment-name models

#### Scenario: Active codex profile missing a verb fails

- **WHEN** `~/.codebus/config.yaml` contains `agent.providers.codex.active: system` and a `system` block with `goal`, `query`, `fix` populated but `verify` absent
- **THEN** the config loader SHALL return `ConfigLoadError::YamlParse` identifying the missing `system.verify` sub-block AND SHALL NOT silently fall back to defaults

#### Scenario: Codex azure keyring service defaults to the shared claude key

- **WHEN** `agent.providers.codex.azure` omits `keyring_service`
- **THEN** the loader SHALL default `keyring_service` to `codebus-azure`, allowing the codex provider to read the same Azure key as the claude provider while remaining a separate config block

<!-- @trace
source: codex-backend
updated: 2026-05-23
code:
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/quiz.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/store/chat.ts
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/verb/chat.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/mod.rs
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-core/src/verb/error.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-app/src/store/goals.ts
  - codebus-core/src/verb/fix.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-core/src/agent/dispatch.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/config/endpoint.rs
tests:
  - codebus-app/src/store/chat.test.ts
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->