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

The Lobby SHALL render in exactly one of two states determined by `vault_list` length.

The populated state SHALL display vault cards with `display_name`, `path`, and human-readable relative `last_opened` (absolute date after 30 days), plus a top-right primary action button whose label SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale. The populated state SHALL render a section label above the card list using the shared `SectionLabel` component (default variant, no uppercase tracking), and the label text SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale. The populated state SHALL render a drag-tip caption below the card list whose text SHALL NOT contain the literal word "Vault" or "vault" in any rendered locale.

Each vault card SHALL expose a visible kebab (`⋮`) button on hover and on keyboard focus that opens the per-vault action menu (Reveal in files / Remove); right-click (context menu) on the card SHALL continue to open the same menu as a shortcut. The kebab button SHALL be hidden (zero opacity) when neither hovered nor focused so it does not add static visual noise.

The empty state SHALL display a hero with a large 🚌 emoji, a title, a subtitle, a primary `+ Board a new bus` CTA (or its localized equivalent), and a Quickstart 3-step orientation card. The Quickstart card SHALL render each step number as a monospace digit without a trailing period, in `text-fg-tertiary` color. The Quickstart step 2 SHALL render its example fragment inside an amber-tinted monospace pill (background `accent-tint`, foreground `accent`, 1px amber-tinted border, `rounded-sm`, monospace font); the example fragment SHALL be sourced from a dedicated i18n key separate from the step prefix so that pill styling and step wording can evolve independently.

The Lobby `<main>` content SHALL flow from the top using a vertical flex column; it SHALL NOT vertically center its content within the viewport in either state. The bottom strip (Settings gear left, version label right) SHALL render ONLY when the application route is the Lobby (i.e., `route.kind === "lobby"`); it SHALL NOT render in the Workspace route. When rendered, the bottom strip SHALL remain a sibling of the Lobby `<main>` at the application shell level, naturally occupying the bottom of the viewport.

#### Scenario: Empty list renders empty state

- **WHEN** the Lobby loads and `vault_list` is empty
- **THEN** the empty-state hero (🚌 emoji, title, subtitle, Board-a-new-bus CTA, Quickstart card) is rendered and no vault cards are shown

#### Scenario: Non-empty list renders cards

- **WHEN** the Lobby loads and `vault_list` contains one or more entries
- **THEN** vault cards are rendered in reverse-chronological order by `last_opened`, the top-right primary add-action button is shown, and a non-uppercase-tracked section label is rendered above the card list

#### Scenario: Populated state UI text contains no "Vault" literal

- **WHEN** the Lobby loads in populated state in any supported locale (zh, en)
- **THEN** the topbar add-action button label, the section label above the card list, and the drag-tip caption below the card list each contain no occurrence of the literal string "Vault" or "vault"

##### Example: locale string audit

| Element             | zh literal forbidden | en literal forbidden |
| ------------------- | -------------------- | -------------------- |
| Topbar add-action   | "Vault" / "vault"    | "Vault" / "vault"    |
| Populated section   | "Vault" / "vault"    | "Vault" / "vault"    |
| Drag-tip caption    | "Vault" / "vault"    | "Vault" / "vault"    |

#### Scenario: Vault card kebab visible on hover and focus

- **WHEN** a user moves the pointer over a vault card or focuses the card via keyboard navigation
- **THEN** a visible kebab (`⋮`) button appears at the card's right edge, and activating it opens the action menu anchored to the button

#### Scenario: Vault card kebab hidden when idle

- **WHEN** a vault card is neither pointer-hovered nor keyboard-focused
- **THEN** the kebab button is rendered at zero opacity so it does not contribute static visual noise to the list

#### Scenario: Vault card right-click still opens menu

- **WHEN** a user right-clicks (context menu) anywhere on a vault card
- **THEN** the same action menu opens, positioned at the cursor

#### Scenario: Quickstart step number uses monospace digits without period

- **WHEN** the Lobby renders the empty state Quickstart card
- **THEN** each step is prefixed by a monospace digit (1, 2, 3) with no trailing period and rendered in `text-fg-tertiary` color

#### Scenario: Quickstart step 2 example renders in amber pill

- **WHEN** the Lobby renders the empty state Quickstart card
- **THEN** the example fragment of step 2 is wrapped in an inline element styled as an amber-tinted monospace pill (background `accent-tint`, foreground `accent`, 1px amber-tinted border, `rounded-sm` corners) distinct from the surrounding step text

#### Scenario: Lobby content flows from the top

- **WHEN** the Lobby is rendered in either empty or populated state at common desktop viewports (e.g., 1920×1080 at 100% scaling)
- **THEN** the Lobby `<main>` content (hero / cards) is aligned to the top of the available area, not vertically centered, and the application-shell bottom strip occupies the bottom of the viewport naturally

#### Scenario: Section labels use the shared SectionLabel component

- **WHEN** the Lobby renders the populated state's recent-cards label or the empty state's Quickstart label
- **THEN** each label is rendered by the shared `SectionLabel` component in its default (non-uppercase-tracked) variant, so the visual treatment is identical between Latin and CJK label text

#### Scenario: Bottom strip is hidden in the Workspace route

- **GIVEN** the user has opened a vault and the application route is the Workspace
- **WHEN** the application shell renders
- **THEN** no element with `data-testid="bottom-strip"` exists in the DOM AND no Lobby version-label element exists in the DOM

#### Scenario: Bottom strip reappears when returning to the Lobby

