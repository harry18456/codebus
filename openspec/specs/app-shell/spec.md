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

The `check_cli_installed` command SHALL accept a `provider: String` argument whose legal values are the literals `"claude_code"` and `"codex"`. The command SHALL probe whether the agentic CLI binary for that provider is reachable by spawning `<binary> --version`. It SHALL return a `CliStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `installed { version }` and `not_installed`). Any spawn failure — binary missing, non-zero exit, empty stdout — SHALL collapse to `not_installed`; the underlying error SHALL NOT surface to the frontend. A `provider` value outside the legal set SHALL collapse to `not_installed` (never an error to the frontend). Further provider values (`gemini_cli`, etc.) extend this match arm in a separate change.

The three keyring-management commands (`set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key`) SHALL accept a `service: String` argument naming the OS keyring service to act on. The Settings editor supplies the active provider's `azure.keyring_service` (defaulting to `codebus-claude-azure` for claude / `codebus-codex-azure` for codex), so claude and codex keys occupy DISTINCT keyring entries and the GUI writes to exactly the service the user sees — without a stale on-disk config lookup. An empty / whitespace-only `service` SHALL reject the call with `AppError::Invalid { field: "service", message: ... }`. The commands SHALL delegate to the codebus-core keyring helpers (`store_azure_key` / `probe_keyring_only` / `delete_azure_key`) — there is no separate keyring backend implementation in the app crate.

`set_endpoint_key` SHALL accept a `key: String` argument and store the value via the codebus-core helper. On success it SHALL return `Ok(())`. The key value SHALL NOT be cached anywhere in the app process beyond the Tauri command call boundary.

`get_endpoint_key` SHALL return a `KeyStatus` enum (serialised as `serde(tag = "kind", rename_all = "snake_case")` with variants `set` and `unset`) reflecting only whether the keyring entry exists. The command SHALL NOT return the key value under any circumstance, including with any optional flag — verifying the key value SHALL require running the CLI verb (`codebus query "ping"`) instead.

`delete_endpoint_key` SHALL be idempotent: removing a non-existent entry SHALL return `Ok(())` rather than an error.

The six new commands `spawn_goal`, `cancel_goal`, `list_runs`, `get_run_detail`, `list_wiki_pages`, and `read_wiki_page` are defined normatively in the `app-workspace` capability (Tauri IPC Commands for Goal Lifecycle and Wiki Read requirement). Their argument shapes, return types, and error behavior live in that capability; this registry requirement only pins their existence and total count.

#### Scenario: check_cli_installed probes claude binary

- **WHEN** the frontend invokes `check_cli_installed("claude_code")`
- **THEN** the command SHALL probe the claude binary and return `installed { version }` or `not_installed`

#### Scenario: check_cli_installed probes codex binary

- **WHEN** the frontend invokes `check_cli_installed("codex")`
- **THEN** the command SHALL probe the codex binary and return `installed { version }` or `not_installed` (a missing codex binary collapses to `not_installed`, never a frontend error)

#### Scenario: Unknown provider collapses to not_installed

- **WHEN** the frontend invokes `check_cli_installed("gemini_cli")`
- **THEN** the command SHALL return `not_installed` without surfacing an error


<!-- @trace
source: codex-settings-ui
updated: 2026-05-23
code:
  - codebus-core/src/agent/dispatch.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/verb/error.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/chat.rs
  - codebus-app/src/store/chat.ts
  - codebus-app/src/store/goals.ts
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
tests:
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
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

The Settings modal SHALL be invoked by the bottom-left gear in either Lobby or Workspace state. The modal SHALL display, in addition to the CLI Status row (see "Settings UI CLI Status Field") and the Endpoint Section (see "Settings UI Endpoint Section"), the following editable configuration fields:

1. AI Provider (read-only label: "Claude CLI (only option for now)")
2. PII scanner (dropdown showing scanner name and dynamic pattern count, e.g. `regex_basic · 14 patterns`)
3. PII on-hit policy (dropdown: `warn` / `skip` / `mask`) mapping to `pii.on_hit`
4. PII extra patterns (`pii.patterns_extra`): an editable list of raw regex strings with add and remove controls, no display label per entry
5. Lint fix enabled (toggle) mapping to `lint.fix.enabled`
6. Quiz content verify (toggle) mapping to `quiz.content_verify`
7. Goal content verify (toggle) mapping to `goal.content_verify`
8. Log sink (path display + Change folder link) with an additional control that disables logging entirely by writing `log.sink: none`
9. Quiz pass threshold (slider 50–100%, displayed value with `%` unit suffix)
10. Default quiz length (slider 3–10, displayed value with `questions` unit suffix)
11. Block image / binary reads (toggle) mapping to `hooks.read_image_block`. The toggle SHALL display the current resolved boolean value (default `true` when the config key is absent), and changing it SHALL set `hooks.read_image_block` to the new value on the next Save. The toggle SHALL be accompanied by visible copy stating that disabling it allows the agent to read image / PDF / binary files into its context AND that doing so bypasses the regex_basic PII filter (which only scans text). This copy SHALL be a security-conscious warning, not a neutral description, because the default is `true` (block) and disabling it weakens the PII safety floor.

The Endpoint Section SHALL render a read-only `chat` row that displays the model and effort the `chat` verb inherits from the `query` verb, in the form "沿用 query（<model> / <effort>）", kept in sync with the editable `query` row. The `chat` row SHALL NOT be editable and SHALL NOT introduce any `chat`-specific configuration key.

No theme toggle, language switcher, or per-vault override section SHALL be present. Sub-labels under fields SHALL NOT promise features absent from v1. The PII on-hit field SHALL display copy stating that Critical-severity matches are always masked regardless of this setting (the security floor cannot be disabled from the UI). The Quiz content verify and Goal content verify toggles SHALL each display copy stating that enabling them incurs additional verify/repair agent spawns.

The `save_global_config` IPC SHALL preserve every known and unknown subkey under any namespace it does not exclusively own. In particular, when enriching the `quiz` namespace with the resolved `default_length`, the IPC SHALL merge into the existing `quiz` object rather than replace it, so sibling keys (e.g. `quiz.content_verify`) set by the Settings UI survive a save→load round-trip. Unknown top-level YAML sections SHALL likewise continue to round-trip unchanged. The `hooks` namespace SHALL likewise round-trip through Save without losing unknown subkeys (forward-compat for future hook toggles).

#### Scenario: Modal opens from Lobby gear

- **WHEN** the user clicks the bottom-left gear in the Lobby
- **THEN** the Settings modal opens centered over a dimmed Lobby background

#### Scenario: PII pattern count is dynamic

- **WHEN** the Settings modal renders the PII scanner field
- **THEN** the displayed pattern count is read at runtime from the active scanner registry (not hard-coded in the UI source)

#### Scenario: PII on-hit field states the Critical security floor

- **WHEN** the Settings modal renders the PII on-hit policy field
- **THEN** the field displays selectable values `warn`, `skip`, `mask` AND visible copy stating that Critical-severity matches are always masked regardless of the selected value

#### Scenario: Content verify toggles state their cost

- **WHEN** the Settings modal renders the Quiz content verify and Goal content verify toggles
- **THEN** each toggle displays copy stating that enabling it incurs additional verify/repair agent spawns

#### Scenario: Invalid extra PII pattern blocks save

- **WHEN** the user enters a string that is not a valid regular expression into the PII extra patterns list
- **THEN** the field shows an inline error AND the Save button is disabled until the invalid pattern is corrected or removed

#### Scenario: Disabling logging writes sink none

- **GIVEN** `~/.codebus/config.yaml` has no `log` section
- **WHEN** the user activates the disable-logging control in the Log sink field and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `log:` with `sink: none` after save

#### Scenario: Chat row is read-only and mirrors query

- **GIVEN** the `query` verb resolves to model `haiku-4-5` and effort `low`
- **WHEN** the user opens the Settings modal Endpoint Section
- **THEN** a non-editable `chat` row displays "沿用 query（haiku-4-5 / low）" AND no `chat` key is written to `~/.codebus/config.yaml` on save

#### Scenario: Save persists atomically

- **WHEN** the user changes any field and clicks Save
- **THEN** the system writes `~/.codebus/config.yaml` atomically (temporary file then rename), closes the modal, and shows a "Saved" toast

##### Example: Quiz pass threshold round-trip

- **GIVEN** `~/.codebus/config.yaml` has `app.quiz.pass_threshold: 80`
- **WHEN** the user opens Settings, changes the threshold slider to 70, and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `app.quiz.pass_threshold: 70` after save, and reopening Settings shows the slider at 70

#### Scenario: quiz sibling subkeys survive save

- **GIVEN** the in-memory config payload has `quiz.default_length: 7` AND `quiz.content_verify: true`
- **WHEN** `save_global_config` writes the payload to disk and a subsequent `load_global_config` reads it back
- **THEN** the reloaded payload still contains `quiz.default_length: 7` AND `quiz.content_verify: true`

#### Scenario: Block image reads toggle defaults on when config key is absent

- **WHEN** `~/.codebus/config.yaml` has no `hooks` section AND the user opens the Settings modal
- **THEN** the "Block image / binary reads" toggle SHALL render in the ON position (matches the runtime default of `hooks.read_image_block: true`)

#### Scenario: Block image reads toggle displays security warning copy

- **WHEN** the Settings modal renders the "Block image / binary reads" toggle
- **THEN** the toggle SHALL display visible copy warning that disabling it allows the agent to read image and binary files which would bypass the regex_basic PII filter

#### Scenario: Disabling block image reads writes hooks.read_image_block false

- **GIVEN** `~/.codebus/config.yaml` has no `hooks` section AND the toggle is ON
- **WHEN** the user clicks the "Block image / binary reads" toggle to OFF and clicks Save
- **THEN** `~/.codebus/config.yaml` contains a `hooks` section with `read_image_block: false` after save AND reopening Settings shows the toggle in the OFF position

#### Scenario: Hooks namespace survives save

- **GIVEN** the in-memory config payload has `hooks.read_image_block: false` AND `hooks.future_hook_toggle: true` (forward-compat unknown subkey)
- **WHEN** `save_global_config` writes the payload to disk and a subsequent `load_global_config` reads it back
- **THEN** the reloaded payload still contains `hooks.read_image_block: false` AND `hooks.future_hook_toggle: true`

---
### Requirement: AppConfig Namespace Isolation

The system SHALL maintain an `app.*` namespace inside `~/.codebus/config.yaml`. After this change the namespace SHALL contain only `app.quiz.pass_threshold` (integer, 50–100, default 80). The `app.quiz.default_length` key SHALL NO LONGER live in `app.*`; the default quiz length is relocated to the shared `quiz.default_length` key defined by the `quiz` capability's Shared Quiz Config Namespace requirement. The codebus CLI binaries (`init`, `goal`, `query`, `lint`, `fix`, `quiz`) SHALL NOT read, write, or otherwise depend on the `app.*` namespace; the `codebus quiz` subcommand obtains its question count from the shared `quiz.*` namespace, never from `app.*`.

#### Scenario: CLI ignores app namespace

- **WHEN** any codebus CLI verb (including `quiz`) runs against a `~/.codebus/config.yaml` containing the `app.*` namespace
- **THEN** the CLI executes normally with no warnings about `app.*` and no modification to `app.*` values

#### Scenario: App reads pass_threshold default

- **WHEN** the app loads global config and `app.quiz.pass_threshold` is absent from the YAML
- **THEN** the loaded `GlobalConfig` returns `app.quiz.pass_threshold = 80` (default)

#### Scenario: default_length no longer read from app namespace

- **GIVEN** a `~/.codebus/config.yaml` that still contains a stale `app.quiz.default_length: 7` from a prior version
- **WHEN** the app resolves the default quiz length
- **THEN** the value SHALL be sourced from the shared `quiz.default_length` key (or its default of 5 when that shared key is absent) AND the stale `app.quiz.default_length` SHALL NOT be the source of truth


<!-- @trace
source: v3-app-quiz
updated: 2026-05-16
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src/lib/ipc.ts
  - docs/spike-artifacts/quiz-fixture-vault/manifest.yaml
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/jwt-token-lifecycle.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/index.md
  - docs/spike-artifacts/spike-quiz-7-F5.jsonl
  - codebus-app/src-tauri/src/ipc/quiz.rs
  - codebus-cli/src/main.rs
  - codebus-core/src/config/quiz.rs
  - docs/spike-artifacts/spike-quiz-7-F1.jsonl
  - codebus-app/src-tauri/src/ipc/config.rs
  - docs/2026-05-15-v3-app-quiz-spike-plan.md
  - docs/spike-artifacts/spike-quiz-7-F6.jsonl
  - docs/spike-artifacts/spike-quiz-8-E3.jsonl
  - docs/spike-artifacts/spike-quiz-9-S1.jsonl
  - codebus-core/src/verb/quiz.rs
  - docs/v3-app-roadmap.md
  - codebus-cli/src/commands/mod.rs
  - codebus-core/src/config/claude_code.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run2.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC1.jsonl
  - docs/spike-artifacts/spike-quiz-10-NC2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/user-store.md
  - docs/spike-artifacts/spike-quiz-10-R1-run1.jsonl
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-core/src/skill_bundle/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/wiki/log.md
  - docs/spike-artifacts/spike-quiz-7-F2.jsonl
  - docs/spike-artifacts/spike-quiz-8-E4.jsonl
  - codebus-app/src/components/workspace/QuizAnswering.tsx
  - docs/2026-05-15-v3-app-quiz-discussion.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/concepts/session-vs-token.md
  - docs/spike-artifacts/spike-quiz-8-E5.jsonl
  - codebus-cli/src/commands/quiz.rs
  - docs/spike-artifacts/spike-quiz-9-S3.jsonl
  - codebus-core/src/config/mod.rs
  - codebus-core/src/log/events/sink.rs
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - docs/spike-artifacts/spike-quiz-runbook.md
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-core/src/verb/mod.rs
  - docs/spike-artifacts/quiz-fixture-vault/CLAUDE.md
  - codebus-core/src/verb/event.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/log/events/jsonl_sink.rs
  - docs/spike-artifacts/spike-quiz-8-E2.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/raw/code/auth.py
  - docs/spike-artifacts/spike-quiz-8-E1.jsonl
  - docs/spike-artifacts/spike-quiz-7-F3.jsonl
  - docs/spike-artifacts/quiz-fixture-vault/.claude/skills/codebus-quiz/SKILL.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/modules/auth-middleware.md
  - docs/spike-artifacts/quiz-fixture-vault/wiki/processes/login-flow.md
  - docs/spike-artifacts/spike-quiz-9-S2.jsonl
  - codebus-core/src/vault/source_gitignore.rs
  - docs/spike-artifacts/spike-quiz-10-R1-run3.jsonl
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src-tauri/src/ipc/mod.rs
  - docs/spike-artifacts/spike-quiz-7-F4.jsonl
tests:
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-core/tests/vault_init.rs
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/quiz_flow.rs
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-core/tests/verb_library_surface.rs
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/workspace/QuizAnswering.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-cli/tests/cli_routing.rs
-->

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
- **THEN** the rendered modal contains exactly the fields defined in "Global Settings Modal Field Set" plus the CLI Status row and Endpoint Section defined by their own requirements, and no theme or language controls

#### Scenario: No telemetry network calls

- **WHEN** the codebus-app launches and runs through any Lobby or Settings flow
- **THEN** no outbound network requests are made by the app shell itself (LLM/agent invocations remain the responsibility of `codebus-core` and are out of scope for this change)


<!-- @trace
source: settings-config-frontend
updated: 2026-05-20
code:
  - codebus-core/src/verb/goal.rs
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/git/mod.rs
  - codebus-cli/src/commands/goal.rs
  - codebus-core/src/verb/content_verify.rs
  - docs/2026-05-14-pii-settings-ui-backlog.md
  - codebus-core/src/git/nested_repo.rs
  - codebus-core/src/skill_bundle/mod.rs
  - docs/2026-05-19-settings-config-coverage-backlog.md
  - docs/BACKLOG.md
  - codebus-core/src/config/goal.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/verb/mod.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-19-raw-sync-nested-git-leak-backlog.md
  - codebus-app/src/i18n/messages.ts
  - codebus-core/src/config/mod.rs
tests:
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-cli/tests/bins/mock_claude.rs
  - codebus-cli/tests/goal_flow.rs
  - codebus-cli/tests/goal_content_verify_cli.rs
-->

---
### Requirement: Settings UI CLI Status Field

The Settings modal SHALL render a CLI Status row that probes whether the agentic CLI binary for the currently selected provider is installed. The row SHALL invoke `check_cli_installed` with the selected provider's `cliBinaryId` (from the provider registry) on modal open AND whenever the selected provider changes. It SHALL display one of three states: `Checking…` (probe in flight), `Installed · <version>` (success), or `Not installed` (any failure). When the state is `Not installed`, the row SHALL render an inline hint instructing the user to install that provider's CLI before configuring the endpoint. The row label SHALL reflect the selected provider's `displayName`.

#### Scenario: CLI status row probes claude when claude is selected

- **WHEN** the selected provider is `claude` AND `check_cli_installed("claude_code")` returns `{ kind: "installed", version: "2.1.139 (Claude Code)" }`
- **THEN** the Settings UI SHALL display a status badge containing `Installed` AND the version string `2.1.139 (Claude Code)`

#### Scenario: CLI status row probes codex when codex is selected

- **WHEN** the selected provider is `codex` AND `check_cli_installed("codex")` returns `{ kind: "not_installed" }`
- **THEN** the Settings UI SHALL display a status badge containing `Not installed` AND an inline hint instructing the user to install the codex CLI


<!-- @trace
source: codex-settings-ui
updated: 2026-05-23
code:
  - codebus-core/src/agent/dispatch.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/verb/error.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/chat.rs
  - codebus-app/src/store/chat.ts
  - codebus-app/src/store/goals.ts
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
tests:
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->

---
### Requirement: Settings UI Endpoint Section

The Settings modal SHALL render a provider selector AND an Endpoint section that lets the user configure the currently selected provider's endpoint profiles entirely from the GUI. The provider selector SHALL list the providers in the registry (`claude`, `codex`); selecting one SHALL set `agent.active_provider` and SHALL switch the Endpoint section to that provider's editor component and the CLI Status row to that provider's binary probe. The Endpoint section heading SHALL reflect the selected provider's `displayName`.

The Endpoint editor rendered SHALL be the registry entry's editor component for the selected provider:

- The claude editor SHALL contain an `active` radio group (`system` / `azure`); a System Profile sub-section with four verb rows (`goal` / `query` / `fix` / `verify`), each row a free-text `model` combobox input wired to a suggestion `<datalist>` of the known aliases (`opus-4-7`, `opus-4-6`, `haiku-4-5`, `sonnet-4-6`) — a newly-released Claude model MAY be typed directly (codebus-core relaxed `SystemModel` to a free string translated to the CLI `--model` flag via the `claude-` prefix) — AND an `effort` `<select>` with six options (`low`, `medium`, `high`, `xhigh`, `max`, `auto`); and an Azure Profile sub-section with a `base_url` text input, a `keyring_service` text input (default `codebus-claude-azure`), an API-key status indicator with `Set new...` / `Delete` buttons, AND four verb rows each with a free-text `model` (deployment name) input AND an `effort` `<select>`.
- The codex editor SHALL contain an `active` radio group over the codex provider's declared profiles (`system` / `azure`); a System Profile sub-section with four verb rows each containing a free-text `model` input (codex model names are arbitrary strings, NOT a closed enum) AND an `effort` input; and an Azure Profile sub-section with a `base_url` text input, an `api_version` text input, a `keyring_service` text input (default `codebus-azure`), the API-key status indicator with `Set new...` / `Delete` buttons, AND four verb rows each with a free-text deployment-name `model` input AND an `effort` input.

Both profile sub-sections of the active provider SHALL be present in the DOM organised as an accordion: the sub-section whose name matches the provider's `active` value SHALL be expanded; the other SHALL be collapsed showing only its header with an `(inactive)` label. Toggling the `active` radio SHALL auto-expand the newly-active sub-section and auto-collapse the previously-active one WITHOUT clearing any form input values.

The `Save` button SHALL persist only yaml content via `save_global_config`; it SHALL NOT carry any API key. API keys SHALL flow exclusively through the three `*_endpoint_key` IPC commands. The Settings UI SHALL NOT include a Test Connection button.

The Settings modal SHALL perform client-side validation of the selected provider's endpoint block before allowing Save, using the registry entry's `validate(block)` function. The validation rules SHALL match the corresponding codebus-core parser (`parse_claude_code_yaml` for claude, `parse_codex_yaml` for codex) so the frontend and the `save_global_config` backend gate produce the same reject/accept decision. On failure the modal SHALL disable Save, render an inline validation summary listing each failing field, AND apply `aria-invalid="true"` to the offending inputs. The backend `save_global_config` SHALL validate the active provider's block via the matching core parser and reject an invalid block before writing the file.

The codex provider's validation SHALL require, when codex `active === "azure"`, non-empty `base_url`, `api_version`, `keyring_service`, AND each verb's `model`; and when `active === "system"`, non-empty `model` for each verb. Unknown codex `model` strings SHALL NOT be rejected (codex models are not a closed enum).

#### Scenario: Selecting codex switches editor and CLI probe

- **WHEN** the user selects `codex` in the provider selector
- **THEN** `agent.active_provider` SHALL become `codex` AND the Endpoint section SHALL render the codex editor (free-text model inputs plus an `api_version` field in the azure sub-section) AND the CLI Status row SHALL probe the codex binary

#### Scenario: Claude editor uses a free-text model combobox

- **WHEN** the selected provider is `claude`
- **THEN** the System Profile model control SHALL be a free-text input wired to a suggestion `<datalist>` of the four known aliases, AND typing a not-yet-known model (e.g. `opus-4-8`) SHALL be accepted (no closed-enum rejection)

#### Scenario: Codex azure requires base_url, api_version, keyring_service, and verb models

- **WHEN** the selected provider is `codex`, codex `active === "azure"`, AND `api_version` (or any required azure field) is the empty string AND the user has edited a setting
- **THEN** the Save button SHALL be disabled AND the validation summary SHALL list the failing field(s) including `api_version`

#### Scenario: Codex system accepts arbitrary model strings

- **WHEN** the selected provider is `codex`, codex `active === "system"`, AND a verb `model` is set to `gpt-5.5`
- **THEN** validation SHALL accept it (no closed-enum rejection) AND, if all verbs' models are non-empty and the user has edited a setting, the Save button SHALL be enabled

#### Scenario: Backend rejects an invalid codex block on save

- **WHEN** `save_global_config` receives a config with `agent.active_provider: codex` and a codex block whose active profile is missing a required verb
- **THEN** the command SHALL reject with an `AppError` (validated via `parse_codex_yaml`) AND SHALL NOT write the file


<!-- @trace
source: codex-settings-ui
updated: 2026-05-23
code:
  - codebus-core/src/agent/dispatch.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/verb/error.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/chat.rs
  - codebus-app/src/store/chat.ts
  - codebus-app/src/store/goals.ts
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
tests:
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->

---
### Requirement: Lobby Subscribes To Vault List Watcher

The Lobby SHALL subscribe to the `vault-list-changed` Tauri event (defined by the `fs-watcher` capability) via the `useWatcherEvent` hook and SHALL invoke `useVaultListStore.load()` whenever the event fires. The subscription SHALL be active for the entire lifetime of the Lobby component and SHALL be cleaned up on unmount.

#### Scenario: External vault add refreshes Lobby

- **GIVEN** the Lobby is displayed AND a vault watcher monitors `~/.codebus/app-state.json`
- **WHEN** an external process appends a new vault entry to `~/.codebus/app-state.json`
- **THEN** the Lobby SHALL re-render with the new vault card visible within 400 ms (200 ms debounce window plus scheduling slack)

#### Scenario: Subscription is cleaned up on unmount

- **GIVEN** the Lobby has subscribed to `vault-list-changed`
- **WHEN** the Lobby unmounts (user opens a vault and enters Workspace)
- **THEN** the `useWatcherEvent` cleanup function SHALL be invoked AND no further Lobby re-render SHALL be triggered by subsequent `vault-list-changed` events while the Lobby is unmounted


<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Workspace Manages Per-Vault Watcher Lifecycle

The Workspace component SHALL invoke `start_vault_watcher(vault_path)` on mount and `stop_vault_watcher(vault_path)` on unmount, binding the per-vault watcher's lifecycle to the Workspace as defined by the `fs-watcher` capability. Switching from one vault's Workspace to another's SHALL release the prior vault's watcher before starting the new one.

#### Scenario: Workspace mount starts the watcher for the open vault

- **WHEN** the user opens vault V from the Lobby and the Workspace component mounts
- **THEN** `start_vault_watcher(V)` SHALL be invoked exactly once before any watcher-driven refresh is expected to occur

#### Scenario: Workspace unmount stops the watcher

- **WHEN** the user returns from Workspace to Lobby
- **THEN** `stop_vault_watcher(V)` SHALL be invoked for the previously open vault V

#### Scenario: Vault switch releases the prior watcher

- **GIVEN** the Workspace is mounted for vault V1
- **WHEN** the user switches to vault V2 (Workspace remounts)
- **THEN** `stop_vault_watcher(V1)` SHALL be invoked AND then `start_vault_watcher(V2)` SHALL be invoked, in that order

<!-- @trace
source: codebus-fs-watcher
updated: 2026-05-20
code:
  - codebus-app/src/components/workspace/WikiTab.tsx
  - codebus-app/src-tauri/src/watcher.rs
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/components/workspace/WikiPreview.tsx
  - Cargo.toml
  - codebus-app/src/components/workspace/Workspace.tsx
  - codebus-app/src/hooks/useWatcherEvent.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/src-tauri/src/lib.rs
  - codebus-app/src-tauri/Cargo.toml
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/src/components/workspace/WatcherStatusBanner.tsx
  - codebus-app/src/store/vault-watcher-status.ts
  - codebus-app/src-tauri/src/ipc/mod.rs
tests:
  - codebus-app/src/components/workspace/WikiTab.test.tsx
  - codebus-app/src/hooks/useWatcherEvent.test.ts
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/QuizTab.test.tsx
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/store/vault-watcher-status.test.ts
  - codebus-app/src/components/workspace/WikiPreview.test.tsx
  - codebus-app/src/components/lobby/Lobby.test.tsx
-->

---
### Requirement: Settings Provider Registry

The Settings layer SHALL drive its provider-specific behavior from a single provider registry rather than hard-coding the claude provider. Each registry entry SHALL declare: a stable `id` (the `agent.active_provider` value, e.g. `claude` or `codex`), a `displayName`, a `cliBinaryId` (the argument passed to `check_cli_installed`), the set of endpoint `profiles` the provider supports, a `validate(block)` function returning the provider's client-side validation errors, and an endpoint-editor component. The registry SHALL contain exactly two entries in this change: `claude` and `codex`.

The set of profiles SHALL be declared per provider, NOT assumed universal. `claude` and `codex` each declare `["system", "azure"]`; a future provider MAY declare a different set (e.g. only `["system"]`) and SHALL NOT be forced to expose an azure profile. Each provider's endpoint editor is a concrete component (no generic schema-to-form engine); the registry abstracts only the cross-provider glue (provider selection, config read/write keyed by provider id, CLI status probe by `cliBinaryId`, validation dispatch, and backend validation dispatch).

The in-memory config store SHALL expose provider-keyed accessors `getProviderBlock(id)` and `updateProviderBlock(id, block)` that read and write `agent.providers.<id>` and set `agent.active_provider` to the currently selected provider. The store SHALL NOT hard-code `active_provider` to `claude`. Adding a future provider SHALL require adding a registry entry plus its editor component, with no change to the cross-provider glue.

#### Scenario: Registry exposes claude and codex entries

- **WHEN** the Settings modal initializes its provider registry
- **THEN** the registry SHALL contain entries for `claude` and `codex`, each with a `cliBinaryId`, declared `profiles`, a `validate` function, and an editor component

#### Scenario: Provider declares its own profiles without an assumed azure slot

- **WHEN** a provider registry entry declares its supported `profiles`
- **THEN** the declared set SHALL be used verbatim (codex declares `["system", "azure"]`) AND no azure profile SHALL be synthesized for a provider that did not declare one

#### Scenario: Store writes the selected provider block and active_provider

- **WHEN** `updateProviderBlock("codex", block)` is called
- **THEN** the in-memory config SHALL set `agent.providers.codex` to `block` AND `agent.active_provider` to `codex`, preserving sibling `agent.providers.*` entries

<!-- @trace
source: codex-settings-ui
updated: 2026-05-23
code:
  - codebus-core/src/agent/dispatch.rs
  - codebus-app/src-tauri/src/ipc/goals.rs
  - codebus-core/src/config/codex.rs
  - codebus-core/src/agent/codex_backend.rs
  - codebus-core/src/verb/fix.rs
  - codebus-app/src/store/settings.ts
  - codebus-app/src/components/workspace/ChatTranscript.tsx
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-core/src/stream/codex_parser.rs
  - codebus-core/src/config/endpoint.rs
  - codebus-core/src/agent/claude_backend.rs
  - codebus-core/src/vault/init.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-cli/src/commands/config.rs
  - codebus-core/src/verb/goal.rs
  - codebus-core/src/verb/quiz.rs
  - docs/2026-05-14-multi-provider-agent-backend-backlog.md
  - codebus-app/src-tauri/src/ipc/cli_status.rs
  - codebus-core/src/config/mod.rs
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-core/src/verb/error.rs
  - codebus-app/src-tauri/src/ipc/keyring.rs
  - codebus-core/src/stream/mod.rs
  - codebus-core/src/config/claude_code.rs
  - codebus-app/src/lib/ipc.ts
  - codebus-core/src/skill_bundle/mod.rs
  - codebus-core/src/verb/chat.rs
  - codebus-app/src/store/chat.ts
  - codebus-app/src/store/goals.ts
  - codebus-app/src/components/settings/SetKeyDialog.tsx
  - codebus-core/src/agent/mod.rs
  - codebus-core/src/verb/query.rs
  - codebus-app/src/lib/providers.ts
  - codebus-app/src/components/settings/SettingsModal.tsx
tests:
  - codebus-app/src/components/settings/SettingsModal.codex.test.tsx
  - codebus-app/src/lib/providers.test.ts
  - codebus-app/src/store/goals.test.ts
  - codebus-app/src/lib/codex-validation.test.ts
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src-tauri/tests/keyring_ipc.rs
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
  - codebus-app/src/store/chat.test.ts
  - codebus-cli/tests/parse_error_aborts_all_verbs.rs
-->

---
### Requirement: i18n Bundle Coverage Policy

All user-facing strings rendered by `codebus-app` components SHALL be defined as keys in `codebus-app/src/i18n/messages.ts` and consumed through the `useT` hook. "User-facing" SHALL include: visible button labels, link text, headings, body copy, DialogTitle / DialogDescription content, form labels, input placeholders, status badges, error and success messages, toast content, AND assistive-technology attributes (`aria-label`, `aria-description`, `title` attr surfaced as tooltip / accessible name).

The bundle SHALL maintain key parity between `en` and `zh` locales — TypeScript MUST fail to compile if a key exists in `en` but is missing in `zh`, or vice versa.

The following identifier categories SHALL be treated as jargon and SHALL remain English in BOTH locales (i.e., still defined in the bundle for centralization, but `en` and `zh` values are identical English strings):

1. Workspace tab labels: `Goals`, `Wiki`, `Quiz`.
2. Verb names visible in settings UI: `goal`, `query`, `fix`, `verify`, `chat`.
3. Codex effort enum values: `low`, `medium`, `high`, `xhigh`.
4. PII action enum values: `warn`, `mask`, `block`.
5. Config YAML key names rendered as field labels: `base_url`, `api_version`, `keyring_service`.

The following SHALL NOT be required to go through the i18n bundle, because they are program identifiers and not UI labels:

1. Claude API tool name identifiers used in `case` match statements or stream-event discriminants (e.g. `Read`, `Write`, `Glob`, `Grep`, `Edit`, `Bash`).
2. Internal log messages, comments, JSDoc, and developer-facing console output.

Where the same accessibility concept appears in more than one component, the bundle SHALL expose ONE shared key consumed by all sites, rather than per-component duplicate keys.

When a label is composed of an emoji or symbol prefix immediately followed by translatable text (e.g. `🎯 Goal target`, `🚌 Here comes the CodeBus...`), the entire string including the emoji SHALL be stored as a single bundle value. The emoji and text MUST NOT be split into separate keys, because the emoji is part of the label's semantic meaning and translation MUST preserve them as one unit per locale.

#### Scenario: Component renders a user-facing string

- **WHEN** a `codebus-app` component renders any visible label, placeholder, DialogTitle, error message, status badge, button text, or sets an `aria-label` / `title` attribute used as an accessible name
- **THEN** the string MUST be sourced from `t("<key>")` where `<key>` is defined in both `en` and `zh` maps of `codebus-app/src/i18n/messages.ts`

#### Scenario: Adding a jargon term to the bundle

- **WHEN** a jargon identifier from the allow-list (workspace tab label, verb name, codex effort value, PII action value, or config YAML key name) is rendered in UI
- **THEN** the bundle SHALL define a key for it AND the `en` and `zh` values SHALL be identical English strings

##### Example: jargon allow-list bundle entries

| Key | en value | zh value | Allow-list category |
| --- | -------- | -------- | ------------------- |
| `workspace.tab.goals` | `Goals` | `Goals` | tab label |
| `settings.codex.effort.value.high` | `high` | `high` | codex effort enum |
| `settings.pii.action.block` | `block` | `block` | PII action enum |
| `settings.endpoint.field.baseUrl` | `base_url` | `base_url` | config YAML key |

#### Scenario: Shared accessibility key reused across components

- **WHEN** multiple components surface the SAME accessibility concept (such as the "Page not found" tooltip rendered on broken wiki links inside `ChatTranscript`, `ExplanationText`, and `WikiPreview`)
- **THEN** the bundle SHALL define one shared key (e.g. `a11y.pageNotFound`) AND all such components SHALL consume that single key rather than each defining its own

#### Scenario: Emoji-prefixed label stored as one bundle value

- **WHEN** a component renders a label whose visible form is an emoji or symbol prefix followed by translatable text (e.g. activity banner labels such as `🎯 Goal target`, status banners such as `🚌 Here comes the CodeBus...`)
- **THEN** the bundle SHALL define ONE key whose value contains both the emoji and the text in each locale, AND the component MUST NOT concatenate two separate keys for the emoji and the text

##### Example: bannerLabel emoji-text bundle entries

| Key | en value | zh value |
| --- | -------- | -------- |
| `workspace.activity.banner.goal` | `🎯 Goal target: {goalText}` | `🎯 任務目標：{goalText}` |
| `workspace.activity.banner.done` | `🎉 Complete` | `🎉 完成` |

#### Scenario: 6-pattern sweep finds no policy violations

- **WHEN** a reviewer or maintainer runs the canonical 6-pattern grep sweep (Patterns 1a, 1b, 2, 3, 4 against `codebus-app/src/components/**/*.tsx`; Pattern 5 against `codebus-app/src/**/*.{ts,tsx}` for template-literal interpolation with Latin neighbours; Pattern 6 against `codebus-app/src/**/*.ts` outside `components/` for helper / lib files)
- **THEN** every reported line MUST resolve to one of: (a) a `t("...")` call, (b) an entry from the Cat D jargon allow-list, (c) a Claude API tool name identifier from the non-UI exclusion list, or (d) a documented runtime-keyword identifier such as the re-init confirmation literal `delete` in `NewVaultFlow.tsx`. Any unaccounted line constitutes a policy violation requiring a follow-up change.

##### Example: canonical 6-pattern sweep commands

| # | Purpose | Command (run from `codebus-app/`) |
| - | ------- | --------------------------------- |
| 1a | JSX text content with Latin (single-line) | `grep -rPn '>([^<{]*[A-Za-z]+[^<{]*)<' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -vE 't\("\|\{t\(\|className=\|data-testid='` |
| 1b | Indented JSX text (multi-line) | `grep -rPn "^[[:space:]]+[A-Z][a-zA-Z][a-zA-Z' ]*[a-zA-Z][\.…!\?]?[[:space:]]*$" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 2 | Emoji / arrow-prefixed Latin string | `grep -rPn '[←→↻⏹⚠✓✕▸▿⤢⤡⏺] [A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 3 | Untranslated `aria-label` / `title` / `placeholder` attrs | `grep -rPn '(aria-label\|title\|placeholder)="[^"]*[A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -v 't("'` |
| 4 | String literals with placeholder syntax | `grep -rPn "'[A-Za-z][^']*\{\w+\}" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 5 | Template-literal interpolation with Latin neighbour | `grep -rPn "\`[^\`]*\$\{[^}]+\}[^\`]*[A-Za-z]" src/ --include='*.ts' --include='*.tsx' \| grep -v '.test.' \| grep -v 't("'` |
| 6 | Helper / lib hard-codes outside `components/` | Re-run Patterns 1a / 1b / 2 / 3 / 4 with `src/` as the search root and `--include='*.ts' --include='*.tsx'`, then exclude paths under `src/components/` (already covered by Patterns 1-4) and any `.test.` files |

Pattern 1 was originally written as `>(?![\s{<])([^<{]*[A-Za-z]){2,}[^<{]*<` requiring two or more Latin chunks; that excluded single-word JSX text such as `Checking…` and `Loading…` (Phase 3A residual sweep 2026-05-26). Pattern 1 is now split into 1a (single-line JSX node) and 1b (multi-line JSX text whose Latin word lives on its own indented line) to cover both shapes.

Patterns 5 and 6 were added in the Phase 3A follow-up sweep (2026-05-26) after CDP en-locale smoke surfaced template-literal hard-codes such as `` `${diffHr}h ago` `` in `RunListItem.tsx` and helper-produced strings such as the pass / fail verdict in `src/lib/quiz-parse.ts` that the original 4-pattern sweep could not match. Pattern 5 catches strings inside backtick literals (no `<>` brackets, no symbol prefix, no attribute prefix, double quotes only). Pattern 6 widens the search root from `src/components/` to all of `src/` so that helper modules, formatters, and other `.ts` files outside `components/` are covered.

JSX text starting with non-Latin punctuation (e.g. `+ New goal`, `+ New chat`) remains a known gap of Patterns 1a and 1b — those button labels lose the `[A-Z]` anchor and Latin-chunk count requirement, so they SHALL be discovered by manual CDP smoke or by reviewer pattern-matching rather than by automated sweep. Future follow-up changes MAY introduce Pattern 7 for that shape if recurring instances justify it.

<!-- @trace
source: i18n-sweep-phase-3a-followup
updated: 2026-05-26
code:
  - codebus-app/scripts/.i18n-followup-smoke/02-workspace-goals.png
  - codebus-app/scripts/.i18n-followup-smoke/05-rundetail-done.png
  - codebus-app/scripts/.i18n-followup-smoke/force-en.mjs
  - codebus-app/scripts/.i18n-followup-smoke/SMOKE-REPORT.md
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-app/src/components/workspace/GoalsTab.tsx
  - codebus-app/src/lib/quiz-parse.ts
  - codebus-app/src/components/workspace/ActivityStreamItem.tsx
  - codebus-app/src/components/workspace/QuizTab.tsx
  - codebus-app/scripts/.i18n-followup-smoke/01-lobby.png
  - codebus-app/src/components/workspace/RunListItem.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/scripts/.i18n-followup-smoke/06-settings.png
  - codebus-app/src/components/workspace/RunDetailDone.tsx
  - codebus-app/scripts/.i18n-followup-smoke/04-chat.png
  - codebus-app/src/components/workspace/ChatNewChatButton.tsx
  - codebus-app/scripts/.i18n-followup-smoke/03-quiz-tab.png
  - codebus-app/src/components/workspace/ChatTokenDisplay.tsx
tests:
  - codebus-app/src/components/workspace/RunDetailDone.test.tsx
  - codebus-app/src/components/workspace/ChatNewChatButton.test.tsx
  - codebus-app/src/components/workspace/GoalsTab.test.tsx
  - codebus-app/src/components/workspace/ActivityStreamItem.test.tsx
  - codebus-app/src/i18n/activityBanner.test.ts
  - codebus-app/src/components/workspace/RunListItem.test.tsx
  - codebus-app/src/lib/quiz-parse.test.ts
-->