## MODIFIED Requirements

### Requirement: Tauri IPC Commands for Goal Lifecycle and Wiki Read

The system SHALL register Tauri commands beyond the foundation's nine commands, covering goal-mode lifecycle, chat-turn lifecycle, and wiki read paths. The full added set is:

- `spawn_goal(vault_path: String, goal_text: String) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::goal::run_goal` with the given vault and goal text. The function SHALL sample the wall-clock time EXACTLY ONCE via `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` and derive from that single sample BOTH the `RunId` slug (the RFC 3339 string with `:` replaced by `-`) AND the colon-form RFC 3339 string passed down into `run_goal` as its `run_started_at` argument. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the `RunId` slug, return the `RunId` slug to the caller, AND emit each `VerbEvent` produced by the closure to a Tauri event channel named `"goal-stream"` with payload `{ run_id: String, event: VerbEvent }` carrying that same `RunId` slug. The `goal-terminal` payload SHALL carry that same `RunId` slug. Because the events.jsonl filename slug and `RunLog.started_at` written by `run_goal` derive from the SAME single sample, the `RunId` returned to the frontend SHALL (after slugging) be byte-equal to the persisted run's identity, so a completed run's `get_run_detail` lookup by that `RunId` SHALL succeed. On thread completion (success, failure, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_goal(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (run already terminated), return `Ok(())` (idempotent).

- `list_runs(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` — read all `runs-*.jsonl` files under `<vault>/.codebus/log/`, parse each row to `RunLogSummary`, apply `mode_filter` (`Goal` keeps only `mode=="goal"`; `All` keeps everything), then scan `events-*.jsonl` files for interrupted detection per the next requirement, merge virtual entries, and return the combined list sorted by `started_at` descending.

- `get_run_detail(vault_path: String, run_id: String) -> Result<RunDetail, AppError>` — find the matching `RunLogSummary` (real or virtual interrupted), open the corresponding `events-*.jsonl`, replay all events into `Vec<RecordedEvent>`, and return `RunDetail { summary, events }`.

- `list_wiki_pages(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` — glob `<vault>/.codebus/wiki/**/*.md`, parse each file's frontmatter to extract `title`, derive slug from the filename (without `.md`), and return one `WikiPageMeta { slug, path, title }` per file. Files without parseable frontmatter SHALL still be returned with `title` equal to the slug.

- `read_wiki_page(vault_path: String, page_slug: String) -> Result<String, AppError>` — look up the page by slug among the wiki files, read its raw bytes, strip the leading frontmatter block (delimited by `---\n...\n---\n` at the start), and return the remaining markdown body as a `String`. If the slug does not match any wiki file, return `AppError::Invalid { field: "page_slug", message: "no such page" }`.

The chat-turn lifecycle commands (`spawn_chat_turn`, `cancel_chat_turn`) are defined separately under `Tauri IPC Commands for Chat Turn Lifecycle` and SHALL coexist with the above in `codebus-app/src-tauri/src/ipc/mod.rs` registration.

`ModeFilter` SHALL be a serde-tagged enum with variants `Goal` and `All` (snake_case).

`AppError` SHALL be the same discriminated union used by the foundation's commands — no new variants added by this change.

The goal `RunId` SHALL be sampled EXACTLY ONCE in the IPC layer at `chrono::SecondsFormat::Millis` precision and threaded down into `run_goal` (as `run_started_at`), so that (a) two `spawn_goal` invocations within the same wall-clock second receive distinct `active_runs` keys, AND (b) the `active_runs` key, the `RunId` returned to the frontend, the events.jsonl filename slug, and the `RunLog.started_at` value all originate from that single sample and are therefore byte-identical strings — never two independent `Utc::now()` samples that can drift apart.

#### Scenario: spawn_goal returns run id derived from a single IPC sample

- **WHEN** the frontend calls `invoke("spawn_goal", { vault_path: "/some/vault", goal_text: "X" })` AND the IPC layer samples the wall-clock once at `2026-05-13T14:56:21.123Z`
- **THEN** the IPC call resolves with `"2026-05-13T14-56-21.123Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string AND the colon-form `"2026-05-13T14:56:21.123Z"` is passed to `run_goal` as `run_started_at`

#### Scenario: spawn_goal same-second calls yield distinct RunIds

- **WHEN** the frontend calls `invoke("spawn_goal", ...)` twice in rapid succession AND both calls land within the same wall-clock second but on distinct wall-clock milliseconds
- **THEN** the two IPC calls SHALL resolve with two distinct `RunId` strings differing in the `.fff` fractional component AND `AppState.active_runs` SHALL contain two entries simultaneously, each with its own cancel handle

#### Scenario: spawn_goal RunId resolves via get_run_detail after the run terminates

- **WHEN** the frontend calls `invoke("spawn_goal", ...)` and receives `RunId` `R` AND the spawned `run_goal` runs to a terminal outcome (`succeeded` or `failed`), writing its `events-*.jsonl` file and `RunLog` row to disk
- **THEN** a subsequent `invoke("get_run_detail", { vault_path, run_id: R })` SHALL resolve with the `RunDetail` for that run (the persisted run's slug equals `R`) AND SHALL NOT return `AppError::Invalid { field: "run_id" }`

#### Scenario: cancel_goal idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_goal", { run_id: "nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: list_runs filters by mode

- **WHEN** the frontend calls `invoke("list_runs", { vault_path: ..., mode_filter: { kind: "goal" } })` AND the vault's `runs-*.jsonl` contain three goal rows, two chat rows, and one fix row
- **THEN** the returned `Vec<RunLogSummary>` length is 3 AND every entry has `mode == "goal"`

#### Scenario: read_wiki_page strips frontmatter

- **WHEN** the frontend calls `invoke("read_wiki_page", { vault_path: ..., page_slug: "uv-lib" })` AND the file at `<vault>/.codebus/wiki/modules/uv-lib.md` contains a frontmatter block followed by markdown body
- **THEN** the IPC returns the markdown body string without the leading `---\n...\n---\n` block

### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches.

When no matching `runs-*.jsonl` row exists AND the events file is identifiable as a goal-mode run (one of its leading events is a `VerbBanner::Goal` event), the system SHALL surface a virtual `RunLogSummary` whose `outcome` field SHALL be determined by whether the run is still alive: if the slug is currently present in the process-wide `active_runs` map (the in-memory cancel-flag registry maintained by `spawn_goal` / cleanup), `outcome` SHALL be `"running"` and `interrupt_reason` SHALL be absent; otherwise `outcome` SHALL be `"interrupted"` and `interrupt_reason` SHALL be `AppClose`. In both cases the synthesized entry SHALL have `started_at` derived from the slug, `goal` extracted from the `VerbBanner::Goal` event, `mode="goal"`, and an empty `finished_at`. This dual projection prevents the GUI from misreading an in-flight goal (whose terminal RunLog has not yet been written) as interrupted just because the user navigated to the Lobby and back.

When an orphan events file is NOT identifiable as a goal-mode run (no `VerbBanner::Goal` event among its leading events), the system SHALL NOT surface any virtual entry for it, and that events file SHALL NOT contribute any row to the `list_runs` response. This prevents in-progress or interrupted `chat` / `query` / `fix` / `quiz` runs — whose `events-*.jsonl` file exists before their terminal `runs-*.jsonl` row is written — from transiently appearing in the Goals list with empty goal text.

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state plus the current `active_runs` snapshot.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

**NOTE — Single-Source Run Id Invariant:** The `active_runs` map key, the `RunId` returned to the frontend, the `events-<slug>.jsonl` filename slug, AND the `RunLog.started_at` value persisted in `runs-*.jsonl` SHALL all originate from a SINGLE `chrono::Utc::now()` sample taken once in the IPC layer (`spawn_goal` / `spawn_chat_turn`) and threaded down into the verb function (e.g. the `run_started_at` argument of `run_goal`). Deriving the verb-side slug from an INDEPENDENT second `Utc::now()` sample is FORBIDDEN: equal `SecondsFormat` precision does NOT guarantee equal values across two samples taken at different instants, so the orphan-detection join in `list_runs` (which joins these values as strings) would silently miss — a live goal would be mis-labeled `"interrupted"` AND a completed run's detail would be unreachable by the frontend's `RunId` (a permanent loading state). The CLI path, which has no cross-layer id join, MAY let the verb function sample internally (passing `run_started_at = None`). This invariant is enforced by a regression test asserting that the `RunId` returned by `spawn_goal` resolves via `get_run_detail` / `list_runs` after the verb writes its terminal RunLog.

#### Scenario: Orphan goal events file with no active_runs entry produces interrupted virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00.000Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="describe auth flow"` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00.000Z"` AND `active_runs` does NOT contain the slug
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `mode == "goal"` AND `goal == "describe auth flow"` AND `started_at == "2026-05-13T03:00:00.000Z"` AND `interrupt_reason == "app_close"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Orphan goal events file with live active_runs entry produces running virtual entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-28T07-39-26.123Z.jsonl` whose leading events include a `VerbBanner::Goal` event with `goal_text="smoke probe goal"` AND no `runs-*.jsonl` row matches its slug AND `active_runs` currently contains an entry keyed by `"2026-05-28T07-39-26.123Z"` (the in-flight spawn from the current app session, whose key is the same single IPC sample as the events file slug)
- **THEN** the returned list contains a virtual entry with `outcome == "running"` AND `mode == "goal"` AND `goal == "smoke probe goal"` AND `started_at == "2026-05-28T07:39:26.123Z"` AND `interrupt_reason` absent AND `finished_at` empty

#### Scenario: Single-source run id keeps active_runs key, events slug, and RunLog.started_at byte-equal

- **WHEN** `spawn_goal` samples the wall-clock once as `2026-05-28T09:50:42.322Z` AND threads the colon form into `run_goal` as `run_started_at` AND the run writes `events-2026-05-28T09-50-42.322Z.jsonl` plus a `RunLog` row with `started_at == "2026-05-28T09:50:42.322Z"`
- **THEN** the `active_runs` key, the `RunId` returned to the frontend, the events-file slug, and the slugged `RunLog.started_at` SHALL all be the byte-identical string `"2026-05-28T09-50-42.322Z"` AND `list_runs` SHALL join them without a miss (the run shows `"running"` while live and is reachable by `get_run_detail` once terminal — never a spurious `"interrupted"` or an unresolvable `RunId`)

## ADDED Requirements

### Requirement: Run Detail Load Failure Surfacing

The system SHALL surface a load failure when fetching a selected run's `RunDetail` (via `get_run_detail`) rejects, instead of remaining indefinitely in the loading state. When the user has selected a non-active run whose `RunDetail` has not yet loaded, the Goals content area SHALL show the loading affordance (`workspace.runDetail.loading`); when the `get_run_detail` call for that run rejects, the Goals content area SHALL render an error state that names the failure AND offers a retry action AND a path back to the Goals list. The frontend SHALL NOT silently discard the `get_run_detail` rejection (no empty catch handler) and SHALL NOT leave the user on the loading affordance after a rejection.

#### Scenario: get_run_detail rejection shows retriable error state

- **WHEN** the user is viewing a selected non-active run AND the `get_run_detail` IPC for that run rejects with an error
- **THEN** the Goals content area SHALL render an error state (not the `workspace.runDetail.loading` affordance) AND the error state SHALL expose a retry control AND a control to return to the Goals list

#### Scenario: successful load after terminal transition shows terminal detail

- **WHEN** the user is sitting in the `Running` detail of a goal AND the goal reaches a terminal outcome (so `activeRun` clears) AND `get_run_detail` for that run resolves
- **THEN** the Goals content area SHALL transition to the matching terminal view (`Done` for `succeeded`, or the interrupted/failed view) AND SHALL NOT remain on the `workspace.runDetail.loading` affordance
