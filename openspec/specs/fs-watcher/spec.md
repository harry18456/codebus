# fs-watcher Specification

## Purpose

TBD - created by archiving change 'codebus-fs-watcher'. Update Purpose after archive.

## Requirements

### Requirement: Filesystem Watcher Module

The codebus-app SHALL provide a single filesystem watcher module under `codebus-app/src-tauri/src/watcher.rs` that owns all OS-level filesystem monitoring on behalf of the application. The module SHALL use the `notify` crate's `RecommendedWatcher` so the OS-specific backend (macOS FSEvents, Linux inotify, Windows ReadDirectoryChangesW) is selected automatically. No other module in the application SHALL invoke `notify` directly; all filesystem monitoring SHALL flow through this seam.

#### Scenario: Watcher module is the single seam

- **WHEN** a developer searches the codebase for direct uses of `notify::Watcher::new` or `notify::RecommendedWatcher`
- **THEN** the only call sites SHALL be inside `codebus-app/src-tauri/src/watcher.rs`

#### Scenario: Recommended backend selection per platform

- **WHEN** the watcher module starts a watcher at runtime
- **THEN** the underlying `notify` backend SHALL be the `RecommendedWatcher` and no explicit platform-specific `PollWatcher` or alternate backend SHALL be configured


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
### Requirement: Per-Vault Watcher Lifecycle

The application SHALL expose two Tauri commands, `start_vault_watcher(vault_path)` and `stop_vault_watcher(vault_path)`, that bind a watcher's lifecycle to the currently active vault. `start_vault_watcher` SHALL recursively watch the three directories `<vault_path>/.codebus/wiki/`, `<vault_path>/.codebus/log/`, and `<vault_path>/.codebus/quiz/`. Calling `start_vault_watcher` for a vault that already has a watcher SHALL be idempotent: the existing watcher is stopped before the new one starts. `stop_vault_watcher` for a vault with no active watcher SHALL be a no-op and SHALL NOT return an error.

#### Scenario: Workspace mount starts the vault watcher

- **WHEN** the Workspace component mounts for vault V
- **THEN** the frontend invokes `start_vault_watcher(V)` AND the watcher module begins emitting events for `V/.codebus/wiki/`, `V/.codebus/log/`, `V/.codebus/quiz/`

#### Scenario: Workspace unmount stops the vault watcher

- **WHEN** the Workspace component unmounts (user returns to Lobby or switches vault)
- **THEN** the frontend invokes `stop_vault_watcher(V)` AND the watcher module releases the OS-level handles for V

#### Scenario: Idempotent start replaces the prior watcher

- **WHEN** `start_vault_watcher(V)` is invoked while a watcher for V already exists
- **THEN** the prior watcher SHALL be stopped before the new one is created AND only one active watcher for V SHALL exist afterwards

#### Scenario: Stop on unstarted vault is a no-op

- **WHEN** `stop_vault_watcher(V)` is invoked for a vault that has no active watcher
- **THEN** the command SHALL return success without raising an error


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
### Requirement: Lobby Watcher

The application SHALL start a single long-lived watcher during app `setup` that monitors `~/.codebus/app-state.json` for changes for the entire application session. This watcher SHALL emit a `vault-list-changed` Tauri event whenever the file changes. The watcher SHALL be released when the application exits.

#### Scenario: Lobby watcher starts at app setup

- **WHEN** the codebus-app process completes its Tauri `setup` hook
- **THEN** a watcher monitoring `~/.codebus/app-state.json` SHALL be active

#### Scenario: External edit to app-state.json emits vault-list-changed

- **GIVEN** the Lobby watcher is active AND the Lobby is displayed
- **WHEN** an external process modifies `~/.codebus/app-state.json`
- **THEN** a `vault-list-changed` Tauri event SHALL be emitted within 400 ms (200 ms debounce window plus scheduling slack)


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
### Requirement: Per-Path Debounce Window

The watcher module SHALL coalesce raw filesystem events on a per-path basis with a 200 ms debounce window. After receiving a raw event for path P, the module SHALL start or reset a 200 ms timer keyed by P; only when the timer elapses without another raw event for P SHALL the corresponding Tauri event be emitted. Events for distinct paths SHALL be debounced independently — a save to path A SHALL NOT delay the emission of an event for path B.

#### Scenario: Atomic-rename save emits a single event

- **WHEN** an editor saves a file as a temp-plus-rename sequence that produces three raw filesystem events within 50 ms
- **THEN** the watcher SHALL emit exactly one Tauri event for that path

#### Scenario: Distinct paths debounce independently

- **WHEN** path A receives a raw event at t=0 ms AND path B receives a raw event at t=50 ms with no further events on either
- **THEN** the event for B SHALL be emitted at t approximately 250 ms even though the event for A is emitted at t approximately 200 ms


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
### Requirement: Event Catalog And Payloads

The watcher module SHALL emit exactly seven Tauri event names with the payload shapes below. No other Tauri event SHALL be emitted by the watcher module.

