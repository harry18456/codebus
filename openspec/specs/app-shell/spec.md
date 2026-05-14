# app-shell Specification

## Purpose

TBD - created by archiving change 'v3-app-foundation'. Update Purpose after archive.

## Requirements

### Requirement: Tauri Shell Runtime

The codebus-app SHALL launch as a Tauri v2 desktop application binding `codebus-core` as a direct Rust dependency (no separate process spawn for core access). The main window SHALL open at 1280×800 with a minimum size of 960×640, MUST be resizable, and the application SHALL be single-instance — a second launch SHALL focus the existing window instead of opening a new one. The Tauri Rust source SHALL live under `codebus-app/src-tauri/`; the frontend (Vite + React + TypeScript) SHALL live under `codebus-app/` root with its own `package.json`.

#### Scenario: First launch creates the main window

- **WHEN** the user launches the codebus-app binary for the first time
- **THEN** a single Tauri window opens at 1280×800, resizable, with no system browser chrome

#### Scenario: Second launch focuses existing window

- **WHEN** the codebus-app is already running and the user invokes the binary again
- **THEN** the existing window receives focus and no second window is created

#### Scenario: Window respects minimum size

- **WHEN** the user attempts to resize the window below 960 width or 640 height
- **THEN** the window stops shrinking at 960×640

---
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


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

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

---
### Requirement: App-State Persistence

The system SHALL persist app-level state to `~/.codebus/app-state.json`. The file SHALL include a top-level `schema_version: 1` field and a top-level `vault_list` array. Each `vault_list` entry SHALL contain an absolute `path` string, a `display_name` string, and a `last_opened` ISO 8601 UTC timestamp string. The CLI SHALL NOT read or write `app-state.json`.

On startup, if the file does not exist, the system SHALL create it with `{ "schema_version": 1, "vault_list": [] }`. On startup, if the file fails to parse OR if `schema_version` exceeds the current supported value, the system SHALL log a warning and proceed with an empty in-memory `vault_list` without overwriting the file.

#### Scenario: First launch creates empty state file

- **WHEN** the codebus-app launches and `~/.codebus/app-state.json` does not exist
- **THEN** the system creates the file containing `{ "schema_version": 1, "vault_list": [] }`

#### Scenario: Corrupt state falls back to empty without overwrite

- **WHEN** the codebus-app launches and `~/.codebus/app-state.json` contains invalid JSON
- **THEN** the system logs a parse warning, the in-memory vault list is empty, and the on-disk file is not modified

##### Example: schema version mismatch

- **GIVEN** `~/.codebus/app-state.json` contains `{ "schema_version": 99, "vault_list": [...] }`
- **WHEN** the codebus-app launches
- **THEN** the system logs a `schema_version unsupported` warning, the in-memory vault list is empty, and the on-disk file is preserved

---
### Requirement: Vault List Lifecycle

The Lobby vault list SHALL derive only from `~/.codebus/app-state.json` and SHALL NOT be derived from filesystem scanning. When the app loads the vault list, each entry's `path` SHALL be verified to exist on disk; entries whose path is missing or unreadable SHALL be marked `is_missing: true` in the returned `VaultEntry` for the frontend to display a missing badge. Removing a vault from the list SHALL only unbind the entry from `app-state.json` and SHALL NOT delete the on-disk `.codebus/` directory.

#### Scenario: Missing path surfaces as is_missing

- **WHEN** `list_vaults` is invoked and a stored vault path no longer exists on disk
- **THEN** the returned `VaultEntry` for that path has `is_missing: true` and the corresponding card in the Lobby displays a missing badge

#### Scenario: Remove unbinds without deletion

- **WHEN** the user invokes `remove_vault` on an existing vault entry
- **THEN** the entry is removed from `app-state.json` and the on-disk `.codebus/` directory at that path is unchanged

---
### Requirement: Lobby Two-State Rendering

The Lobby SHALL render in exactly one of two states determined by `vault_list` length. The populated state SHALL display vault cards with `display_name`, `path`, and human-readable relative `last_opened` (absolute date after 30 days), plus a top-right `+ New Vault` button. The empty state SHALL display a hero with a large 🚌 emoji, the title "來搭第一台公車吧" or "Board your first bus" (based on system locale `zh-*` vs other), a subtitle, a primary `+ Board a new bus` button, and a Quick start 3-step orientation card. Both states SHALL show a bottom strip containing a Settings gear (left) and a version label (right).

#### Scenario: Empty list renders empty state

- **WHEN** the Lobby loads and `vault_list` is empty
- **THEN** the empty-state hero (🚌 emoji, title, subtitle, Board-a-new-bus button, Quick start card) is rendered and no vault cards are shown

#### Scenario: Non-empty list renders cards

- **WHEN** the Lobby loads and `vault_list` contains one or more entries
- **THEN** vault cards are rendered in reverse-chronological order by `last_opened`, and the top-right `+ New Vault` button is shown

---
### Requirement: New Vault Flow Detection Branches

