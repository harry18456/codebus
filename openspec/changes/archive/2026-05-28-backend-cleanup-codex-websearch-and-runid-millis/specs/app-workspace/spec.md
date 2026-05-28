## MODIFIED Requirements

### Requirement: Tauri IPC Commands for Goal Lifecycle and Wiki Read

The system SHALL register Tauri commands beyond the foundation's nine commands, covering goal-mode lifecycle, chat-turn lifecycle, and wiki read paths. The full added set is:

- `spawn_goal(vault_path: String, goal_text: String) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::goal::run_goal` with the given vault and goal text. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` equals the run's `started_at` slug derived from `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` with `:` replaced by `-`), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"goal-stream"` with payload `{ run_id: String, event: VerbEvent }`. On thread completion (success, failure, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_goal(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (run already terminated), return `Ok(())` (idempotent).

- `list_runs(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` — read all `runs-*.jsonl` files under `<vault>/.codebus/log/`, parse each row to `RunLogSummary`, apply `mode_filter` (`Goal` keeps only `mode=="goal"`; `All` keeps everything), then scan `events-*.jsonl` files for interrupted detection per the next requirement, merge virtual entries, and return the combined list sorted by `started_at` descending.

- `get_run_detail(vault_path: String, run_id: String) -> Result<RunDetail, AppError>` — find the matching `RunLogSummary` (real or virtual interrupted), open the corresponding `events-*.jsonl`, replay all events into `Vec<RecordedEvent>`, and return `RunDetail { summary, events }`.

- `list_wiki_pages(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` — glob `<vault>/.codebus/wiki/**/*.md`, parse each file's frontmatter to extract `title`, derive slug from the filename (without `.md`), and return one `WikiPageMeta { slug, path, title }` per file. Files without parseable frontmatter SHALL still be returned with `title` equal to the slug.

- `read_wiki_page(vault_path: String, page_slug: String) -> Result<String, AppError>` — look up the page by slug among the wiki files, read its raw bytes, strip the leading frontmatter block (delimited by `---\n...\n---\n` at the start), and return the remaining markdown body as a `String`. If the slug does not match any wiki file, return `AppError::Invalid { field: "page_slug", message: "no such page" }`.

The chat-turn lifecycle commands (`spawn_chat_turn`, `cancel_chat_turn`) are defined separately under `Tauri IPC Commands for Chat Turn Lifecycle` and SHALL coexist with the above in `codebus-app/src-tauri/src/ipc/mod.rs` registration.

`ModeFilter` SHALL be a serde-tagged enum with variants `Goal` and `All` (snake_case).

`AppError` SHALL be the same discriminated union used by the foundation's commands — no new variants added by this change.

The goal `RunId` SHALL be derived using `chrono::SecondsFormat::Millis` precision so that two `spawn_goal` invocations occurring within the same wall-clock second receive distinct `active_runs` keys and the second invocation does not overwrite the first invocation's cancel handle.

#### Scenario: spawn_goal returns run id derived from started_at

- **WHEN** the frontend calls `invoke("spawn_goal", { vault_path: "/some/vault", goal_text: "X" })` AND the spawned `run_goal` invocation's first stream event timestamps the run at `2026-05-13T14:56:21.123Z`
- **THEN** the IPC call resolves with `"2026-05-13T14-56-21.123Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: spawn_goal same-second calls yield distinct RunIds

- **WHEN** the frontend calls `invoke("spawn_goal", ...)` twice in rapid succession AND both calls land within the same wall-clock second but on distinct wall-clock milliseconds
- **THEN** the two IPC calls SHALL resolve with two distinct `RunId` strings differing in the `.fff` fractional component AND `AppState.active_runs` SHALL contain two entries simultaneously, each with its own cancel handle

#### Scenario: cancel_goal idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_goal", { run_id: "nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: list_runs filters by mode

- **WHEN** the frontend calls `invoke("list_runs", { vault_path: ..., mode_filter: { kind: "goal" } })` AND the vault's `runs-*.jsonl` contain three goal rows, two chat rows, and one fix row
- **THEN** the returned `Vec<RunLogSummary>` length is 3 AND every entry has `mode == "goal"`

#### Scenario: read_wiki_page strips frontmatter

- **WHEN** the frontend calls `invoke("read_wiki_page", { vault_path: ..., page_slug: "uv-lib" })` AND the file at `<vault>/.codebus/wiki/modules/uv-lib.md` contains a frontmatter block followed by markdown body
- **THEN** the IPC returns the markdown body string without the leading `---\n...\n---\n` block

### Requirement: Tauri IPC Commands for Chat Turn Lifecycle

The system SHALL register two new Tauri commands for chat turn lifecycle, extending the goal lifecycle IPC surface:

- `spawn_chat_turn(vault_path: String, text: String, session_id: Option<String>) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::chat::run_chat_turn` with `ChatTurnOptions { text, session_id }`. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` = `chat-<started_at_slug>` and the slug is derived from `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` with `:` replaced by `-`), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"chat-stream"` with payload `{ run_id: String, event: VerbEvent }`. The chat-stream channel SHALL be separate from the existing `goal-stream` channel. On thread completion (success, failure, cancel, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_chat_turn(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (turn already terminated), return `Ok(())` (idempotent).

`spawn_chat_turn` SHALL return `AppError::Invalid { field: "active_runs", message: "another chat turn is already active in this session" }` when invoked while `active_runs` already contains a `chat-*` keyed entry for the same vault. Goal-mode entries SHALL NOT block chat spawn AND vice versa (see `One Active Goal Run At A Time` modification).

The chat `RunId` SHALL be derived using `chrono::SecondsFormat::Millis` precision so that two `spawn_chat_turn` invocations occurring within the same wall-clock second (necessarily across distinct vault paths, since per-vault concurrency is bounded to one) receive distinct `active_runs` keys and the second invocation does not overwrite the first invocation's cancel handle.

#### Scenario: spawn_chat_turn returns chat run id

- **WHEN** the frontend calls `invoke("spawn_chat_turn", { vault_path: "/some/vault", text: "X", session_id: null })` AND the spawned `run_chat_turn` invocation's first stream event timestamps the turn at `2026-05-14T10:20:30.456Z`
- **THEN** the IPC call resolves with `"chat-2026-05-14T10-20-30.456Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: spawn_chat_turn same-second calls across vaults yield distinct RunIds

- **WHEN** the frontend calls `invoke("spawn_chat_turn", { vault_path: "V1", ... })` AND then `invoke("spawn_chat_turn", { vault_path: "V2", ... })` in rapid succession AND both calls land within the same wall-clock second but on distinct wall-clock milliseconds
- **THEN** the two IPC calls SHALL resolve with two distinct `chat-<slug>` `RunId` strings differing in the `.fff` fractional component AND `AppState.active_runs` SHALL contain two entries simultaneously, one for each vault, each with its own cancel handle

#### Scenario: spawn_chat_turn rejects when chat turn already active

- **WHEN** a chat turn is currently active for vault `V` (an entry exists in `active_runs` with key starting `chat-`) AND the frontend invokes `spawn_chat_turn` with the same vault
- **THEN** the IPC call rejects with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring `"chat turn is already active"`

#### Scenario: chat-stream events forwarded with run_id payload

- **WHEN** `spawn_chat_turn` is invoked AND the backend emits a `VerbEvent::Stream { ... }` event
- **THEN** the Tauri event channel `chat-stream` SHALL receive a payload `{ run_id: "chat-<slug>", event: <VerbEvent JSON> }`

#### Scenario: cancel_chat_turn idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_chat_turn", { run_id: "chat-nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches.

When no matching `runs-*.jsonl` row exists AND the events file is identifiable as a goal-mode run (one of its leading events is a `VerbBanner::Goal` event), the system SHALL surface a virtual `RunLogSummary` whose `outcome` field SHALL be determined by whether the run is still alive: if the slug is currently present in the process-wide `active_runs` map (the in-memory cancel-flag registry maintained by `spawn_goal` / cleanup), `outcome` SHALL be `"running"` and `interrupt_reason` SHALL be absent; otherwise `outcome` SHALL be `"interrupted"` and `interrupt_reason` SHALL be `AppClose`. In both cases the synthesized entry SHALL have `started_at` derived from the slug, `goal` extracted from the `VerbBanner::Goal` event, `mode="goal"`, and an empty `finished_at`. This dual projection prevents the GUI from misreading an in-flight goal (whose terminal RunLog has not yet been written) as interrupted just because the user navigated to the Lobby and back.

When an orphan events file is NOT identifiable as a goal-mode run (no `VerbBanner::Goal` event among its leading events), the system SHALL NOT surface any virtual entry for it, and that events file SHALL NOT contribute any row to the `list_runs` response. This prevents in-progress or interrupted `chat` / `query` / `fix` / `quiz` runs — whose `events-*.jsonl` file exists before their terminal `runs-*.jsonl` row is written — from transiently appearing in the Goals list with empty goal text.

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state plus the current `active_runs` snapshot.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

**NOTE — Precision Alignment Invariant:** The `active_runs` map key (set by `spawn_goal` / `spawn_chat_turn` in the IPC layer), the `events-<slug>.jsonl` filename slug (set by the verb function's `run_started_at` capture), AND the `RunLog.started_at` value persisted in `runs-*.jsonl` (also set from the verb function's `run_started_at`) SHALL all be derived at the SAME `chrono::SecondsFormat::Millis` precision. The orphan-detection join in `list_runs` joins these three values as strings; if a future change reverts any one of them to `SecondsFormat::Secs` (or upgrades to a higher precision asymmetrically), the join silently breaks and live goals are mis-labeled `"interrupted"` because `active_runs.get(events_slug)` always misses. This invariant is enforced by the `goal_run_id_precision_matches_verb_run_started_at_slug` unit test in `codebus-app/src-tauri/src/ipc/goals.rs`; that test SHALL fail loudly the moment the precisions drift apart.

#### Scenario: Orphan goal events file with no active_runs entry produces interrupted virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00.000Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="describe auth flow"` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00.000Z"` AND `active_runs` does NOT contain the slug
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `mode == "goal"` AND `goal == "describe auth flow"` AND `started_at == "2026-05-13T03:00:00.000Z"` AND `interrupt_reason == "app_close"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Orphan goal events file with live active_runs entry produces running virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-28T07-39-26.123Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="smoke probe goal"` AND no `runs-*.jsonl` row matches its slug AND `active_runs` currently contains an entry keyed by `"2026-05-28T07-39-26.123Z"` (the in-flight spawn from the current app session, with millisecond precision matching the events file slug)
- **THEN** the returned list contains a virtual entry with `outcome == "running"` AND `mode == "goal"` AND `goal == "smoke probe goal"` AND `started_at == "2026-05-28T07:39:26.123Z"` AND `interrupt_reason` absent AND `finished_at` empty

#### Scenario: Precision drift between active_runs key and events file slug breaks orphan detection

- **WHEN** the IPC layer's `active_runs` map keys are derived at `SecondsFormat::Millis` precision (e.g., `"2026-05-28T09-50-42.322Z"`) AND a verb function reverts to `SecondsFormat::Secs` so its `events-<slug>.jsonl` filename and `RunLog.started_at` use second precision (e.g., `"2026-05-28T09-50-42Z"`)
- **THEN** `list_runs` SHALL mis-label the still-running goal as `"interrupted"` because the events-file slug (Secs) cannot match any `active_runs` key (Millis), violating this requirement; the `goal_run_id_precision_matches_verb_run_started_at_slug` unit test SHALL fail and name the offending derivation site
