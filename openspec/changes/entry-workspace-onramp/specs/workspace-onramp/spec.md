## ADDED Requirements

### Requirement: Entry page exposes folder-picker workspace onramp

The frontend SHALL replace the Phase 6 step 25.5 `<AppShell>` ping-smoke placeholder at `/` (`web/app/pages/index.vue`) with an entry shell that lets a configured user (one whose `/healthz.dependency.llm_chat` and `llm_embed` both report `ready`) launch a native folder picker and start a workspace onramp.

The entry page MUST preserve the existing onboarding redirect behavior: on mount it MUST poll `/healthz` and route to `/onboarding/welcome` if either LLM lane reports `not-configured` (this is the same guard `phase7-onboarding-polish` shipped). Only when both lanes report `ready` does the onramp UI render.

The onramp surface MUST contain at minimum:

1. A `<FolderPickerButton>` element labeled "+ 開新 codebase" that, on click, invokes the `tauri-plugin-dialog` plugin's folder-picker IPC.
2. A `<WorkspaceOnrampCard>` element that, after a folder is picked, displays the resolved `workspace_id`, the selected path's tail directory name, and the current onramp phase (idle / scanning / indexing / generating / ready / error).
3. An `<OnrampProgress>` strip rendered while the SSE task is in flight, showing the current phase plus a throughput counter (e.g. "scanned 142 files" / "indexed 38 chunks").

#### Scenario: Entry page renders onramp UI when both LLM lanes are ready

- **WHEN** the user lands on `/` and `/healthz.dependency.llm_chat === 'ready'` and `llm_embed === 'ready'`
- **THEN** the page MUST render `<FolderPickerButton>` with `data-testid="onramp-folder-picker"` enabled
- **AND** the page MUST NOT redirect to any onboarding route

#### Scenario: Entry page redirects to onboarding when an LLM lane is not configured

- **WHEN** the user lands on `/` and either `/healthz.dependency.llm_chat === 'not-configured'` or `llm_embed === 'not-configured'`
- **THEN** the page MUST replace the current route with `/onboarding/welcome`
- **AND** the onramp UI MUST NOT render

---
### Requirement: Folder picker invocation flow

When the user clicks `<FolderPickerButton>`, the frontend SHALL invoke `tauri-plugin-dialog`'s `open({ directory: true, multiple: false })` IPC. The Tauri host SHALL be configured (in `tauri/src-tauri/capabilities/default.json`) with the `dialog:default` permission so the plugin's open IPC is callable from the renderer. Cancellation (the user closes the picker without selecting) MUST leave the onramp state unchanged — no error message, no transition.

The selected absolute path MUST be passed to the onramp composable, which derives the `workspace_id` locally via `web/app/utils/workspace-id.ts::deriveWorkspaceId(path)` (a SHA-256 of the canonical lowercased POSIX path, parity with sidecar `auth.service.workspace_id_for_path`).

#### Scenario: User cancels folder picker

- **WHEN** the user clicks `<FolderPickerButton>` and closes the OS folder dialog without selecting a directory
- **THEN** the onramp phase MUST remain `idle`
- **AND** no error message MUST be shown
- **AND** subsequent clicks of `<FolderPickerButton>` MUST still open the dialog

#### Scenario: Selected path produces deterministic workspace_id

- **WHEN** the picker returns an absolute path `P` for the first time
- **THEN** `deriveWorkspaceId(P)` MUST return a 15-character string starting with `ws_` followed by 12 lowercase hex characters
- **AND** invoking `deriveWorkspaceId(P)` again later in the same session MUST return the exact same string
- **AND** if a second invocation passes the same path with different case (e.g. `C:/Foo` vs `c:/foo` on Windows) the derived id MUST be identical

---
### Requirement: Workspace onramp drives scan then generate via SSE

After `deriveWorkspaceId` produces an id, the onramp composable SHALL POST to the sidecar's `/scan` endpoint with body shape `{ workspace_root: <path>, workspace_type: "folder" }` and subscribe to the returned task's SSE event stream. While the scan task is in flight `<OnrampProgress>` MUST surface phase + counters from the SSE events.