The system SHALL provide three entry points to add a vault: the `+ New Vault` button (top-right of populated Lobby or center of empty state), the `Cmd+N` / `Ctrl+N` keyboard shortcut (Lobby state only), and folder drag-and-drop on the Lobby window. All three entry points SHALL converge on the same detection step.

The detection step SHALL examine the selected folder for an existing `.codebus/` directory and branch as follows:

- **No `.codebus/`**: the system SHALL invoke the codebus-core init equivalent silently, add the vault to `app-state.json`, and transition to the Workspace state for that vault.
- **Existing `.codebus/`**: the system SHALL present a dialog with two options — "Just bind it to Lobby (recommended)" and "Re-initialize (destructive)". Just-Bind SHALL add the vault to `app-state.json` without modifying any vault data. Re-initialize SHALL require the user to type the literal word `delete` to confirm, then SHALL delete the existing `.codebus/` and run a fresh init.

#### Scenario: Folder without .codebus initializes silently

- **WHEN** the user picks (or drops) a folder that has no `.codebus/` subdirectory
- **THEN** the system runs codebus-core init on that folder, adds the vault to `app-state.json`, and transitions to the Workspace state for that vault

#### Scenario: Folder with .codebus offers Just-Bind

- **WHEN** the user picks (or drops) a folder that already contains `.codebus/`
- **THEN** a dialog appears with Just-Bind (selected by default) and Re-initialize options, and Cancel returns the user to the Lobby with no state change

#### Scenario: Re-initialize requires typed confirmation

- **WHEN** the user picks the Re-initialize option in the detection dialog
- **THEN** an inner confirmation step requires the user to type the literal `delete` before the destructive action proceeds; closing or cancelling at this step leaves the existing `.codebus/` untouched

---
### Requirement: Drag-Drop Scope Limited to Lobby

Folder drag-and-drop SHALL be accepted only while the application is in the Lobby state. In the Workspace state, drag-drop SHALL be either disabled or ignored, and SHALL NOT trigger the New Vault flow.

#### Scenario: Drop on Workspace is ignored

- **WHEN** a user drags a folder onto the application window while the Workspace is showing
- **THEN** no New Vault dialog appears, no `add_vault` is invoked, and the Workspace remains visible


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Global Settings Modal Field Set

The Settings modal SHALL be invoked by the bottom-left gear in either Lobby or Workspace state. The modal SHALL display exactly seven editable fields in this order:

1. AI Provider (read-only label: "Claude CLI (only option for now)")
2. Authentication (OAuth status label + Re-authenticate link button)
3. Default model per verb (three dropdowns: goal, query, fix)
4. PII scanner (dropdown showing scanner name and dynamic pattern count, e.g. `regex_basic · 14 patterns`)
5. Log sink (path display + Change folder link)
6. Quiz pass threshold (slider 50–100%, displayed value with `%` unit suffix)
7. Default quiz length (slider 3–10, displayed value with `questions` unit suffix)

No additional fields SHALL be present in v1 (no theme toggle, no language switcher, no per-vault override section). Sub-labels under fields SHALL NOT promise features absent from v1 (e.g., the Default model sub-label MUST NOT say "overridden per goal" or similar that implies a non-existent override UI).

#### Scenario: Modal opens from Lobby gear

- **WHEN** the user clicks the bottom-left gear in the Lobby
- **THEN** the Settings modal opens centered over a dimmed Lobby background

#### Scenario: PII pattern count is dynamic

- **WHEN** the Settings modal renders the PII scanner field
- **THEN** the displayed pattern count is read at runtime from the active scanner registry (not hard-coded in the UI source)

#### Scenario: Save persists atomically

- **WHEN** the user changes any field and clicks Save
- **THEN** the system writes `~/.codebus/config.yaml` atomically (temporary file then rename), closes the modal, and shows a "Saved" toast

##### Example: Quiz pass threshold round-trip

- **GIVEN** `~/.codebus/config.yaml` has `app.quiz.pass_threshold: 80`
- **WHEN** the user opens Settings, changes the threshold slider to 70, and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `app.quiz.pass_threshold: 70` after save, and reopening Settings shows the slider at 70

---
### Requirement: AppConfig Namespace Isolation

The system SHALL introduce an `app.*` namespace inside `~/.codebus/config.yaml`. The namespace SHALL contain `app.quiz.pass_threshold` (integer, 50–100, default 80) and `app.quiz.default_length` (integer, 3–10, default 5). The codebus CLI binaries (`init`, `goal`, `query`, `lint`, `fix`) SHALL NOT read, write, or otherwise depend on the `app.*` namespace.

#### Scenario: CLI ignores app namespace

- **WHEN** any codebus CLI verb runs against a `~/.codebus/config.yaml` containing the `app.*` namespace
- **THEN** the CLI executes normally with no warnings about `app.*` and no modification to `app.*` values

#### Scenario: App reads app namespace defaults

