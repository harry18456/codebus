## MODIFIED Requirements

### Requirement: IPC Command Registry

The system SHALL expose exactly nine Tauri commands invokable from the frontend: `list_vaults`, `add_vault`, `remove_vault`, `load_global_config`, `save_global_config`, `set_endpoint_key`, `get_endpoint_key`, `delete_endpoint_key`, `check_cli_installed`. No other Tauri commands SHALL be registered by this change. Each command SHALL have a stable name (snake_case), a typed argument shape, and a typed return shape mirroring the design contract.

The `check_cli_installed` command SHALL accept a `provider: String` argument whose only legal value is the literal `"claude_code"`. The command SHALL probe whether the agentic CLI binary for that provider is reachable by spawning `<binary> --version`. It SHALL return a `CliStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed { version }` and `not_installed`). Any spawn failure — binary missing, non-zero exit, empty stdout — SHALL collapse to `not_installed`; the underlying error SHALL NOT surface to the frontend. Future provider values (`codex`, `gemini_cli`, etc.) extend this match arm in a separate change.

The three keyring-management commands (`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`) SHALL accept a `profile: String` argument whose only legal value is the literal `"azure"`. Any other profile value SHALL reject the call with `AppError::Invalid { field: "profile", message: ... }`. The commands SHALL delegate to the codebus-core keyring helpers (`store_azure_key` / `probe_keyring_only` / `delete_azure_key`) — there is no separate keyring backend implementation in the app crate.

`set_endpoint_key` SHALL accept a `key: String` argument and store the value via the codebus-core helper. On success it SHALL return `Ok(())`. The key value SHALL NOT be cached anywhere in the app process beyond the Tauri command call boundary.

