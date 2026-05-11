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

The system SHALL expose exactly five Tauri commands invokable from the frontend: `list_vaults`, `add_vault`, `remove_vault`, `load_global_config`, `save_global_config`. No other Tauri commands SHALL be registered by this change. Each command SHALL have a stable name (snake_case), a typed argument shape, and a typed return shape mirroring the design contract.

#### Scenario: Frontend invokes list_vaults

- **WHEN** the frontend calls `invoke("list_vaults")`
- **THEN** the command returns an array of `VaultEntry` objects, each with `path`, `display_name`, `last_opened`, and `is_missing` fields

#### Scenario: Unregistered commands fail invocation

- **WHEN** the frontend attempts to invoke any command name other than the five registered
- **THEN** Tauri returns a command-not-found error and the call rejects

---
### Requirement: AppError Discriminated Union

All five IPC commands SHALL return errors as a single `AppError` enum serialized with `serde(tag = "kind", rename_all = "snake_case")`. The variants SHALL be: `io`, `config_parse`, `vault_not_found`, `vault_already_exists`, `invalid`, `internal`. The frontend SHALL be able to discriminate on the `kind` field to render appropriate UI (toast vs inline error vs dialog).

#### Scenario: Vault path missing returns vault_not_found

- **WHEN** `add_vault` is invoked with a path that does not exist on disk
- **THEN** the command rejects with `AppError` having `kind: "vault_not_found"` and a `path` field containing the offered path

#### Scenario: Invalid threshold returns invalid with field name

- **WHEN** `save_global_config` is invoked with `app.quiz.pass_threshold` outside the 50–100 range
- **THEN** the command rejects with `AppError` having `kind: "invalid"`, `field: "app.quiz.pass_threshold"`, and a descriptive `message`

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

Folder drag-and-drop SHALL be accepted only while the application is in the Lobby state. In the Workspace state (and Workspace stub state for this change), drag-drop SHALL be either disabled or ignored, and SHALL NOT trigger the New Vault flow.

#### Scenario: Drop on Workspace stub is ignored

- **WHEN** a user drags a folder onto the application window while the Workspace stub is showing
- **THEN** no New Vault dialog appears, no `add_vault` is invoked, and the Workspace stub remains visible

#### Scenario: Drop multiple folders picks the first

- **WHEN** a user drags multiple folders onto the Lobby window in a single drop event
- **THEN** the system processes only the first folder for the New Vault flow and ignores the rest, with no error toast

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

When the user opens a vault from the Lobby (by clicking a card or completing a New Vault flow), the system SHALL transition the main view to a Workspace stub. The stub SHALL display the vault's display name and path in a sidebar, plus a primary message stating "Workspace coming in v3-app-workspace-goal" and a `← Back to Lobby` control. The stub SHALL NOT render any of the deferred Workspace functionality (no wiki tree, no goal list, no quiz, no Cmd+K overlay).

#### Scenario: Opening vault transitions to stub

- **WHEN** the user clicks a vault card or completes a New Vault flow
- **THEN** the main view transitions to the Workspace stub for that vault and the Lobby content is no longer visible

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control in the Workspace stub
- **THEN** the main view returns to the Lobby in whichever state matches the current vault list

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
- Quest banner, progress bar, or any "graduated" / "mastered" / "learned" page-level state in the Lobby or Workspace stub
- Tutorial slideshow UI, embedded checkpoints, or tutorial md generation triggers
- Telemetry, analytics, crash reporting, or auto-update channels
- A "Recent Pages" panel inside any sidebar
- Graph view entry in any sidebar
- Chat-mode Cmd+K with conversation memory (the overlay itself is out of scope for this change, but no precursor UI element SHALL be added)
- Direct LLM API calls from the frontend (all agent interaction goes through `codebus-core`)

#### Scenario: Settings modal has no theme or language controls

- **WHEN** the user opens the Settings modal in any state
- **THEN** the rendered modal contains exactly the seven fields defined in "Global Settings Modal Field Set" and no theme or language controls

#### Scenario: No telemetry network calls

- **WHEN** the codebus-app launches and runs through any Lobby or Settings flow
- **THEN** no outbound network requests are made by the app shell itself (LLM/agent invocations remain the responsibility of `codebus-core` and are out of scope for this change)