- **WHEN** the app loads global config and `app.quiz.pass_threshold` is absent from the YAML
- **THEN** the loaded `GlobalConfig` returns `app.quiz.pass_threshold = 80` (default) and `app.quiz.default_length = 5` (default)

---
### Requirement: Workspace Stub Transition

When the user opens a vault from the Lobby (by clicking a card or completing a New Vault flow), the system SHALL transition the main view to the Workspace state for that vault. The Workspace SHALL render a left sidebar with the vault's display name, path, a `← Back to Lobby` control, and the three workspace tabs (`Goals`, `Wiki`, `Quiz`) defined by the `app-workspace` capability. The full layout, tab behavior, and workspace functionality are defined normatively in `app-workspace`; this requirement only pins the transition trigger and the back-to-lobby contract. (Historical note: in v3-app-foundation this requirement defined a placeholder "Workspace stub" that rendered a "coming soon" message; v3-app-workspace-goal replaces the stub body with the real Workspace while keeping the same transition contract — name retained for spec-history continuity.)

#### Scenario: Opening vault transitions to Workspace

- **WHEN** the user clicks a vault card or completes a New Vault flow
- **THEN** the main view transitions to the Workspace for that vault AND the Lobby content is no longer visible AND the Workspace renders the `Goals` tab as the default selection per `app-workspace`'s Workspace Layout requirement

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control in the Workspace
- **THEN** the main view returns to the Lobby in whichever state matches the current vault list AND any active goal run continues running in the background per `app-workspace`'s active-run lifecycle


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
### Requirement: Design Token Translation to Tailwind v4 Theme

The system SHALL translate the canonical design tokens from `codebus-app/design-handoff/README.md` ("Design Tokens (canonical)" section) into a Tailwind v4 `@theme` declaration colocated in `codebus-app/src/styles/tokens.css`. The following token categories SHALL be defined: background tints (bg, bg-raised, bg-hover, bg-active, bg-sunken), borders (border, border-strong, border-subtle), text tints (fg, fg-secondary, fg-tertiary, fg-quaternary), the single `accent` (amber #F5A623) plus its hover/dim/fg/tint/ring variants, semantic colors (success, warn, error, info), and the radii (sm, md, lg) plus 8px for modal cards. No additional accent color SHALL be introduced.

#### Scenario: Tailwind theme contains the canonical accent

- **WHEN** the developer inspects `codebus-app/src/styles/tokens.css`
- **THEN** the file declares `--accent: #f5a623` (or its Tailwind v4 `@theme` equivalent) and no other accent color is declared

#### Scenario: No second accent color is introduced

- **WHEN** any component in `codebus-app/src/` uses a non-grayscale color
- **THEN** that color resolves through the declared `accent` / `success` / `warn` / `error` / `info` token only, and is not a one-off hex literal

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


<!-- @trace
source: v3-app-workspace-goal
updated: 2026-05-14
code:
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/render/banner.rs
  - codebus-app/src-tauri/gen/schemas/acl-manifests.json
  - codebus-app/src/components/LoadingOverlay.tsx
  - codebus-app/src/components/workspace/WikiTree.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/2026-05-14-skill-bundles-vault-only-backlog.md
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/capabilities/default.json
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/store/route.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - codebus-app/src/components/workspace/RunDetailRunning.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/lib/milkdown-wikilink.tsx
  - codebus-app/src-tauri/src/state/app_state.rs
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.tsx
  - codebus-cli/src/commands/init.rs
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/store/goals.ts
  - codebus-app/src-tauri/gen/schemas/desktop-schema.json
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/package.json
  - codebus-app/src/store/wiki.ts
  - docs/2026-05-14-git-context-tool-backlog.md
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src-tauri/gen/schemas/capabilities.json
  - codebus-app/src-tauri/src/state/active_runs.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.tsx
  - codebus-app/src-tauri/gen/schemas/windows-schema.json
  - codebus-app/src-tauri/src/state/mod.rs
  - codebus-app/src-tauri/src/ipc/wiki.rs
  - codebus-app/src/components/workspace/NewGoalModal.tsx
  - codebus-core/src/verb/event.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src-tauri/src/lib.rs
  - docs/BACKLOG.md
  - codebus-app/src/components/workspace/WikiTab.tsx
  - Cargo.toml
tests:
  - codebus-app/src/hooks/useNewVaultShortcut.test.tsx
  - codebus-app/src/components/workspace/WorkspaceStub.test.tsx
  - codebus-app/src/lib/milkdown-wikilink.test.tsx
  - codebus-app/src/components/workspace/RunDetailRunning.test.tsx
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/hooks/useLobbyDragDrop.test.tsx
  - codebus-app/src/lib/ipc.test.ts
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/store/wiki.test.ts
  - codebus-app/src/components/workspace/NewGoalModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/store/route.test.ts
  - codebus-app/src/i18n/workspace.test.ts
  - codebus-app/src/components/workspace/WikiTree.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
  - codebus-app/src/components/workspace/RunDetailCancelled.test.tsx
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
-->

---
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