- `wiki-list-changed` — no payload — triggered by any add, remove, or rename of `*.md` under `<vault>/.codebus/wiki/`
- `wiki-page-changed` — payload `{ path: string }` (absolute) — triggered by content modification of a specific `.md` under `<vault>/.codebus/wiki/`
- `goals-changed` — no payload — triggered by any add or remove of `events-*.jsonl` or `runs-*.jsonl` under `<vault>/.codebus/log/`
- `goal-run-changed` — payload `{ run_id: string }` (started_at slug) — triggered by content modification of a specific `events-<slug>.jsonl` or `runs-<date>.jsonl`
- `quiz-changed` — no payload — triggered by any add or remove of files or directories under `<vault>/.codebus/quiz/`
- `quiz-attempt-changed` — payload `{ slug: string, id: string }` — triggered by content modification of `<slug>/<id>.md` or `<slug>/<id>.progress.json`
- `vault-list-changed` — no payload — triggered by content modification of `~/.codebus/app-state.json`

#### Scenario: wiki-page-changed carries the absolute path

- **GIVEN** the vault watcher for V is active
- **WHEN** the file `<V>/.codebus/wiki/concepts/foo.md` is modified
- **THEN** a `wiki-page-changed` event SHALL be emitted with payload `{ path: "<V>/.codebus/wiki/concepts/foo.md" }`

#### Scenario: goal-run-changed carries the run_id

- **GIVEN** the vault watcher for V is active
- **WHEN** the file `<V>/.codebus/log/events-2026-05-20T08-30-00Z.jsonl` is appended
- **THEN** a `goal-run-changed` event SHALL be emitted with payload `{ run_id: "2026-05-20T08-30-00Z" }`

#### Scenario: quiz-attempt-changed carries slug and id

- **GIVEN** the vault watcher for V is active
- **WHEN** the file `<V>/.codebus/quiz/jwt-basics/2026-05-20T08-30-00Z.progress.json` is modified
- **THEN** a `quiz-attempt-changed` event SHALL be emitted with payload `{ slug: "jwt-basics", id: "2026-05-20T08-30-00Z" }`


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
### Requirement: Excluded Watch Scope

The watcher module SHALL NOT watch the following paths even when they live inside a watched vault root or the user home directory: `~/.codebus/config.yaml`, `<vault>/.codebus/raw/`, `<vault>/.codebus/CLAUDE.md`, and any subdirectory matching the v1 internal-gitignore exclusion list (`.lock`, `.git`, `**/.obsidian/`, and similar).

#### Scenario: Settings file is not watched

- **GIVEN** the Lobby watcher and a vault watcher are active
- **WHEN** an external process modifies `~/.codebus/config.yaml`
- **THEN** no Tauri event SHALL be emitted by the watcher module

#### Scenario: raw mirror changes are not surfaced

- **GIVEN** the vault watcher for V is active
- **WHEN** an external process modifies a file under `<V>/.codebus/raw/code/`
- **THEN** no Tauri event SHALL be emitted by the watcher module


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
### Requirement: Watcher Startup Failure Surfaces Loudly

When `notify::Watcher::new` fails (most commonly Linux ENOSPC from `fs.inotify.max_user_watches`, or macOS missing file-access permission), the watcher module SHALL NOT silently fall back to polling. Instead the module SHALL emit a single `vault-watcher-error` Tauri event with payload `{ vault_path: string, reason: string }` and SHALL NOT subsequently emit any other watcher event for that vault. The frontend SHALL display a persistent auto-refresh-disabled indicator for that vault and SHALL NOT auto-retry the watcher.

#### Scenario: Linux ENOSPC fails loud

- **GIVEN** the OS-level inotify watch limit has been exhausted
- **WHEN** the frontend invokes `start_vault_watcher(V)`
- **THEN** the watcher module SHALL emit `vault-watcher-error` with `reason` containing `ENOSPC` AND no `wiki-list-changed`, `wiki-page-changed`, `goals-changed`, `goal-run-changed`, `quiz-changed`, or `quiz-attempt-changed` events SHALL be emitted for V thereafter

#### Scenario: No silent polling fallback

- **GIVEN** the watcher module has emitted `vault-watcher-error` for vault V
- **WHEN** files under `V/.codebus/` are subsequently modified
- **THEN** the watcher module SHALL NOT emit any watcher event for V AND the frontend SHALL NOT issue automatic refresh IPC calls for V


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
### Requirement: Frontend useWatcherEvent Hook

The codebus-app frontend SHALL provide a single React hook `useWatcherEvent(eventName, handler)` under `codebus-app/src/hooks/useWatcherEvent.ts` that wraps `@tauri-apps/api/event::listen` and returns a cleanup function suitable for use in a React `useEffect`. All store and component subscriptions to watcher events SHALL use this hook; direct calls to `listen` for watcher events SHALL NOT appear elsewhere in the frontend.

#### Scenario: Hook is the only listen entry point for watcher events

- **WHEN** a developer searches the frontend for direct `listen("wiki-list-changed" | "wiki-page-changed" | "goals-changed" | "goal-run-changed" | "quiz-changed" | "quiz-attempt-changed" | "vault-list-changed" | "vault-watcher-error", ...)` calls
- **THEN** the only call sites SHALL be inside `codebus-app/src/hooks/useWatcherEvent.ts`

#### Scenario: Cleanup function is returned

- **WHEN** `useWatcherEvent("wiki-list-changed", handler)` is invoked inside a `useEffect`
- **THEN** the return value SHALL be a function that when invoked unsubscribes the listener from the Tauri event channel

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