`get_endpoint_key` SHALL return a `KeyStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `set` and `unset`) reflecting only whether the keyring entry exists. The command SHALL NOT return the key value under any circumstance, including with any optional flag — verifying the key value SHALL require running the CLI verb (`codebus query "ping"`) instead.

`delete_endpoint_key` SHALL be idempotent: removing a non-existent entry SHALL return `Ok(())` rather than an error.

#### Scenario: Frontend invokes list_vaults

- **WHEN** the frontend calls `invoke("list_vaults")`
- **THEN** the command returns an array of `VaultEntry` objects, each with `path`, `display_name`, `last_opened`, and `is_missing` fields

#### Scenario: Frontend invokes set_endpoint_key with azure profile

- **WHEN** the frontend calls `invoke("set_endpoint_key", { profile: "azure", key: "sk-test" })`
- **THEN** the keyring entry `(<keyring_service from config>, "default")` SHALL contain `sk-test` AND the command SHALL return `Ok(())` AND no record of the key value SHALL persist in any in-memory app state beyond the call boundary

#### Scenario: Frontend invokes get_endpoint_key for an existing entry

- **WHEN** the frontend calls `invoke("get_endpoint_key", { profile: "azure" })` AND a keyring entry exists for the configured service
- **THEN** the command SHALL return `{ kind: "set" }` AND the response payload SHALL NOT contain the key value

#### Scenario: Frontend invokes get_endpoint_key with no entry present

- **WHEN** the frontend calls `invoke("get_endpoint_key", { profile: "azure" })` AND no keyring entry exists for the configured service
- **THEN** the command SHALL return `{ kind: "unset" }`

#### Scenario: Frontend invokes delete_endpoint_key when no entry exists

- **WHEN** the frontend calls `invoke("delete_endpoint_key", { profile: "azure" })` AND no keyring entry exists for the configured service
- **THEN** the command SHALL return `Ok(())` (idempotent)

#### Scenario: Unknown profile value rejected

- **WHEN** the frontend calls any of the three keyring commands with `profile: "bedrock"` (or any value other than `"azure"`)
- **THEN** the command SHALL reject with `AppError` having `kind: "invalid"`, `field: "profile"`, and a `message` naming the rejected value

#### Scenario: Config parse failure aborts keyring command

- **WHEN** the frontend calls any of the three keyring commands AND `~/.codebus/config.yaml` exists but fails to parse
- **THEN** the command SHALL reject with `AppError` having `kind: "config_parse"` AND a `message` naming the failing section AND the keyring entry SHALL NOT be touched (no `store_azure_key` / `probe_keyring_only` / `delete_azure_key` call performed)

#### Scenario: check_cli_installed returns installed status when binary is reachable

- **WHEN** the frontend calls `invoke("check_cli_installed", { provider: "claude_code" })` AND `claude --version` exits zero with non-empty stdout
- **THEN** the command SHALL return `{ kind: "installed", version: "<version-string>" }` where `<version-string>` is the trimmed stdout

#### Scenario: check_cli_installed returns not_installed on probe failure

- **WHEN** the frontend calls `invoke("check_cli_installed", { provider: "claude_code" })` AND the `claude` binary is not on PATH
- **THEN** the command SHALL return `{ kind: "not_installed" }` AND SHALL NOT surface the underlying spawn error

#### Scenario: check_cli_installed rejects unknown provider

- **WHEN** the frontend calls `invoke("check_cli_installed", { provider: "codex" })`
- **THEN** the command SHALL reject with `AppError` having `kind: "invalid"`, `field: "provider"`, and a `message` naming the rejected value

#### Scenario: Unregistered commands fail invocation

- **WHEN** the frontend attempts to invoke any command name other than the nine registered
- **THEN** Tauri returns a command-not-found error and the call rejects

---

### Requirement: AppError Discriminated Union

All nine IPC commands SHALL return errors as a single `AppError` enum serialized with `serde(tag = "kind", rename_all = "snake_case")`. The variants SHALL be: `io`, `config_parse`, `vault_not_found`, `vault_already_exists`, `invalid`, `internal`. The frontend SHALL be able to discriminate on the `kind` field to render appropriate UI (toast vs inline error vs dialog). Keyring backend failures (entry store / read / delete operation returns a non-missing error from the OS keyring) SHALL be surfaced as `AppError::Internal { message }` containing the underlying error description.

`save_global_config` SHALL additionally validate the `claude_code` block in the payload (if present) against the codebus-core endpoint schema BEFORE writing yaml to disk. When the validation fails — e.g. `active=azure` with an empty `base_url`, missing required `keyring_service`, or empty deployment-name model — the command SHALL reject the call with `AppError::Invalid { field: "claude_code", message: <parse-error-detail> }`, AND the yaml file SHALL remain unchanged on disk. This prevents the GUI from producing a yaml that the CLI's fail-loud loader would reject on next read.

#### Scenario: Vault path missing returns vault_not_found

- **WHEN** `add_vault` is invoked with a path that does not exist on disk
- **THEN** the command rejects with `AppError` having `kind: "vault_not_found"` and a `path` field containing the offered path

#### Scenario: Invalid threshold returns invalid with field name

- **WHEN** `save_global_config` is invoked with `app.quiz.pass_threshold` outside the 50–100 range
- **THEN** the command rejects with `AppError` having `kind: "invalid"`, `field: "app.quiz.pass_threshold"`, and a descriptive `message`

#### Scenario: Keyring backend unavailable surfaces as internal error

- **WHEN** the frontend calls `set_endpoint_key` AND the OS keyring backend is not reachable (e.g. headless Linux session without Secret Service)
- **THEN** the command rejects with `AppError` having `kind: "internal"` AND a `message` mentioning keyring backend unavailability

#### Scenario: save_global_config rejects incomplete azure profile

- **WHEN** the frontend calls `invoke("save_global_config", { config })` where `config.claude_code.active` is `"azure"` AND `config.claude_code.azure.base_url` is empty
- **THEN** the command SHALL reject with `AppError` having `kind: "invalid"`, `field: "claude_code"`, and a `message` describing the parse error AND the on-disk yaml file SHALL remain unchanged (or never created on first save)

## ADDED Requirements

### Requirement: Settings UI CLI Status Field

The Settings modal SHALL render a CLI Status row that probes whether the agentic CLI binary for each supported provider is installed. The row SHALL invoke `check_cli_installed` on modal open AND SHALL display one of three states: `Checking…` (probe in flight), `Installed · <version>` (success), or `Not installed` (any failure). When the state is `Not installed`, the row SHALL render an inline hint instructing the user to install the CLI before configuring the endpoint. The row SHALL replace the prior `Authentication` / OAuth-status pseudo-field — the v1 OAuth label was a placeholder that did not reflect any real auth state.

#### Scenario: CLI status row shows Installed after a successful probe

- **WHEN** the user opens the Settings modal AND `check_cli_installed("claude_code")` returns `{ kind: "installed", version: "2.1.139 (Claude Code)" }`
- **THEN** the Settings UI SHALL display a status badge containing the text `Installed` AND the version string `2.1.139 (Claude Code)`

#### Scenario: CLI status row shows Not installed when probe fails

- **WHEN** the user opens the Settings modal AND `check_cli_installed("claude_code")` returns `{ kind: "not_installed" }`
- **THEN** the Settings UI SHALL display a status badge containing the text `Not installed` AND an inline hint instructing the user to install the CLI

---

### Requirement: Settings UI Endpoint Section

The Settings modal SHALL render an Endpoint section that lets the user configure the Claude Code endpoint profile schema and manage the Azure API key entirely from the GUI. The section heading SHALL read `Claude Code endpoint settings` (or the locale-specific translation) and SHALL NOT include a provider-selector control — single-implementation selectors are out of scope until a second provider is integrated.

The section SHALL contain three controls plus two sub-sections:

1. An `active` radio group with exactly two options, `system` and `azure`. Selecting an option SHALL mutate the in-memory `claude_code.active` field; it SHALL NOT clear any field values in the non-selected profile sub-section.
2. A System Profile sub-section containing three verb rows (`goal` / `query` / `fix`). Each row SHALL contain a `model` `<select>` with exactly four `<option>` values (`opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6`) AND a free-text `effort` input.
3. An Azure Profile sub-section containing: a `base_url` text input, a `keyring_service` text input (pre-filled with the default `codebus-azure` when the underlying config field is empty or absent), an API-key status indicator (`Set` / `Unset`) with `Set new...` and `Delete` action buttons, AND three verb rows each containing a free-text `model` (deployment name) input AND a free-text `effort` input.

Both profile sub-sections SHALL be present in the DOM regardless of the `active` value, organised as an **accordion**: the sub-section whose name matches `active` SHALL be expanded; the other sub-section SHALL be collapsed showing only its header (which SHALL include an `(inactive)` label). The user SHALL be able to click the collapsed header to expand the inactive sub-section and edit cold-storage configuration. When the user toggles the `active` radio, the newly-active sub-section SHALL auto-expand AND the previously-active sub-section SHALL auto-collapse; this auto-folding SHALL NOT delete or reset any form input values (collapsed inputs remain in the DOM, hidden via CSS, so values persist).

The `Save` button at the Settings modal level SHALL persist only yaml content via `save_global_config`; it SHALL NOT carry the Azure API key. The API key SHALL flow exclusively through the three `*_endpoint_key` IPC commands triggered by the `Set new...` / `Delete` action buttons.

The Settings UI SHALL NOT include a Test Connection / endpoint reachability button — verification SHALL require running `codebus query "ping"` from the terminal.

The Settings modal SHALL perform client-side validation of the `claude_code` block before allowing the user to save. Specifically: when `active === "azure"`, all of `base_url`, `keyring_service`, AND each verb's `model` (deployment name) SHALL be non-empty strings (trimmed). When any of these are empty, the modal SHALL disable the Save button AND SHALL render an inline validation summary listing each failing field AND SHALL apply `aria-invalid="true"` to the offending inputs. The validation rules SHALL match the codebus-core `Endpoint Profile Schema` validation so the frontend and `save_global_config` backend gate produce the same reject/accept decision.

#### Scenario: Save button is disabled when active=azure has empty required fields

- **WHEN** `claude_code.active === "azure"` AND `claude_code.azure.base_url` is the empty string (or any required azure field is empty) AND the user has edited any setting (dirty)
- **THEN** the Save button SHALL be disabled AND the Endpoint section SHALL render an inline validation summary listing the failing fields

#### Scenario: Empty azure field gets aria-invalid

- **WHEN** `claude_code.active === "azure"` AND `claude_code.azure.goal.model` is the empty string
- **THEN** the `azure-deployment-goal` input SHALL have `aria-invalid="true"` AND the validation summary SHALL list `claude_code.azure.goal.model`

#### Scenario: Save button enables when active=azure becomes fully populated

- **WHEN** `claude_code.active === "azure"` AND all azure required fields are non-empty AND the user has edited any setting (dirty)
- **THEN** the Save button SHALL be enabled AND the Endpoint section SHALL NOT render a validation summary

#### Scenario: Active radio switch preserves non-active profile inputs

- **WHEN** the user has typed `https://example.com/anthropic` into the azure `base_url` input AND `active` is currently `system` AND the user toggles `active` to `azure` then back to `system`
- **THEN** the azure `base_url` input SHALL still contain `https://example.com/anthropic` (the value SHALL NOT be cleared by the toggle) — even though auto-fold collapses the azure sub-section back to a header when `active` returns to `system`

