# workspace-onramp Specification

## Purpose

TBD - created by archiving change 'entry-workspace-onramp'. Update Purpose after archive.

## Requirements

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


<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->

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


<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->

---
### Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE

After `deriveWorkspaceId` produces an id, the onramp composable SHALL drive a four-step sidecar pipeline behind two user clicks. The first click (`<FolderPickerButton>` `picked` event) chains `/scan?stream=true` then `/kb/build`; the second click ("+ 產生 tutorial") chains `/explore` then `/generate`. Each sidecar task uses its own SSE subscription via `useSseTask`; `<OnrampProgress>` MUST surface the active task's phase + counters in real time.

The internal pipeline phase set is `idle | scanning | indexing | scan-complete | exploring | generating | ready | error`. Phase transitions:

1. Pre-click 1 → `idle`.
2. Click 1 (`onramp.start(path)`):
   - POST `/scan?stream=true` with `{ workspace_root: <path>, workspace_type: "folder" }` → phase `scanning`.
   - On scan SSE `done`: GET `/tasks/<scan_task_id>/result` to retrieve the full `ScanResult`. POST `/kb/build` with `{ workspace_root: <path>, scan_result: <ScanResult> }` → phase `indexing`.
   - On kb-build SSE `done`: phase `scan-complete`.
3. Click 2 (`onramp.triggerGenerate()`):
   - POST `/explore` with `{ workspace_root: <path>, task: <ONRAMP_DEFAULT_TASK> }` → phase `exploring`. `ONRAMP_DEFAULT_TASK` is the constant `"認識整個 codebase"` exported from `web/app/composables/useWorkspaceOnramp.ts` (Decision 6); it satisfies the sidecar `task: str = Field(min_length=1)` constraint without prompting the user.
   - On explore SSE `done`: GET `/tasks/<explore_task_id>/result` to retrieve the `ExplorerState`. POST `/generate` with `{ workspace_root: <path>, task: <ONRAMP_DEFAULT_TASK>, stations: <ExplorerState.stations> }` → phase `generating`.
   - On generate SSE `done`: phase `ready`. Render an anchor whose target is `/tutorial/<workspace_id>`. The CTA MUST NOT auto-navigate.

Any SSE `error` event from any of the four tasks transitions the onramp to phase `error` with the error code/message preserved in state. `retry()` re-issues the POST that owned the failed task — never restarts from an earlier phase.

#### Scenario: Scan terminal event chains kb-build automatically

- **WHEN** the SSE stream of the in-flight `/scan?stream=true` task emits a `done` event
- **THEN** the composable MUST GET `/tasks/<scan_task_id>/result` to fetch the `ScanResult` payload
- **AND** the composable MUST POST `/kb/build` with body `{ workspace_root: <path>, scan_result: <ScanResult> }`
- **AND** the onramp phase MUST transition to `indexing`
- **AND** the user MUST NOT see an intermediate CTA between scan and kb-build

#### Scenario: kb-build terminal event unlocks generate CTA

- **WHEN** the SSE stream of the in-flight `/kb/build` task emits a `done` event
- **THEN** the onramp phase MUST transition to `scan-complete`
- **AND** `<WorkspaceOnrampCard>` MUST render a button with `data-testid="onramp-generate-cta"`
- **AND** clicking that button MUST start the explore task by issuing `POST /explore` with `workspace_root` matching the picked path

#### Scenario: Explore terminal event chains generate automatically

- **WHEN** the SSE stream of the in-flight `/explore` task emits a `done` event
- **THEN** the composable MUST GET `/tasks/<explore_task_id>/result` to fetch the `ExplorerState` payload
- **AND** the composable MUST POST `/generate` with body `{ workspace_root: <path>, task: <ONRAMP_DEFAULT_TASK>, stations: <ExplorerState.stations> }`
- **AND** the onramp phase MUST transition to `generating`
- **AND** the user MUST NOT see an intermediate CTA between explore and generate

#### Scenario: Generate terminal event renders enter-tutorial CTA

- **WHEN** the SSE stream of the in-flight `/generate` task emits a `done` event
- **THEN** the onramp phase MUST transition to `ready`
- **AND** `<WorkspaceOnrampCard>` MUST render an anchor with `data-testid="onramp-enter-tutorial"` whose `href` resolves to `/tutorial/<workspace_id>`
- **AND** the anchor MUST NOT trigger automatic navigation; the user MUST click it

#### Scenario: SSE error pauses onramp with retry affordance