- **GIVEN** the user is in the Workspace with no bottom strip rendered
- **WHEN** the user clicks the sidebar `← Back to Lobby` control and the route transitions back to the Lobby
- **THEN** the bottom strip is rendered again as a sibling of the Lobby `<main>` with its Settings gear on the left and the version label on the right


<!-- @trace
source: workspace-sidebar-rework
updated: 2026-05-27
code:
  - codebus-app/src/store/quiz-history.ts
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/App.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/store/quiz-history.test.ts
-->

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
2. PII scanner (dropdown showing scanner name and dynamic pattern count, e.g. `regex_basic · 13 patterns`)
3. PII on-hit policy (dropdown: `warn` / `skip` / `mask`) mapping to `pii.on_hit`
4. PII extra patterns (`pii.patterns_extra`): an editable list of raw regex strings with add and remove controls, no display label per entry
5. Lint fix enabled (toggle) mapping to `lint.fix.enabled`
6. Quiz content verify (toggle) mapping to `quiz.content_verify`
7. Goal content verify (toggle) mapping to `goal.content_verify`
8. Log sink (path display + Change folder link) with an additional control that disables logging entirely by writing `log.sink: none`
9. Quiz pass threshold (slider 50–100%, displayed value with `%` unit suffix)
10. Default quiz length (slider 3–10, displayed value with `questions` unit suffix)
11. Block image / binary reads (toggle) mapping to `hooks.read_image_block`. The toggle SHALL display the current resolved boolean value (default `true` when the config key is absent), and changing it SHALL set `hooks.read_image_block` to the new value on the next Save. The toggle SHALL be accompanied by visible copy stating that disabling it allows the agent to read image / PDF / binary files into its context AND that doing so bypasses the regex_basic PII filter (which only scans text). This copy SHALL be a security-conscious warning, not a neutral description, because the default is `true` (block) and disabling it weakens the PII safety floor.
12. Language (dropdown with exactly three options: "Auto", "中文", "English") mapping to `app.locale_override`. The "Auto" option SHALL write `null`, "中文" SHALL write `"zh"`, and "English" SHALL write `"en"`. The dropdown SHALL be positioned below the Endpoint Section and above the PII scanner field. The two non-Auto option labels ("中文" and "English") SHALL appear identically in both locales because they identify the language they select; only the "Auto" label and the field label itself SHALL be localized.

The Endpoint Section SHALL render a read-only `chat` row that displays the model and effort the `chat` verb inherits from the `query` verb, in the form "沿用 query（<model> / <effort>）", kept in sync with the editable `query` row. The `chat` row SHALL NOT be editable and SHALL NOT introduce any `chat`-specific configuration key.

No theme toggle and no per-vault override section SHALL be present. Sub-labels under fields SHALL NOT promise features absent from v1. The PII on-hit field SHALL display copy stating that Critical-severity matches are always masked regardless of this setting (the security floor cannot be disabled from the UI). The Quiz content verify and Goal content verify toggles SHALL each display copy stating that enabling them incurs additional verify/repair agent spawns.

The `save_global_config` IPC SHALL preserve every known and unknown subkey under any namespace it does not exclusively own. In particular, when enriching the `quiz` namespace with the resolved `default_length`, the IPC SHALL merge into the existing `quiz` object rather than replace it, so sibling keys (e.g. `quiz.content_verify`) set by the Settings UI survive a save→load round-trip. Unknown top-level YAML sections SHALL likewise continue to round-trip unchanged. The `hooks` namespace SHALL likewise round-trip through Save without losing unknown subkeys (forward-compat for future hook toggles). The `app` namespace SHALL likewise preserve unknown sibling subkeys when the Settings UI writes `app.locale_override`.

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

#### Scenario: Language dropdown is positioned and labeled correctly

- **WHEN** the user opens the Settings modal
- **THEN** a Language dropdown SHALL be present below the Endpoint Section AND above the PII scanner field, AND the dropdown SHALL offer exactly three options whose displayed strings are "Auto" (or its localized equivalent), "中文", and "English"

#### Scenario: Identifier-style language labels are not translated

- **GIVEN** the active locale is `"en"`
- **WHEN** the Settings modal renders the Language dropdown
- **THEN** the option labels for the two non-Auto values SHALL appear as "中文" and "English" verbatim, identical to how they appear when the active locale is `"zh"`

<!-- @trace
source: settings-language-switcher, pii-mirror-completeness
updated: 2026-06-26
code:
  - codebus-app/src/App.tsx
  - codebus-app/src/store/settings.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-app/src/components/settings/SettingsModal.tsx
tests:
  - codebus-app/src/components/settings/SettingsModal.test.tsx
-->

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
- Vault-specific settings override UI in the Settings modal
- Multi-AI-provider selection UI
- Quest banner, progress bar, or any "graduated" / "mastered" / "learned" page-level state in the Lobby or Workspace
- Tutorial slideshow UI, embedded checkpoints, or tutorial md generation triggers
- Telemetry, analytics, crash reporting, or auto-update channels
- A "Recent Pages" panel inside any sidebar
- Graph view entry in any sidebar
- Chat-mode Cmd+K with conversation memory (the overlay / command-palette UI itself is out of scope; no precursor UI element such as a centered Cmd+K spotlight, an input modal triggered by Cmd+K, or a Recent-Pages-style palette SHALL be added)
- Direct LLM API calls from the frontend (all agent interaction goes through `codebus-core`)
- Multiple concurrently-active goal runs within a single vault session (per the One Active Goal Run At A Time requirement in `app-workspace`)