#### Scenario: Initial render collapses the non-active sub-section

- **WHEN** the Endpoint section first renders AND `claude_code.active` is `system`
- **THEN** the System Profile sub-section SHALL be expanded (its verb rows and inputs visible) AND the Azure Profile sub-section SHALL be collapsed (only its header with `(inactive)` label visible)

#### Scenario: User can expand inactive sub-section to edit cold storage

- **WHEN** `active` is `system` AND the user clicks the Azure Profile collapsed header
- **THEN** the Azure Profile sub-section SHALL expand revealing its inputs AND the System Profile sub-section SHALL remain expanded (the user-driven expansion of the inactive sub-section SHALL NOT collapse the active one)

#### Scenario: Toggling active auto-collapses the previously-active sub-section

- **WHEN** the System Profile is expanded (active) AND the user toggles `active` to `azure`
- **THEN** the Azure Profile SHALL expand AND the System Profile SHALL collapse to its header — but the System Profile verb model dropdowns and effort inputs SHALL remain in the DOM (hidden via CSS) so their values persist across toggles

#### Scenario: System model dropdown lists exactly four versioned options

- **WHEN** the System Profile sub-section is rendered AND the user opens any of the three verb `model` dropdowns
- **THEN** the dropdown SHALL list exactly four options whose `value` attributes are `opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6` in that order