- **WHEN** an SSE event of any in-flight pipeline task (`scan` / `kb` / `explore` / `generate`) carries an `error` field
- **THEN** the onramp phase MUST transition to `error`
- **AND** the error message MUST be displayed inside `<WorkspaceOnrampCard>` (no silent log-only failure)
- **AND** a button with `data-testid="onramp-retry"` MUST render that re-issues the POST that owned the failed task when clicked (NOT an earlier phase)
- **AND** the user's selected path / derived id MUST remain visible (the user MUST NOT have to re-pick the folder)


<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->

---
### Requirement: Onramp state survives navigation away from entry page

The `useWorkspaceOnramp` composable SHALL be a module-level singleton (matching the pattern used by `useQaSession` and `useIntervention`) so that pipeline state is preserved when the user navigates to `/settings` or `/audit/*` and back to `/`. The active SSE subscription (whichever of the four pipeline tasks is currently running) MUST NOT be torn down on `/` unmount.

#### Scenario: User leaves entry mid-scan and returns

- **WHEN** the onramp is in any in-flight phase (`scanning` / `indexing` / `exploring` / `generating`) and the user navigates to `/settings`
- **THEN** the SSE subscription to the active task MUST remain active
- **AND** when the user navigates back to `/`, `<OnrampProgress>` MUST resume rendering with the latest counter values
- **AND** the onramp phase MUST reflect any progression that happened during the absence (e.g. `scan-complete` if scan + kb-build finished while away, or `ready` if the entire pipeline completed)


<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->

---
### Requirement: AppShell ping-smoke placeholder is removed

The Phase 6 step 25.5 `web/app/components/AppShell.vue` ping-smoke placeholder SHALL be removed from the codebase as part of this change. Its sole import site is `web/app/pages/index.vue`, which this change rewrites; no other component references it (verified by repo-wide grep at task time).

#### Scenario: AppShell.vue is deleted and not referenced

- **WHEN** the source tree is searched for the substring `AppShell`
- **THEN** no `.vue` / `.ts` file under `web/app/` MUST contain that token
- **AND** the file `web/app/components/AppShell.vue` MUST NOT exist


<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->

---
### Requirement: Tauri host wires the dialog plugin

The Tauri host SHALL declare `tauri-plugin-dialog = "2"` in `tauri/src-tauri/Cargo.toml`, register the plugin in `tauri/src-tauri/src/lib.rs` via `.plugin(tauri_plugin_dialog::init())`, and grant `dialog:default` permission in `tauri/src-tauri/capabilities/default.json`. The web side SHALL declare `@tauri-apps/plugin-dialog` in `web/package.json`.

The folder-picker IPC SHALL be exclusive to the entry-page onramp surface; no other route in this change SHALL expose a `dialog::open` invocation.

#### Scenario: Dialog plugin registered and permission granted

- **WHEN** the Tauri host application starts
- **THEN** `Cargo.toml` MUST list `tauri-plugin-dialog` as a dependency
- **AND** `lib.rs` builder chain MUST include `tauri_plugin_dialog::init()`
- **AND** `capabilities/default.json` permissions array MUST contain the entry `"dialog:default"`

<!-- @trace
source: entry-workspace-onramp
updated: 2026-05-03
code:
  - sidecar/src/codebus_agent/api/tasks.py
  - web/package.json
  - tauri/src-tauri/src/lib.rs
  - web/app/pages/index.vue
  - tauri/src-tauri/Cargo.toml
  - web/app/components/workspace-onramp/FolderPickerButton.vue
  - web/app/components/workspace-onramp/WorkspaceOnrampCard.vue
  - web/app/utils/workspace-id.ts
  - web/dist
  - sidecar/src/codebus_agent/auth/__init__.py
  - .spectra.yaml
  - CLAUDE.md
  - web/app/components/AppShell.vue
  - sidecar/src/codebus_agent/api/main.py
  - web/app/components/workspace-onramp/OnrampProgress.vue
  - web/app/composables/useWorkspaceOnramp.ts
  - web/app/composables/useSseTask.ts
  - tauri/src-tauri/capabilities/default.json
tests:
  - sidecar/tests/auth/test_workspace_id_parity.py
  - sidecar/tests/auth/test_bearer_query_param.py
  - web/tests/onramp/OnrampProgress.spec.ts
  - tauri/src-tauri/tests/dialog_plugin_smoke.rs
  - sidecar/tests/auth/test_access_log_invariant.py
  - web/tests/onramp/useWorkspaceOnramp.spec.ts
  - web/tests/onramp/WorkspaceOnrampCard.spec.ts
  - sidecar/tests/api/test_tasks_sse_wire_format.py
  - web/tests/onramp/FolderPickerButton.spec.ts
  - web/tests/setup.ts
  - web/tests/composables/useSseTask.connection-error.spec.ts
  - web/tests/utils/workspace-id.spec.ts
  - web/tests/onboarding/index-page-redirect.spec.ts
-->