A user-facing language override SHALL be permitted in the Settings modal as defined by "Settings Language Override" and "Global Settings Modal Field Set"; it is explicitly NOT a forbidden behavior.

A passive `⌘K` keyboard-shortcut chip rendered in the Workspace sidebar footer (per the `app-workspace` capability's "Workspace Sidebar Footer" requirement) SHALL be permitted; it labels the existing `useChatShortcut`-bound ChatWidget toggle and is NOT a Cmd+K overlay or command palette. The chip SHALL render only `<kbd>`-styled `⌘K` and SHALL NOT open a centered overlay or palette UI.

#### Scenario: Settings modal has no theme controls

- **WHEN** the user opens the Settings modal in any state
- **THEN** the rendered modal contains exactly the fields defined in "Global Settings Modal Field Set" (including the Language dropdown) plus the CLI Status row and Endpoint Section defined by their own requirements, AND no theme controls are present

#### Scenario: No telemetry network calls

- **WHEN** the codebus-app launches and runs through any Lobby or Settings flow
- **THEN** no outbound network requests are made by the app shell itself (LLM/agent invocations remain the responsibility of `codebus-core` and are out of scope for this change)

#### Scenario: Workspace sidebar ⌘K chip does not open a palette

- **WHEN** the user is in the Workspace and the sidebar footer's `⌘K` kbd chip is rendered
- **THEN** the chip displays only `<kbd>`-styled `⌘K` text AND no centered Cmd+K overlay or command-palette UI is mounted in the DOM as a result of the chip being rendered or hovered


<!-- @trace
source: workspace-sidebar-rework
updated: 2026-05-27
code:
  - codebus-app/src/store/quiz-history.ts
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/App.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/store/quiz-history.test.ts
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

#### Scenario: 7-pattern sweep finds no policy violations

- **WHEN** a reviewer or maintainer runs the canonical 7-pattern grep sweep (Patterns 1a, 1b, 1c, 2, 3, 4 against `codebus-app/src/components/**/*.tsx`; Pattern 5 against `codebus-app/src/**/*.{ts,tsx}` for template-literal interpolation with Latin neighbours; Pattern 6 against `codebus-app/src/**/*.ts` outside `components/` for helper / lib files)
- **THEN** every reported line MUST resolve to one of: (a) a `t("...")` call, (b) an entry from the Cat D jargon allow-list, (c) a Claude API tool name identifier from the non-UI exclusion list, or (d) a documented runtime-keyword identifier such as the re-init confirmation literal `delete` in `NewVaultFlow.tsx`. Any unaccounted line constitutes a policy violation requiring a follow-up change.

##### Example: canonical 7-pattern sweep commands

| # | Purpose | Command (run from `codebus-app/`) |
| - | ------- | --------------------------------- |
| 1a | JSX text content with Latin (single-line) | `grep -rPn '>([^<{]*[A-Za-z]+[^<{]*)<' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -vE 't\("\|\{t\(\|className=\|data-testid='` |
| 1b | Indented JSX text (multi-line) | `grep -rPn "^[[:space:]]+[A-Z][a-zA-Z][a-zA-Z' ]*[a-zA-Z][\.…!\?]?[[:space:]]*$" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 1c | JSX text with Latin split by `{}` interpolation | `grep -rPn '>[^<>{}]*\{[^}]+\}[^<>{}]*[A-Za-z]+[^<>]*<' src/components/ --include='*.tsx' \| grep -v '.test.' \| grep -v 't("'` |
| 2 | Emoji / arrow-prefixed Latin string | `grep -rPn '[←→↻⏹⚠✓✕▸▿⤢⤡⏺] [A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 3 | Untranslated `aria-label` / `title` / `placeholder` attrs | `grep -rPn '(aria-label\|title\|placeholder)="[^"]*[A-Za-z]' src/components/ --include='*.tsx' \| grep -v '.test.tsx' \| grep -v 't("'` |
| 4 | String literals with placeholder syntax | `grep -rPn "'[A-Za-z][^']*\{\w+\}" src/components/ --include='*.tsx' \| grep -v '.test.tsx'` |
| 5 | Template-literal interpolation with Latin neighbour | `grep -rPn "\`[^\`]*\$\{[^}]+\}[^\`]*[A-Za-z]" src/ --include='*.ts' --include='*.tsx' \| grep -v '.test.' \| grep -v 't("'` |
| 6 | Helper / lib hard-codes outside `components/` | Re-run Patterns 1a / 1b / 1c / 2 / 3 / 4 with `src/` as the search root and `--include='*.ts' --include='*.tsx'`, then exclude paths under `src/components/` (already covered by Patterns 1-4 and 1c) and any `.test.` files |

Pattern 1 was originally written as `>(?![\s{<])([^<{]*[A-Za-z]){2,}[^<{]*<` requiring two or more Latin chunks; that excluded single-word JSX text such as `Checking…` and `Loading…` (Phase 3A residual sweep 2026-05-26). Pattern 1 is now split into 1a (single-line JSX node) and 1b (multi-line JSX text whose Latin word lives on its own indented line) to cover both shapes.

Pattern 1c was added in the Phase 3A blind-spots cleanup (2026-05-27) after the `settings-language-switcher` apply step surfaced JSX text such as `<span>Install {provider.displayName} first; then reopen Settings.</span>` in `SettingsModal.tsx`. Pattern 1a and 1b fail on this shape because the Latin word run is split by a `{}` interpolation into short fragments that fall below Pattern 1a's contiguous-Latin threshold; Pattern 1c targets JSX text nodes that contain at least one `{}` interpolation surrounded by Latin neighbours, so interpolation-split copy is no longer invisible to the sweep.

Patterns 5 and 6 were added in the Phase 3A follow-up sweep (2026-05-26) after CDP en-locale smoke surfaced template-literal hard-codes such as `` `${diffHr}h ago` `` in `RunListItem.tsx` and helper-produced strings such as the pass / fail verdict in `src/lib/quiz-parse.ts` that the original 4-pattern sweep could not match. Pattern 5 catches strings inside backtick literals (no `<>` brackets, no symbol prefix, no attribute prefix, double quotes only). Pattern 6 widens the search root from `src/components/` to all of `src/` so that helper modules, formatters, and other `.ts` files outside `components/` are covered.

JSX text starting with non-Latin punctuation (e.g. `+ New goal`, `+ New chat`) remains a known gap of Patterns 1a, 1b, and 1c — those button labels lose the `[A-Z]` anchor and Latin-chunk count requirement, so they SHALL be discovered by manual CDP smoke or by reviewer pattern-matching rather than by automated sweep. Future follow-up changes MAY introduce Pattern 7 for that shape if recurring instances justify it.

`.ts` layer plain-string user-facing error data (e.g. validation message objects returned from `src/lib/ipc.ts` to React form components) SHALL NOT be detected by sweep patterns, because semantic grep on shapes like `message: "<Latin>"` in `.ts` files produces high false-positive volume (internal log messages, typedef literal defaults, non-user-facing error subclass message arguments, and similar developer-facing strings that the policy explicitly excludes per the non-UI exclusion list above). Such user-facing error sites SHALL instead be guarded architecturally: error data carried from `ipc.ts` to React user-facing surfaces SHALL use a `LocalizedError`-shaped contract (`{key: MessageKey, vars?: Record<string, string | number>}` as defined in `codebus-app/src/i18n/errors.ts`), and TypeScript SHALL fail to compile if a new user-facing error site stores a plain `string` message in place of `{key, vars}`. Internal-only error data (developer console output, log records) MAY continue to carry plain `string` fields without violating this policy.

##### Example: Pattern 1c catches interpolation-split JSX copy

- **GIVEN** a component renders `<span>Install {provider.displayName} first; then reopen Settings.</span>`
- **WHEN** Pattern 1a is run alone, the contiguous-Latin window (`Install ` and ` first; then reopen Settings.`) on either side of the `{provider.displayName}` interpolation is split into fragments that individually fail the contiguous-Latin threshold
- **THEN** Pattern 1c MUST report this line because the `>` ... `<` JSX text region contains a `{}` interpolation flanked by `[A-Za-z]+` neighbours

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


<!-- @trace
source: phase-3a-blind-spots-cleanup
updated: 2026-05-27
code:
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/settings/EndpointSection.tsx
  - codebus-app/src/components/settings/CodexEndpointSection.tsx
  - codebus-app/design-handoff/AUDIT.md
tests:
  - codebus-app/src/lib/ipc.validation-i18n.test.ts
  - codebus-app/src/components/settings/EndpointSection.test.tsx
  - codebus-app/src/lib/ipc.effort.test.ts
  - codebus-app/src/components/settings/CodexEndpointSection.test.tsx
-->

---
### Requirement: Settings Language Override

The codebus-app SHALL persist a user-selected locale override at `app.locale_override` in `~/.codebus/config.yaml` with three valid values: `"zh"`, `"en"`, or `null`. A `null` value (or an absent key, including in configs written by earlier versions) SHALL mean "auto-detect from the system locale".

The `useLocale` hook (`codebus-app/src/hooks/useLocale.ts`) SHALL resolve the active locale by this precedence, evaluated top-down on every render:

1. The `override` argument passed to `useLocale(override?: Locale)`, when non-nullish — this path SHALL remain available for tests to inject a deterministic locale
2. The `app.locale_override` value read reactively from the settings store, when non-`null`
3. Otherwise, `navigator.language`: a value beginning with `zh` (case-insensitive) SHALL resolve to `"zh"`; any other value (including when `navigator` is undefined) SHALL resolve to `"en"`

The settings store SHALL expose `app.locale_override` such that React components subscribing via the store hook re-render when the value changes, so changing the language selection in the Settings modal SHALL take effect immediately without restarting the application or remounting the React tree.

Changes to `app.locale_override` SHALL persist through the existing `save_global_config` / `load_global_config` IPC round-trip, so the selected locale SHALL be sticky across application restarts. Backend errors surfaced through `LocalizedError` (`codebus-app/src/i18n/errors.ts`) SHALL render in the active locale because the toast layer resolves them through `useT` / `useLocale` at display time; this requirement SHALL NOT require any imperative locale lookup in `errors.ts`.

A standalone synchronous helper `tStatic` that resolves locale outside the React tree is out of scope for this requirement and MAY continue to read `navigator.language` directly until a follow-up change wires it to the store.

#### Scenario: Language dropdown switches the UI reactively

- **GIVEN** the user has the Settings modal open and the active locale is `"zh"`
- **WHEN** the user selects "English" in the Language dropdown
- **THEN** the Settings modal contents, the Workspace background, and the Lobby background re-render in English without any restart, remount, or page reload

#### Scenario: Locale override survives application restart

- **GIVEN** the user has set the Language dropdown to "English" and clicked Save
- **WHEN** the user closes and relaunches the codebus-app
- **THEN** `~/.codebus/config.yaml` contains `app.locale_override: "en"` AND the relaunched app renders in English regardless of the system locale

#### Scenario: Auto option follows the system locale

- **GIVEN** `navigator.language` resolves to `zh-TW`
- **WHEN** the user sets the Language dropdown to "Auto" and clicks Save
- **THEN** `~/.codebus/config.yaml` contains `app.locale_override: null` AND the active locale resolves to `"zh"`

#### Scenario: Backend error toast follows the active locale

- **GIVEN** the user has the Language dropdown set to "English" and has saved
- **WHEN** the user triggers a backend error from the Settings modal (for example by submitting an invalid endpoint base URL)
- **THEN** the resulting toast renders the error message in English

#### Scenario: Hook override argument outranks the store

- **GIVEN** `app.locale_override` in the settings store is `"en"`
- **WHEN** a component calls `useLocale("zh")` directly (typically a test injecting a deterministic locale)
- **THEN** the call SHALL return `"zh"`

##### Example: Precedence resolution table

| Hook arg `override` | Store `locale_override` | `navigator.language`  | Resolved locale |
| ------------------- | ---------------------- | --------------------- | --------------- |
| `"zh"`              | `"en"`                 | `en-US`               | `"zh"`          |
| `undefined`         | `"en"`                 | `zh-TW`               | `"en"`          |
| `undefined`         | `null`                 | `zh-TW`               | `"zh"`          |
| `undefined`         | `null`                 | `en-US`               | `"en"`          |
| `undefined`         | `null`                 | `fr-FR`               | `"en"`          |
| `undefined`         | `null`                 | (navigator undefined) | `"en"`          |

#### Scenario: Legacy config without locale_override round-trips safely

- **GIVEN** `~/.codebus/config.yaml` was written by a version before this change and contains no `app.locale_override` key
- **WHEN** the codebus-app loads the config and the user later opens Settings, makes no language change, and clicks Save
- **THEN** the load SHALL succeed AND the active locale SHALL be derived from `navigator.language` AND the saved config SHALL preserve all other existing keys unchanged

<!-- @trace
source: settings-language-switcher
updated: 2026-05-26
code:
  - codebus-app/scripts/.lang-switcher-smoke/step-4-back-to-auto.png
  - codebus-app/src/lib/ipc.ts
  - codebus-app/src/components/settings/LanguageSection.tsx
  - codebus-app/scripts/.lang-switcher-smoke/step-2-switch-en.png
  - codebus-app/src/components/settings/SettingsModal.tsx
  - codebus-app/src/App.tsx
  - codebus-app/src-tauri/src/config.rs
  - codebus-app/src-tauri/src/ipc/config.rs
  - codebus-app/scripts/.lang-switcher-smoke/step-1-default-zh.png
  - codebus-app/scripts/.lang-switcher-smoke/step-5-backend-error-en.png
  - codebus-app/src/i18n/messages.ts
  - codebus-app/scripts/.lang-switcher-smoke/step-3-restart-en.png
  - codebus-app/src/hooks/useLocale.ts
tests:
  - codebus-app/src/App.test.tsx
  - codebus-app/src/components/settings/SettingsModal.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/i18n/errors.test.tsx
  - codebus-app/src/lib/ipc.localeOverride.test.ts
  - codebus-app/src/store/settings.localeOverride.test.ts
  - codebus-app/src/hooks/useLocale.test.tsx
  - codebus-app/src/components/settings/LanguageSection.test.tsx
  - codebus-app/src/i18n/settings.test.ts
-->

---
### Requirement: Lobby Empty State Hero Motion

The empty-state hero bus emoji SHALL render a continuous cyclic motion: the bus glyph SHALL be horizontally mirrored via `transform: scaleX(-1)` so the bus faces left, and SHALL translate along the horizontal axis from approximately -50 pixels to approximately +50 pixels (a total traversal of approximately 100 pixels), with a vertical bumpy displacement of approximately -3 pixels at the mid-traversal keyframes, and a rotation oscillating within approximately ±2 degrees. The motion SHALL loop with a duration of approximately 2.5 seconds and SHALL follow a dwell-and-return phasing in which the bus advances to the far endpoint, dwells briefly at that endpoint, and then returns to the starting endpoint before repeating. The motion SHALL be implemented as pure CSS keyframes; no JavaScript animation library SHALL be introduced for this effect.

When the user agent advertises `prefers-reduced-motion: reduce`, the cyclic motion SHALL be suppressed and the bus emoji SHALL remain completely static; no transform animation SHALL be applied.

The cyclic motion SHALL be confined to the empty-state hero only; the topbar bus wordmark glyph SHALL remain static, and no other Lobby element SHALL animate as a consequence of this requirement. The LoadingOverlay bus animation defined elsewhere SHALL NOT be affected by this requirement.

#### Scenario: Hero bus animates with mirrored cyclic motion by default

- **WHEN** a user opens the Lobby in empty state on a system that does not advertise `prefers-reduced-motion: reduce`
- **THEN** the hero bus emoji SHALL render horizontally mirrored (facing left) AND SHALL visibly translate horizontally across an approximately 100-pixel span (-50px to +50px) within a continuous loop of approximately 2.5 seconds, with a visible rotation oscillation within approximately ±2 degrees and a bumpy vertical displacement of approximately -3 pixels at the mid-traversal keyframes, AND SHALL exhibit a dwell-and-return phasing in which the bus pauses briefly at the far endpoint before returning to the starting endpoint

##### Example: motion keyframe progression

| Keyframe | Horizontal | Vertical | Rotation | Notes |
| -------- | ---------- | -------- | -------- | ----- |
| 0% / 100% | -50px | 0 | -2deg | start / loop boundary |
| 20% | -25px | -3px | 0deg | mid forward bump |
| 45% | +15px | 0 | +2deg | crossing center |
| 65% | +50px | -3px | 0deg | far endpoint reached |
| 75% | +50px | 0 | -1deg | dwell at endpoint |
| (75% → 100%) | +50px → -50px | 0 | -1deg → -2deg | return arc |

#### Scenario: Reduced-motion preference disables cyclic motion

- **WHEN** the user agent advertises `prefers-reduced-motion: reduce`
- **THEN** the hero bus emoji SHALL render with no transform animation applied; computed `animation-name` SHALL be `none` (or equivalent)

#### Scenario: Cyclic motion is scoped to empty-state hero

- **WHEN** the cyclic motion is active in the empty state
- **THEN** the topbar bus wordmark glyph SHALL render statically AND the LoadingOverlay bus animation SHALL retain its own single-direction non-mirrored keyframes AND every other Lobby element SHALL render without any motion attributable to this requirement

<!-- @trace
source: lobby-hero-motion-revise
updated: 2026-05-28
code:
  - codebus-app/design-handoff/AUDIT.md
  - codebus-app/src/components/lobby/VaultCard.tsx
  - codebus-app/src/components/lobby/Lobby.tsx
  - codebus-app/scripts/cdp.mjs
  - codebus-app/src/components/lobby/EmptyState.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/styles/globals.css
tests:
  - codebus-app/src/components/lobby/VaultCard.test.tsx
  - codebus-app/src/components/lobby/EmptyState.test.tsx
-->

---
### Requirement: Settings Modal Invocation From Workspace Sidebar Footer

The Settings modal SHALL be reachable from the Workspace state via a Settings icon button rendered in the Workspace sidebar footer (see `app-workspace` capability: "Workspace Sidebar Footer"). Clicking this Settings icon button SHALL open the same single application-shell `<SettingsModal>` instance that the Lobby BottomStrip's bottom-left gear opens, sharing a single `settingsOpen` state owned by the application shell. The Workspace SHALL receive the open-settings callback via a prop from the application shell and SHALL NOT mount its own `<SettingsModal>` instance.

The Settings modal's field set, save behavior, validation, and IPC contract (defined in "Global Settings Modal Field Set") are unaffected by the invocation source; only the entry point in the Workspace state SHALL move from the BottomStrip gear (now hidden in the Workspace route per "Lobby Two-State Rendering") to the Workspace sidebar footer.

#### Scenario: Workspace sidebar Settings icon opens the application-shell Settings modal

- **GIVEN** the user is in the Workspace and the bottom strip is not rendered
- **WHEN** the user clicks the Settings icon button in the Workspace sidebar footer
- **THEN** the application-shell `<SettingsModal>` opens centered over a dimmed Workspace background, identical in identity and field set to the modal opened by the Lobby BottomStrip's bottom-left gear

#### Scenario: Lobby and Workspace share a single Settings modal instance

- **GIVEN** the application shell exposes one `settingsOpen` state and one `<SettingsModal>` element
- **WHEN** the user opens Settings from the Lobby BottomStrip gear, closes it, navigates to the Workspace, and opens Settings from the sidebar footer
- **THEN** both invocations toggle the same `settingsOpen` state and render the same `<SettingsModal>` instance (no duplicate modal element is mounted in the DOM)

<!-- @trace
source: workspace-sidebar-rework
updated: 2026-05-27
code:
  - codebus-app/src/store/quiz-history.ts
  - codebus-app/src/App.tsx
  - codebus-app/src/components/workspace/Workspace.tsx
tests:
  - codebus-app/src/components/workspace/Workspace.test.tsx
  - codebus-app/src/App.test.tsx
  - codebus-app/src/test/forbidden-behaviors.test.tsx
  - codebus-app/src/store/quiz-history.test.ts
-->

---
### Requirement: Vault Init Progress Event

The system SHALL emit a Tauri event named `vault-init-progress` from the Rust side of `codebus-app` while `add_vault` runs an init-heavy branch (`AddVaultMode::Detect` against a folder without `.codebus/` OR `AddVaultMode::ReInit`). The event payload SHALL be a struct serialised with `serde(rename_all = "snake_case")` carrying three fields: `phase` (integer 1..=6), `init_event_kind` (string, the InitEvent variant debug label such as `"Start"` or `"LayoutCreated"`), and `elapsed_ms` (unsigned integer milliseconds since `add_vault_at` started). The `phase` value SHALL be derived from the InitEvent variant by a Tauri-layer function (not by `codebus-core`) using the authoritative mapping below; the frontend SHALL NOT interpret `init_event_kind` for layout decisions.

The system SHALL change `add_vault_at` from a synchronous function into an asynchronous function that accepts a `tauri::AppHandle`; the existing `add_vault` Tauri command and any internal test callers SHALL be updated accordingly. The `codebus-core` `InitEvent` enum, `run_init` signature semantics, and existing variants SHALL NOT be modified by this change.

The InitEvent variant to phase mapping SHALL be:

- Phase 1: `Start`, `LayoutCreated`, `SourceGitignore`.
- Phase 2: `PiiConfigLoadWarn`, `PiiPatternsExtraWarn`, `RawSyncDone`.
- Phase 3: `InternalGitignoreDone`, `NestedRepoDone`.
- Phase 4: `SchemaDone`, `ManifestSignal`, `ManifestDone`, `SkillBundlesDone`, `NavStubsDone`, `SettingsDone`.
- Phase 5: `ObsidianResult`, `ObsidianSkipped`.
- Phase 6: `StarterConfigUnavailable`, `StarterConfigDone`, `StarterConfigError`, `CommitDone`, `Finished`.

The mapping function SHALL use an exhaustive `match` over all `InitEvent` variants (no catch-all arm) so that adding a new variant to `codebus-core::vault::init::InitEvent` produces a compile-time error in `codebus-app` until the mapping is updated.

#### Scenario: Detect-mode add emits one event per InitEvent

- **WHEN** the user invokes the New Vault flow on a folder with no `.codebus/` directory
- **THEN** the Rust side SHALL emit one `vault-init-progress` event per InitEvent that `run_init` produces AND each payload's `phase` field SHALL match the mapping above for the corresponding InitEvent variant AND `elapsed_ms` SHALL be monotonic non-decreasing across events of a single add operation

#### Scenario: Re-init mode emits events from the second run_init call

- **GIVEN** a folder that already contains `.codebus/` and the user picks the Re-initialize destructive option with the typed `delete` confirmation
- **WHEN** the system removes the existing `.codebus/` and calls `run_init` for the fresh init
- **THEN** the `vault-init-progress` event stream SHALL cover the InitEvents emitted by the fresh `run_init` call with the same phase mapping AND no events from the removed directory's history SHALL be emitted

#### Scenario: Just-bind mode emits no progress events

- **WHEN** the user picks the Just-Bind option on a folder that already contains `.codebus/`
- **THEN** the system SHALL NOT call `run_init` AND SHALL NOT emit any `vault-init-progress` event AND `add_vault` SHALL still return the new `VaultEntry`

#### Scenario: Unknown InitEvent variant is a compile-time error

- **GIVEN** a future change adds a new variant `NewlyAdded` to the `InitEvent` enum in `codebus-core::vault::init`
- **WHEN** `codebus-app` is rebuilt without updating the Tauri-layer mapping function
- **THEN** the Rust compiler SHALL emit a `non-exhaustive patterns` error in the mapping function rather than silently routing the new variant to a default phase

##### Example: phase mapping table

| InitEvent variant         | Emitted phase |
| ------------------------- | ------------- |
| `Start`                   | 1             |
| `LayoutCreated`           | 1             |
| `SourceGitignore`         | 1             |
| `PiiConfigLoadWarn`       | 2             |
| `PiiPatternsExtraWarn`    | 2             |
| `RawSyncDone`             | 2             |
| `InternalGitignoreDone`   | 3             |
| `NestedRepoDone`          | 3             |
| `SchemaDone`              | 4             |
| `ManifestSignal`          | 4             |
| `ManifestDone`            | 4             |
| `SkillBundlesDone`        | 4             |
| `NavStubsDone`            | 4             |
| `SettingsDone`            | 4             |
| `ObsidianResult`          | 5             |
| `ObsidianSkipped`         | 5             |
| `StarterConfigUnavailable`| 6             |
| `StarterConfigDone`       | 6             |
| `StarterConfigError`      | 6             |
| `CommitDone`              | 6             |
| `Finished`                | 6             |


<!-- @trace
source: loading-overlay-live-progress
updated: 2026-05-28
code:
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/main.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/src/App.tsx
  - codebus-app/src/store/vaults.ts
  - codebus-app/src/components/PhaseDots.tsx
  - codebus-app/src-tauri/src/ipc/vault_progress.rs
  - codebus-app/src-tauri/src/ipc/vault_list.rs
  - codebus-app/src/components/LoadingOverlay.tsx
tests:
  - codebus-app/src/components/PhaseDots.test.tsx
  - codebus-app/src/components/LoadingOverlay.test.tsx
-->

---
### Requirement: LoadingOverlay Live Progress

The `LoadingOverlay` component SHALL render while `useVaultsStore.initInProgress` is `true` and SHALL listen for the `vault-init-progress` Tauri event. The overlay SHALL maintain a frontend state machine with a `phase` value (0..6, default 0) and a `failed` boolean (default false). The bus emoji animated by `@keyframes codebus-bus-roll` SHALL remain mounted across all phase transitions; the component SHALL NOT remount the bus element when the phase changes.

When `phase === 0` (no `vault-init-progress` event received yet) the overlay SHALL render the v1 fallback content: the existing `loading.title` and `loading.subtitle` i18n strings together with the bus animation, and SHALL NOT render the phase-dots indicator. When `phase >= 1` the overlay SHALL render the existing `loading.title`, the phase-specific subtitle from `loading.phase.{phase}.title`, and a 6-dot indicator using the shared `PhaseDots` component (extracted from `QuizTab.StepDots`) with `total={6}` and `current={phase}`. The `loading.title` and `loading.subtitle` existing i18n keys SHALL NOT be renamed nor have their values modified by this change.

The state machine SHALL enforce a minimum 300 ms residence time per phase: when an incoming `vault-init-progress` event would advance the phase, the component SHALL delay the visible transition until at least 300 ms have elapsed since the previous phase became visible, queueing the pending phase value in the meantime. Backend events that arrive faster than 300 ms SHALL NOT be dropped — only the visible subtitle / dots update is debounced. When the `add_vault` IPC call resolves successfully (regardless of which phase the last event reported), the overlay SHALL render at phase 6 for at least 300 ms and then fade out over 200 ms before unmounting.

If `add_vault` IPC rejects with an error, the overlay SHALL enter failure mode: the `codebus-bus-roll` animation SHALL be paused, the title SHALL switch to `loading.error.title`, the subtitle SHALL display the rejected `LocalizedError`'s message string, the `PhaseDots` SHALL keep `total={6}` and `current` at the last reached phase with `state="error"` (the current dot SHALL render in `--color-warn` to match the 02c Interrupted banner), and a retry button labeled `loading.error.retry` SHALL appear. The retry button SHALL re-dispatch the same `addVault` call (mode and path unchanged from the failed attempt). The failure styling SHALL use the amber-warm `--color-warn` token and SHALL NOT use a hard-fail red color.

When the visible phase has not advanced for more than 20 000 ms (single-phase stall), the overlay SHALL render the `loading.slow.hint` text in a dim style directly below the phase subtitle. The hint SHALL disappear when the next phase becomes visible.

The component SHALL define and use the following new i18n keys in both `zh` and `en` of `codebus-app/src/i18n/messages.ts`, in addition to the existing `loading.title` and `loading.subtitle`:

- `loading.phase.1.title`, `loading.phase.2.title`, `loading.phase.3.title`, `loading.phase.4.title`, `loading.phase.5.title`, `loading.phase.6.title`
- `loading.error.title`, `loading.error.retry`
- `loading.slow.hint`

#### Scenario: Initial mount shows fallback before any event

- **WHEN** `useVaultsStore.initInProgress` flips to `true` and no `vault-init-progress` event has been received
- **THEN** the overlay renders the existing `loading.title` and `loading.subtitle` strings with the bus animation running AND no element with `data-testid="loading-overlay-phase-dots"` is mounted

#### Scenario: Phase advances on event

- **GIVEN** the overlay is rendering at phase 1 with subtitle `loading.phase.1.title`
- **WHEN** a `vault-init-progress` event with `phase: 2` arrives more than 300 ms after the overlay entered phase 1
- **THEN** the overlay updates the subtitle to `loading.phase.2.title` AND the second dot is marked active AND the bus emoji element is not remounted (same DOM node identity)

#### Scenario: Backend skips phase 5 but UI still pauses

- **GIVEN** the overlay is rendering at phase 4
- **WHEN** the backend emits `ObsidianSkipped` (phase 5) and `CommitDone` (phase 6) within 50 ms of each other
- **THEN** the overlay renders phase 5 subtitle `loading.phase.5.title` for at least 300 ms before transitioning to phase 6 subtitle `loading.phase.6.title`

#### Scenario: Successful finish fades out

- **GIVEN** the overlay is rendering at phase 6
- **WHEN** the `add_vault` IPC resolves with `Ok(VaultEntry)`
- **THEN** the overlay opacity transitions from 1 to 0 over 200 ms AND after the transition completes the overlay is removed from the DOM AND the Workspace for the new vault is now visible

#### Scenario: Backend error enters failure mode

- **GIVEN** the overlay is rendering at phase 3
- **WHEN** the `add_vault` IPC rejects with an `AppError` whose `LocalizedError.message` resolves to "Permission denied writing to .codebus/"
- **THEN** the bus animation pauses AND the title is `loading.error.title` AND the subtitle is "Permission denied writing to .codebus/" AND the third dot renders in the `--color-warn` token AND a retry button with label `loading.error.retry` is visible

#### Scenario: Retry re-dispatches the same add_vault call

- **GIVEN** the overlay is in failure mode after an `AddVaultMode::Detect` failure on path `/Users/alice/repo`
- **WHEN** the user clicks the retry button
- **THEN** the overlay re-invokes `useVaultsStore.addVault("/Users/alice/repo", "detect")` AND the state machine resets to phase 0 and `failed=false` while the new IPC is in flight

#### Scenario: Slow phase shows dim hint

- **GIVEN** the overlay has been rendering at phase 4 for 19 500 ms with no new `vault-init-progress` event
- **WHEN** another 500 ms elapses without an event
- **THEN** the overlay renders a dim hint element with text `loading.slow.hint` directly below the phase subtitle AND the hint disappears when the next `vault-init-progress` event causes the visible phase to advance

#### Scenario: Backend never emits events but IPC succeeds

- **GIVEN** the `vault-init-progress` event listener is never invoked during an `add_vault` call (regression or rollout gap)
- **WHEN** `add_vault` resolves with `Ok(VaultEntry)`
- **THEN** the overlay fades out over 200 ms from its phase-0 fallback render AND no error UI is shown AND the Workspace becomes visible

#### Scenario: Quiz wizard step dots continue to work

- **GIVEN** the `QuizTab` previously rendered four step dots through a local `StepDots` function
- **WHEN** the local function is replaced by the shared `PhaseDots` component with `total={4}` and `current={wizardStep}`
- **THEN** the rendered element continues to expose `data-testid="quiz-wizard-step-dots"` AND the `data-current-step` attribute reflects the active step value as before

<!-- @trace
source: loading-overlay-live-progress
updated: 2026-05-28
code:
  - codebus-app/src-tauri/src/ipc/mod.rs
  - codebus-app/src/main.tsx
  - codebus-app/src/i18n/messages.ts
  - codebus-app/src/components/workspace/QuizTab.tsx
  - docs/2026-05-28-codex-hook-hard-gate-spike.md
  - codebus-app/src/App.tsx
  - codebus-app/src/store/vaults.ts
  - codebus-app/src/components/PhaseDots.tsx
  - codebus-app/src-tauri/src/ipc/vault_progress.rs
  - codebus-app/src-tauri/src/ipc/vault_list.rs
  - codebus-app/src/components/LoadingOverlay.tsx
tests:
  - codebus-app/src/components/PhaseDots.test.tsx
  - codebus-app/src/components/LoadingOverlay.test.tsx
-->