#### Scenario: Azure keyring_service input is pre-filled when config field is empty

- **WHEN** `~/.codebus/config.yaml` either does not exist OR exists with `claude_code.azure.keyring_service` empty / absent AND the user opens the Settings modal
- **THEN** the Azure `keyring_service` input SHALL display the value `codebus-azure`

#### Scenario: Set new... button opens key entry modal

- **WHEN** the user clicks the `Set new...` button in the Azure Profile sub-section
- **THEN** a modal SHALL open containing a password-masked `<input type="password">` AND a `Confirm` button AND a `Cancel` button

#### Scenario: Confirming the key entry modal stores the key without persisting it client-side

- **WHEN** the user enters `sk-modal-test` into the password input AND clicks `Confirm`
- **THEN** the modal SHALL invoke `set_endpoint_key("azure", "sk-modal-test")` AND on success the modal SHALL close AND the API-key status indicator SHALL update to `Set` AND no DOM element OR app state SHALL retain the entered key value

#### Scenario: Delete button removes the keyring entry and updates status

- **WHEN** the API-key status indicator currently shows `Set` AND the user clicks the `Delete` button
- **THEN** the UI SHALL invoke `delete_endpoint_key("azure")` AND on success the status indicator SHALL update to `Unset`

#### Scenario: Save button does not transmit the API key

- **WHEN** the user has made any edits to the System / Azure profile fields AND clicks the `Save` button
- **THEN** the resulting `save_global_config` payload SHALL contain the edited `claude_code` block AND SHALL NOT contain any key, field, or string value matching the Azure API key value