When the scan task emits its terminal `done` event, the onramp composable SHALL transition to phase `scan-complete` and render a "+ 產生 tutorial" button inside `<WorkspaceOnrampCard>`. Clicking the button SHALL POST to `/generate` with body `{ workspace_id: <derived_id> }` and again drive `<OnrampProgress>` from that task's SSE stream.

When `/generate` emits `done`, the onramp transitions to phase `ready` and renders a "進入 tutorial" CTA whose target is `/tutorial/<workspace_id>`. The CTA MUST NOT auto-navigate; the user MUST click it.

#### Scenario: Scan terminal event unlocks generate CTA

- **WHEN** the SSE stream of the in-flight `/scan` task emits a `done` event
- **THEN** the onramp phase MUST transition to `scan-complete`
- **AND** `<WorkspaceOnrampCard>` MUST render a button with `data-testid="onramp-generate-cta"`
- **AND** clicking that button MUST issue a `POST /generate` with `workspace_id` matching the derived id

#### Scenario: Generate terminal event renders enter-tutorial CTA

- **WHEN** the SSE stream of the in-flight `/generate` task emits a `done` event
- **THEN** the onramp phase MUST transition to `ready`
- **AND** `<WorkspaceOnrampCard>` MUST render an anchor with `data-testid="onramp-enter-tutorial"` whose `href` resolves to `/tutorial/<workspace_id>`
- **AND** the anchor MUST NOT trigger automatic navigation; the user MUST click it

#### Scenario: SSE error pauses onramp with retry affordance

- **WHEN** an SSE event of the in-flight scan or generate task carries an `error` field
- **THEN** the onramp phase MUST transition to `error`
- **AND** the error message MUST be displayed inside `<WorkspaceOnrampCard>` (no silent log-only failure)
- **AND** a button with `data-testid="onramp-retry"` MUST render that re-issues the same POST when clicked
- **AND** the user's selected path / derived id MUST remain visible (the user MUST NOT have to re-pick the folder)

---
### Requirement: Onramp state survives navigation away from entry page

The `useWorkspaceOnramp` composable SHALL be a module-level singleton (matching the pattern used by `useQaSession` and `useIntervention`) so that scan / generate state is preserved when the user navigates to `/settings` or `/audit/*` and back to `/`. The active SSE subscription MUST NOT be torn down on `/` unmount.

#### Scenario: User leaves entry mid-scan and returns

- **WHEN** the onramp is in phase `scanning` and the user navigates to `/settings`
- **THEN** the SSE subscription to the scan task MUST remain active
- **AND** when the user navigates back to `/`, `<OnrampProgress>` MUST resume rendering with the latest counter values
- **AND** the onramp phase MUST reflect any progression that happened during the absence (e.g. `scan-complete` if scan finished while away)

---
### Requirement: AppShell ping-smoke placeholder is removed

The Phase 6 step 25.5 `web/app/components/AppShell.vue` ping-smoke placeholder SHALL be removed from the codebase as part of this change. Its sole import site is `web/app/pages/index.vue`, which this change rewrites; no other component references it (verified by repo-wide grep at task time).

#### Scenario: AppShell.vue is deleted and not referenced

- **WHEN** the source tree is searched for the substring `AppShell`
- **THEN** no `.vue` / `.ts` file under `web/app/` MUST contain that token
- **AND** the file `web/app/components/AppShell.vue` MUST NOT exist

---
### Requirement: Tauri host wires the dialog plugin

The Tauri host SHALL declare `tauri-plugin-dialog = "2"` in `tauri/src-tauri/Cargo.toml`, register the plugin in `tauri/src-tauri/src/lib.rs` via `.plugin(tauri_plugin_dialog::init())`, and grant `dialog:default` permission in `tauri/src-tauri/capabilities/default.json`. The web side SHALL declare `@tauri-apps/plugin-dialog` in `web/package.json`.

The folder-picker IPC SHALL be exclusive to the entry-page onramp surface; no other route in this change SHALL expose a `dialog::open` invocation.

#### Scenario: Dialog plugin registered and permission granted

- **WHEN** the Tauri host application starts
- **THEN** `Cargo.toml` MUST list `tauri-plugin-dialog` as a dependency
- **AND** `lib.rs` builder chain MUST include `tauri_plugin_dialog::init()`
- **AND** `capabilities/default.json` permissions array MUST contain the entry `"dialog:default"`
