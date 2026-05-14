## MODIFIED Requirements

### Requirement: IPC Command Registry

The system SHALL expose exactly fifteen Tauri commands invokable from the frontend: `list_vaults`, `add_vault`, `remove_vault`, `load_global_config`, `save_global_config`, `set_endpoint_key`, `get_endpoint_key`, `delete_endpoint_key`, `check_cli_installed`, `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, `read_wiki_page`. No other Tauri commands SHALL be registered by this change. Each command SHALL have a stable name (snake_case), a typed argument shape, and a typed return shape mirroring the design contract.

The `check_cli_installed` command SHALL accept a `provider: String` argument whose only legal value is the literal `"claude_code"`. The command SHALL probe whether the agentic CLI binary for that provider is reachable by spawning `<binary> --version`. It SHALL return a `CliStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed { version }` and `not_installed`). Any spawn failure — binary missing, non-zero exit, empty stdout — SHALL collapse to `not_installed`; the underlying error SHALL NOT surface to the frontend. Future provider values (`codex`, `gemini_cli`, etc.) extend this match arm in a separate change.

The three keyring-management commands (`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`) SHALL accept a `profile: String` argument whose only legal value is the literal `"azure"`. Any other profile value SHALL reject the call with `AppError::Invalid { field: "profile", message: ... }`. The commands SHALL delegate to the codebus-core keyring helpers (`store_azure_key` / `probe_keyring_only` / `delete_azure_key`) — there is no separate keyring backend implementation in the app crate.

`set_endpoint_key` SHALL accept a `key: String` argument and store the value via the codebus-core helper. On success it SHALL return `Ok(())`. The key value SHALL NOT be cached anywhere in the app process beyond the Tauri command call boundary.

`get_endpoint_key` SHALL return a `KeyStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `set` and `unset`) reflecting only whether the keyring entry exists. The command SHALL NOT return the key value under any circumstance, including with any optional flag — verifying the key value SHALL require running the CLI verb (`codebus query "ping"`) instead.

`delete_endpoint_key` SHALL be idempotent: removing a non-existent entry SHALL return `Ok(())` rather than an error.

The six new commands `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, and `read_wiki_page` are defined normatively in the `app-workspace` capability (Tauri IPC Commands for Goal Lifecycle and Wiki Read requirement). Their argument shapes, return types, and error behavior live in that capability; this registry requirement only pins their existence and total count.

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

- **WHEN** the frontend attempts to invoke any command name other than the fifteen registered
- **THEN** Tauri returns a command-not-found error and the call rejects

#### Scenario: Help registry lists exactly fifteen commands

- **WHEN** the developer inspects the Tauri command registration in the app shell's `lib.rs`
- **THEN** exactly the fifteen named commands listed in this requirement are registered AND no additional ad-hoc commands appear

---

### Requirement: Workspace Stub Transition

When the user opens a vault from the Lobby (by clicking a card or completing a New Vault flow), the system SHALL transition the main view to the Workspace state for that vault. The Workspace SHALL render a left sidebar with the vault's display name, path, a `← Back to Lobby` control, and the three workspace tabs (`Goals`, `Wiki`, `Quiz`) defined by the `app-workspace` capability. The full layout, tab behavior, and workspace functionality are defined normatively in `app-workspace`; this requirement only pins the transition trigger and the back-to-lobby contract. (Historical note: in v3-app-foundation this requirement defined a placeholder "Workspace stub" that rendered a "coming soon" message; v3-app-workspace-goal replaces the stub body with the real Workspace while keeping the same transition contract — name retained for spec-history continuity.)

#### Scenario: Opening vault transitions to Workspace

- **WHEN** the user clicks a vault card or completes a New Vault flow
- **THEN** the main view transitions to the Workspace for that vault AND the Lobby content is no longer visible AND the Workspace renders the `Goals` tab as the default selection per `app-workspace`'s Workspace Layout requirement

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control in the Workspace
- **THEN** the main view returns to the Lobby in whichever state matches the current vault list AND any active goal run continues running in the background per `app-workspace`'s active-run lifecycle

---

### Requirement: Drag-Drop Scope Limited to Lobby

Folder drag-and-drop SHALL be accepted only while the application is in the Lobby state. In the Workspace state, drag-drop SHALL be either disabled or ignored, and SHALL NOT trigger the New Vault flow.

#### Scenario: Drop on Workspace is ignored

- **WHEN** a user drags a folder onto the application window while the Workspace is showing
- **THEN** no New Vault dialog appears, no `add_vault` is invoked, and the Workspace remains visible

---

### Requirement: Forbidden Behaviors in v1

The v1 codebus-app SHALL NOT include any of the following:

- Theme toggle or light-mode support (dark mode is hard-coded)
- Language switcher UI (locale auto-detected from system: `zh-*` → 中文, otherwise English)
- Vault-specific settings override UI in the Settings modal
- Multi-AI-provider selection UI
- Quest banner, progress bar, or any "graduated" / "mastered" / "learned" page-level state in the Lobby or Workspace
- Tutorial slideshow UI, embedded checkpoints, or tutorial md generation triggers
- Telemetry, analytics, crash reporting, or auto-update channels
- A "Recent Pages" panel inside any sidebar
- Graph view entry in any sidebar
- Chat-mode Cmd+K with conversation memory (the overlay itself is out of scope for this change; no precursor UI element SHALL be added)
- Direct LLM API calls from the frontend (all agent interaction goes through `codebus-core`)
- Multiple concurrently-active goal runs within a single vault session (per `app-workspace`'s One Active Goal Run At A Time requirement)

#### Scenario: Settings modal has no theme or language controls

- **WHEN** the user opens the Settings modal in any state
- **THEN** the rendered modal contains exactly the seven fields defined in "Global Settings Modal Field Set" and no theme or language controls

#### Scenario: No telemetry network calls

- **WHEN** the codebus-app launches and runs through any Lobby or Settings flow
- **THEN** no outbound network requests are made by the app shell itself (LLM/agent invocations remain the responsibility of `codebus-core` and are out of scope for this change)
