## ADDED Requirements

### Requirement: Workspace Layout and Tab Navigation

The system SHALL replace the Workspace stub with a real Workspace shell composed of a left sidebar with exactly three tabs and a main content area. The three tabs SHALL be `Goals` (default selection), `Wiki`, and `Quiz`. The sidebar SHALL also render the active vault's display name + path block and a `← Back to Lobby` control. Selecting a tab SHALL switch the main content area to the corresponding view; unselected tabs SHALL not retain their internal scroll position across switches (each switch SHALL re-mount the inactive view).

The `Quiz` tab in v1 SHALL render a centered placeholder displaying the literal text "Coming soon — quiz flow ships in v3-app-quiz" and no other interactive controls.

#### Scenario: Workspace mounts with Goals tab selected

- **WHEN** the user opens a vault from the Lobby
- **THEN** the main view transitions to the Workspace AND the `Goals` tab is the active selection AND the `Goals` main content area is rendered

#### Scenario: Tab switch re-renders main content

- **WHEN** the user clicks the `Wiki` tab while `Goals` is active
- **THEN** the main content area renders the Wiki tab content AND the `Goals` main content area is unmounted

#### Scenario: Quiz tab shows v1 placeholder only

- **WHEN** the user clicks the `Quiz` tab
- **THEN** the main content area renders exactly the centered placeholder text "Coming soon — quiz flow ships in v3-app-quiz" AND no quiz-related controls (start button, page selector, history, etc.) SHALL appear

#### Scenario: Back to Lobby control returns to Lobby

- **WHEN** the user clicks the `← Back to Lobby` control while in the Workspace
- **THEN** the main view returns to the Lobby state AND the Workspace component is unmounted AND any active goal run continues running in the background

---

### Requirement: Goals Overview List and Filter

The `Goals` tab main content area SHALL render a vertical list of goal-mode runs from the active vault, sorted by `started_at` descending (newest first). The list SHALL include only `RunLog` entries whose `mode` field equals the literal string `"goal"`. Runs with `mode` equal to `"chat"`, `"query"`, or `"fix"` SHALL NOT appear in this list. Each row SHALL display: an outcome icon (`⚪` for running, `✓` for done, `⏹` for cancelled, `⚠` for interrupted), the goal text (truncated to ~80 chars with ellipsis), and a relative timestamp (e.g., "2m ago", "1h ago").

The list SHALL also include virtual `outcome="interrupted"` entries detected per the `Interrupted Run Detection` requirement. Virtual entries SHALL render with the same row shape but with the `⚠` outcome icon.

When the list is empty (no goal runs in the vault), the main content area SHALL render a centered hint block containing the text "Click + New goal to ask codebus to ingest something into the wiki" followed by exactly three pre-fill example goal strings rendered as clickable rows. Clicking a pre-fill row SHALL open the New Goal modal with that example pre-filled in the textarea.

The Goals tab SHALL also render a `[+ New goal]` button in the top-right corner of the main content area; clicking it SHALL open the New Goal modal.

#### Scenario: Goals overview filters to goal mode only

- **WHEN** the active vault contains `runs-*.jsonl` rows with `mode` values of `"goal"`, `"chat"`, `"query"`, and `"fix"`
- **THEN** the Goals overview list renders exactly the `"goal"`-mode rows AND no rows from the other three modes appear

#### Scenario: Empty Goals overview shows hint with pre-fill examples

- **WHEN** the active vault has zero `mode=goal` rows in `runs-*.jsonl` AND no orphan `events-*.jsonl` files
- **THEN** the Goals tab renders the centered hint text AND exactly three clickable pre-fill example rows AND clicking any pre-fill row opens the New Goal modal with that example text in the textarea

#### Scenario: Run row outcome icon matches RunLog outcome

- **WHEN** the Goals overview list renders a row corresponding to a RunLog entry with `outcome="cancelled"`
- **THEN** the row's leading icon SHALL be `⏹` AND the row remains clickable to navigate to the Cancelled detail view

##### Example: row icon mapping

| RunLog outcome | Row icon |
| -------------- | -------- |
| (active run in progress, no RunLog row yet) | `⚪` |
| `succeeded` | `✓` |
| `cancelled` | `⏹` |
| `failed` | `⚠` |
| virtual `interrupted` (events have no RunLog row) | `⚠` |

---

### Requirement: New Goal Modal Flow

The system SHALL render the `New Goal` modal as a centered dialog with a single multi-line textarea (placeholder text "What should codebus document?") and two buttons labeled `Cancel` and `Run`. The modal SHALL be opened by clicking the Goals tab's `[+ New goal]` button or by clicking any pre-fill example row in the empty-state hint. The modal SHALL trap focus while open and SHALL close on `Esc` keypress, on `Cancel` click, or after a successful `Run` invocation.

Clicking `Run` SHALL invoke the `spawn_goal` Tauri command with the textarea text (trimmed) and the active vault path. On success, the modal SHALL close, the Goals overview list SHALL gain a new running-state row (synthesized client-side before the first `goal-stream` event arrives), AND the main content area SHALL switch to the `Running` detail view for the newly spawned run.

The `Run` button SHALL be disabled (visually and functionally) when any of the following hold: textarea contains only whitespace; another goal run is currently active in this vault (the `useGoalsStore.activeRun` state is non-null). When disabled due to an existing active run, the modal SHALL render a hint line below the textarea reading "Wait for current run to finish or cancel it before starting a new one."

#### Scenario: Submit empty modal text rejected

- **WHEN** the user opens the New Goal modal AND clicks `Run` without typing any text
- **THEN** the `Run` button SHALL be disabled AND no `spawn_goal` invocation SHALL occur

#### Scenario: Submit while another goal run is active

- **WHEN** the user opens the New Goal modal AND a previous goal run is still in the running state (`useGoalsStore.activeRun != null`)
- **THEN** the `Run` button SHALL be disabled AND the modal SHALL render the hint "Wait for current run to finish or cancel it before starting a new one." AND no `spawn_goal` invocation SHALL occur

#### Scenario: Successful Run transitions to Running detail

- **WHEN** the user types `"describe the auth flow"` AND clicks `Run` AND `spawn_goal` resolves with a new `RunId`
- **THEN** the modal closes AND the Goals overview list gains a row with outcome icon `⚪` AND goal text `"describe the auth flow"` AND the main content area switches to the `Running` detail view for that run

---

### Requirement: Run Detail Views — Running

The system SHALL render the `Running` detail view when the user navigates to a run whose state is the currently-active goal run (i.e., `useGoalsStore.activeRun.runId` equals the clicked run id and no RunLog row has been written yet for it). The view SHALL include: a header with `← back`, the goal text, and an `⏺ Running` badge; a metadata line with elapsed time (live-updated every second) and accumulated token count from Usage events received so far; an `Activity stream` block rendering received events in arrival order; and an `[⏹ Cancel]` button.

The Activity stream SHALL render `StreamEvent::ToolUse { name, input }` events as one-line summaries with an emoji leader matching the CLI convention (`render::stream_event` `ToolUse Write/Edit specialization`):

- `ToolUse { name: "Write" | "Edit" }` SHALL render as `✍️ <file_path>` where `<file_path>` is the value of `input.file_path` normalized to forward slashes (e.g., `wiki/modules/auth.md`). The `input` dict shape SHALL NOT leak — only the path renders.
- `ToolUse { other }` SHALL render as `🛠️ <name>[ · <input-summary>]` where the input summary follows the existing abbreviation rules (file_path → file basename; pattern → quoted string; command → first 80 chars).

`StreamEvent::Thought { text }` events SHALL render inline within the Activity stream timeline (NOT buffered to a separate trailing block). Consecutive Thought events SHALL be folded into a single `🤔 <text>` item — the renderer SHALL maintain a running text buffer that flushes when any non-Thought event is observed AND emits one ThoughtItem per fold boundary. When the folded text contains a single line, the ThoughtItem SHALL render `🤔 <text>` on one line. When the folded text contains multiple lines, the ThoughtItem SHALL render `🤔 <first-line>` followed by a `(<N> more lines ▼)` toggle; clicking the toggle expands the remaining lines (indented) and reveals a `▲ collapse` control.

`StreamEvent::ToolResult` SHALL NOT render in this view (results are an internal flow signal — the GUI is a focused viewer, not a linear log). Deep-debug access to ToolResult bodies SHALL remain available via the Done detail's `Run details` collapsible block (which replays the full events.jsonl).

Clicking `[⏹ Cancel]` SHALL invoke `cancel_goal(run_id)`. The button SHALL transition to a `Cancelling…` disabled state immediately upon click and SHALL be replaced once the run transitions to a terminal state (cancelled / done / failed).

#### Scenario: Activity stream renders tool_use with emoji leaders

- **WHEN** the Running detail view receives two `goal-stream` events: `StreamEvent::ToolUse { name: "Read", input: { file_path: "raw/code/auth.rs" } }` then `StreamEvent::ToolUse { name: "Glob", input: { pattern: "wiki/**/*.md" } }`
- **THEN** the Activity stream block SHALL contain exactly two rendered rows in arrival order AND the first row contains `🛠️`, `Read`, AND `auth.rs` AND the second row contains `🛠️`, `Glob`, AND `wiki/**/*.md`

#### Scenario: ToolUse Write specialization renders only the file path

- **WHEN** the Running detail view receives `StreamEvent::ToolUse { name: "Write", input: { file_path: "wiki/modules/auth.md" } }`
- **THEN** the rendered row SHALL contain `✍️` AND `wiki/modules/auth.md` AND SHALL NOT contain the substring `Write` (the emoji conveys the tool) AND SHALL NOT contain `input` / `file_path` dict labels

#### Scenario: Thought chunks fold inline into a single timeline item

- **WHEN** the Running detail view receives `ToolUse(Read)`, then three sequential `StreamEvent::Thought` events with texts `"Analyzing "`, `"the auth "`, `"middleware..."`, then `ToolUse(Glob)`
- **THEN** the Activity stream renders three timeline items in order: a ToolUse row for Read, a single ThoughtItem rendering `🤔 Analyzing the auth middleware...` (the concatenation of the three Thought chunks), then a ToolUse row for Glob

#### Scenario: Multi-line Thought renders first line plus collapsible toggle

- **WHEN** the Running detail view receives `StreamEvent::Thought { text: "first line\nsecond\nthird" }`
- **THEN** the rendered ThoughtItem initially shows `🤔 first line` and a `(2 more lines ▼)` toggle AND clicking the toggle reveals the indented remainder `second\nthird` and a `▲ collapse` control

#### Scenario: Cancel button invokes cancel_goal and disables

- **WHEN** the user clicks `[⏹ Cancel]` in the Running detail view for run id `X`
- **THEN** `cancel_goal("X")` is invoked AND the button transitions to a `Cancelling…` disabled state AND the button SHALL NOT be clickable a second time

---

### Requirement: Run Detail Views — Done

The system SHALL render the `Done` detail view when the user navigates to a run whose corresponding `RunLog` row has `outcome="succeeded"`. The view SHALL include: a header with `← back`, the goal text, and a `✓ Done` badge; a metadata line with duration (finished_at − started_at), accumulated tokens, and commit sha (first 7 chars of the latest commit on `<vault>/.codebus/`); a `Covered pages` block listing pages affected by the run; a `Lint` line summarizing `lint_error_count` and `lint_warn_count`; an `Activity summary` block summarizing tool-use counts derived from the events file; AND a collapsible `Run details` block (collapsed by default) rendering the full events.jsonl replay using the same `ActivityStreamItem` view as the Running detail (Thought events render inline as ThoughtItems per the Running detail's fold rules — there is no separate "Thinking" block, since the inline timeline already exposes them).

The `Covered pages` list SHALL be derived from the events.jsonl tail-replay by collecting unique `file_path` values from `StreamEvent::ToolUse { name: "Write" | "Edit", input.file_path }` events that resolve under the vault's `wiki/` directory. Each covered page SHALL render as a clickable `[[wikilink]]` row; clicking it SHALL switch the Workspace to the `Wiki` tab with that page loaded.

The `Covered pages` list AND the `Activity summary` block SHALL be grouped by verb phase. A "phase" is bounded by the `VerbLifecycleEvent::SpawnStart { verb }` … `VerbLifecycleEvent::SpawnEnd { verb }` event pair — every `StreamEvent::ToolUse` observed between those markers belongs to that phase. Typical phases for a goal run are `goal` (the goal agent itself) and `fix` (the post-spawn lint-and-fix agent invoked from the goal verb's fix loop). When the same verb runs multiple times in a single goal run (e.g., the fix loop iterates twice), the corresponding tool-use counts SHALL be merged under one `fix` heading (one bucket per `verb`, not per spawn).

The `Activity summary` block SHALL render one labelled sub-section per phase observed in the events file, ordered by first appearance. Each sub-section SHALL contain one line per tool name observed in that phase (e.g., `12 Read`, `8 Write`). Tools that did not fire in a phase SHALL NOT render a row inside that phase's section. When a phase produced zero ToolUse events (e.g., goal agent decided not to ingest), the phase SHALL still render its heading with an em-dash or short hint line indicating no tools were used.

The `Covered pages` block SHALL likewise be grouped by phase. Each covered page row SHALL appear under the phase whose ToolUse Write/Edit produced it. Slug uniqueness is enforced across phases — if the same slug is written by both `goal` and `fix` phases (rare), the later phase wins for display ordering but both phase headings still render the row.

The `Run details` block SHALL be collapsed by default with a `Show run details ▼` / `Hide run details ▲` toggle. When expanded, it SHALL render the events.jsonl replay using the same `ActivityStreamItem` + ThoughtItem fold as the Running detail, in arrival order. This block recovers the "Stream history" surface previously deprecated by the design — collapsed-by-default keeps the Done view minimal for the common "verify outcome" case while still giving deep-debug access to the full timeline (including Thoughts inline at the moment they fired).

#### Scenario: Done detail lists covered pages from events

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains `ToolUse { name: "Write", input.file_path: "wiki/modules/auth.md" }` and `ToolUse { name: "Edit", input.file_path: "wiki/index.md" }`
- **THEN** the Done detail's `Covered pages` block lists exactly two rows: `[[auth]]` and `[[index]]` AND each row is clickable

#### Scenario: Done detail covered-page click switches to Wiki tab

- **WHEN** the user clicks a covered-page `[[slug]]` row in the Done detail view
- **THEN** the Workspace switches the active tab to `Wiki` AND `useWikiStore.currentPath` is set to that page's path AND the Milkdown preview renders that page

#### Scenario: Activity summary groups tool counts by verb phase

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains: `Lifecycle::SpawnStart { verb: "goal" }`, then 12 `ToolUse { name: "Read" }`, then `Lifecycle::SpawnEnd { verb: "goal" }`, then `Lifecycle::SpawnStart { verb: "fix" }`, then 2 `ToolUse { name: "Bash" }` + 2 `ToolUse { name: "Write" }`, then `Lifecycle::SpawnEnd { verb: "fix" }`
- **THEN** the `Activity summary` block renders two phase sub-sections — `goal` containing one line `12 Read`, AND `fix` containing two lines `2 Bash` AND `2 Write` — AND no `Write` row appears under the `goal` phase even though the fix phase wrote pages

#### Scenario: Activity summary phase with zero tool uses renders empty hint

- **WHEN** the user navigates to a `succeeded` run whose events.jsonl contains `Lifecycle::SpawnStart { verb: "goal" }` immediately followed by `Lifecycle::SpawnEnd { verb: "goal" }` (goal agent ran but invoked no tools — e.g., judged the goal out-of-scope)
- **THEN** the `Activity summary` block still renders the `goal` phase heading AND the body of that phase contains an em-dash or short hint line ("(no tools used)") AND no `ToolUse` row appears under it

#### Scenario: Covered pages groups slugs by writing phase

- **WHEN** the user navigates to a `succeeded` run whose `goal` phase wrote `wiki/modules/auth.md` AND whose `fix` phase wrote `wiki/index.md` AND `wiki/log.md`
- **THEN** the `Covered pages` block renders two phase sub-sections — `goal` containing one `[[auth]]` row AND `fix` containing two rows `[[index]]` AND `[[log]]` — AND each row remains clickable to switch to the Wiki tab

#### Scenario: Run details block is collapsed by default and replays full timeline on expand

- **WHEN** the user navigates to a `succeeded` run AND the `Run details` toggle is in the default collapsed state
- **THEN** the timeline is NOT visible AND clicking `Show run details ▼` SHALL render every event in events.jsonl using the same ActivityStreamItem + ThoughtItem fold as the Running detail, in arrival order (Thought chunks folded inline at their original position)

---

### Requirement: Run Detail Views — Cancelled and Interrupted

The system SHALL render the `Cancelled` detail view when the user navigates to a run whose corresponding `RunLog` row has `outcome="cancelled"`. The view SHALL include: a header with `← back`, the goal text, and a `⏹ Cancelled` badge; a metadata line with duration and accumulated tokens; a prominent warning block reading "Wiki has uncommitted changes — not auto-committed. Review in terminal if needed."; a `Partial timeline` section summarizing tool_use events grouped by category (reading / writing / other); and a `[Retry with same goal]` button.

The system SHALL render the `Interrupted` detail view for virtual-outcome `"interrupted"` entries (RunLog row missing for an existing events-*.jsonl file). The Interrupted view SHALL share the same layout as Cancelled but with the header badge changed to `⚠ Interrupted` and the warning text replaced with "App was closed before this goal finished. Wiki state may be partial — review in terminal if needed." The `[Retry with same goal]` button SHALL behave identically.

The `[Retry with same goal]` button SHALL extract the goal text from the run's RunLog row (Cancelled) or the events.jsonl first user-prompt event (Interrupted), pre-fill the New Goal modal with that text, and open the modal. The user SHALL still confirm the run by clicking `Run` in the modal — Retry SHALL NOT spawn a new goal directly.

#### Scenario: Cancelled detail shows uncommitted warning

- **WHEN** the user navigates to a run with `outcome="cancelled"`
- **THEN** the detail view renders a prominent warning block containing the exact substring "Wiki has uncommitted changes — not auto-committed"

#### Scenario: Interrupted virtual entry renders Interrupted view

- **WHEN** the vault contains `events-2026-05-13T03-00-00Z.jsonl` AND no corresponding row exists in `runs-*.jsonl` with `started_at` equal to `2026-05-13T03:00:00Z`
- **THEN** the Goals overview list contains a virtual entry with `⚠` icon AND clicking it navigates to the Interrupted detail view AND the warning block contains "App was closed before this goal finished"

#### Scenario: Retry pre-fills modal without spawning

- **WHEN** the user clicks `[Retry with same goal]` in a Cancelled detail view for run with `goal="describe auth flow"`
- **THEN** the New Goal modal opens AND the textarea contains exactly the text `"describe auth flow"` AND no `spawn_goal` invocation occurs until the user clicks `Run`

---

### Requirement: Wiki Tab with Collapsible File Tree

The `Wiki` tab main content area SHALL render a Milkdown editor in read-only mode displaying the markdown body of the currently-selected wiki page. A collapsible `Pages` file tree panel SHALL be rendered as a left-side column that is expanded by default; clicking a folder icon button in the Wiki tab top bar SHALL toggle the panel's visibility. When expanded, the panel SHALL list all wiki pages in `useWikiStore.pages` grouped by taxonomy folder (concepts / entities / modules / processes / synthesis) with the file basename as the row label. Clicking a row SHALL set `useWikiStore.currentPath` to that page's path and load its body via `read_wiki_page`.

The Wiki tab top bar SHALL also display the currently-selected page's title (from frontmatter `title` or the file basename if no frontmatter title).

When the vault has zero wiki pages, the main content area SHALL render a centered hint reading "No wiki pages yet — run a goal to start documenting".

#### Scenario: Wiki tab opens with file tree expanded

- **WHEN** the user clicks the `Wiki` tab for the first time after opening a vault
- **THEN** the `Pages` file tree panel IS visible on the left AND lists all pages grouped by taxonomy folder AND the Milkdown preview occupies the remaining width

#### Scenario: File tree toggle collapses the panel

- **WHEN** the user clicks the folder icon button in the Wiki tab top bar while the tree is expanded
- **THEN** the `Pages` file tree panel hides AND the Milkdown preview expands to occupy the full width of the main content area

#### Scenario: Empty vault shows wiki hint

- **WHEN** the user opens the `Wiki` tab in a vault that has no `wiki/**/*.md` files
- **THEN** the main content area renders the centered hint "No wiki pages yet — run a goal to start documenting"

---

### Requirement: Wikilink Resolution and Click Behavior

The Milkdown preview SHALL render `[[page-slug]]` syntax via a custom ProseMirror node provided by a wikilink plugin. The plugin SHALL parse `[[slug]]` in markdown content (paste rule + input rule), look up `slug` in `useWikiStore.pages` (key = filename basename without `.md` extension), and render the link in one of two states:

- Resolvable (slug exists in pages map): rendered as a colored clickable link; clicking SHALL invoke `useWikiStore.loadPage(slug)` and update `currentPath`
- Unresolvable (slug not in pages map): rendered as a dimmed disabled-style span; hover SHALL display a tooltip reading "Page not found"; clicking SHALL be a no-op

Wikilink resolution SHALL be entirely client-side using the page index loaded at Workspace mount time. The system SHALL NOT issue an IPC call per wikilink click — `read_wiki_page` is invoked only when the navigation lands on a resolvable target.

When two or more pages share the same slug (filename collision across taxonomy folders), the last entry inserted into `useWikiStore.pages` SHALL win; this matches the vault's existing slug-uniqueness expectation enforced by the wiki lint `duplicate-slug` rule.

#### Scenario: Resolvable wikilink navigates to page

- **WHEN** the Milkdown preview contains `[[uv-lib]]` AND `useWikiStore.pages["uv-lib"]` exists AND the user clicks the link
- **THEN** `useWikiStore.currentPath` updates to the resolved page path AND the Milkdown preview re-renders with that page's body

#### Scenario: Unresolvable wikilink renders disabled and tooltip

- **WHEN** the Milkdown preview contains `[[nonexistent-page]]` AND `useWikiStore.pages["nonexistent-page"]` does not exist
- **THEN** the link renders with a dimmed visual style AND hovering the link displays a tooltip with the text "Page not found" AND clicking the link is a no-op (no IPC invocation, no navigation)

#### Scenario: Wikilink resolution is client-side only

- **WHEN** the user clicks any wikilink in the Milkdown preview
- **THEN** the resolution SHALL look up the slug in `useWikiStore.pages` in memory AND SHALL NOT issue a `list_wiki_pages` or other page-index IPC call

---

### Requirement: Tauri IPC Commands for Goal Lifecycle and Wiki Read

The system SHALL register six new Tauri commands beyond the foundation's nine commands, bringing the total to fifteen:

- `spawn_goal(vault_path: String, goal_text: String) -> Result<String, AppError>` — spawn a background thread that invokes `codebus_core::verb::goal::run_goal` with the given vault and goal text. The function SHALL allocate an `Arc<AtomicBool>` cancel flag, store it in `AppState.active_runs` keyed by the new `RunId` (where `RunId` equals the run's `started_at` slug), and emit each `VerbEvent` produced by the closure to a Tauri event channel named `"goal-stream"` with payload `{ run_id: String, event: VerbEvent }`. On thread completion (success, failure, or panic), the entry SHALL be removed from `active_runs`.

- `cancel_goal(run_id: String) -> Result<(), AppError>` — look up the cancel flag in `active_runs` by `run_id`; if found, `store(true, Ordering::Relaxed)` and return `Ok(())`. If not found (run already terminated), return `Ok(())` (idempotent).

- `list_runs(vault_path: String, mode_filter: ModeFilter) -> Result<Vec<RunLogSummary>, AppError>` — read all `runs-*.jsonl` files under `<vault>/.codebus/log/`, parse each row to `RunLogSummary`, apply `mode_filter` (`Goal` keeps only `mode=="goal"`; `All` keeps everything), then scan `events-*.jsonl` files for interrupted detection per the next requirement, merge virtual entries, and return the combined list sorted by `started_at` descending.

- `get_run_detail(vault_path: String, run_id: String) -> Result<RunDetail, AppError>` — find the matching `RunLogSummary` (real or virtual interrupted), open the corresponding `events-*.jsonl`, replay all events into `Vec<RecordedEvent>`, and return `RunDetail { summary, events }`.

- `list_wiki_pages(vault_path: String) -> Result<Vec<WikiPageMeta>, AppError>` — glob `<vault>/.codebus/wiki/**/*.md`, parse each file's frontmatter to extract `title`, derive slug from the filename (without `.md`), and return one `WikiPageMeta { slug, path, title }` per file. Files without parseable frontmatter SHALL still be returned with `title` equal to the slug.

- `read_wiki_page(vault_path: String, page_slug: String) -> Result<String, AppError>` — look up the page by slug among the wiki files, read its raw bytes, strip the leading frontmatter block (delimited by `---\n...\n---\n` at the start), and return the remaining markdown body as a `String`. If the slug does not match any wiki file, return `AppError::Invalid { field: "page_slug", message: "no such page" }`.

`ModeFilter` SHALL be a serde-tagged enum with variants `Goal` and `All` (snake_case).

`AppError` SHALL be the same discriminated union used by the foundation's commands — no new variants added by this change.

#### Scenario: spawn_goal returns run id derived from started_at

- **WHEN** the frontend calls `invoke("spawn_goal", { vault_path: "/some/vault", goal_text: "X" })` AND the spawned `run_goal` invocation's first stream event timestamps the run at `2026-05-13T14:56:21Z`
- **THEN** the IPC call resolves with `"2026-05-13T14-56-21Z"` AND a corresponding entry exists in `AppState.active_runs` keyed by that string

#### Scenario: cancel_goal idempotent on unknown run

- **WHEN** the frontend calls `invoke("cancel_goal", { run_id: "nonexistent" })` AND `active_runs` contains no such key
- **THEN** the IPC call resolves with `Ok(())` without error

#### Scenario: list_runs filters by mode

- **WHEN** the frontend calls `invoke("list_runs", { vault_path: ..., mode_filter: { kind: "goal" } })` AND the vault's `runs-*.jsonl` contain three goal rows, two chat rows, and one fix row
- **THEN** the returned `Vec<RunLogSummary>` length is 3 AND every entry has `mode == "goal"`

#### Scenario: read_wiki_page strips frontmatter

- **WHEN** the frontend calls `invoke("read_wiki_page", { vault_path: ..., page_slug: "uv-lib" })` AND the file at `<vault>/.codebus/wiki/modules/uv-lib.md` contains a frontmatter block followed by markdown body
- **THEN** the IPC returns the markdown body string without the leading `---\n...\n---\n` block

---

### Requirement: Interrupted Run Detection

The system SHALL detect interrupted goal runs by comparing `events-*.jsonl` files against `runs-*.jsonl` rows at workspace mount time (via the `list_runs` IPC). For each `events-<started_at_slug>.jsonl` file present under `<vault>/.codebus/log/`, the system SHALL search the `runs-*.jsonl` files for a row whose `started_at` (slugged identically) matches. When no matching row exists, the system SHALL synthesize a virtual `RunLogSummary` with `outcome="interrupted"`, `started_at` derived from the slug, `goal` extracted from the events file's first user-input or banner event, and `mode="goal"` (interrupted detection applies to goal-mode runs only — chat/query/fix interrupted detection is out of scope at v1).

The virtual entry SHALL NOT be written back to any `runs-*.jsonl` file — it exists only in the IPC response. Subsequent re-invocations of `list_runs` SHALL re-derive the virtual entry from the same on-disk state.

If the same events file later gains a matching RunLog row (e.g., because the original `run_goal` process recovered and wrote its terminal RunLog late), the virtual entry SHALL no longer appear in `list_runs` output — the real row supersedes it.

#### Scenario: Orphan events file produces virtual interrupted entry

- **WHEN** `list_runs` is invoked AND the vault contains `events-2026-05-13T03-00-00Z.jsonl` AND no `runs-*.jsonl` row has `started_at == "2026-05-13T03:00:00Z"`
- **THEN** the returned list contains a virtual entry with `outcome == "interrupted"` AND `started_at == "2026-05-13T03:00:00Z"` AND no row is appended to any `runs-*.jsonl` file on disk

#### Scenario: Real RunLog row supersedes virtual interrupted

- **WHEN** `events-2026-05-13T03-00-00Z.jsonl` exists AND a `runs-2026-05-13.jsonl` row is appended with `started_at == "2026-05-13T03:00:00Z"` and `outcome == "cancelled"` AND `list_runs` is invoked
- **THEN** the returned list contains the real row (`outcome="cancelled"`) AND does NOT contain a virtual `outcome="interrupted"` entry for the same started_at

---

### Requirement: One Active Goal Run At A Time

The system SHALL enforce that at most one goal-mode `run_goal` invocation is active per vault per app session. This invariant SHALL be enforced at two layers:

- Frontend (`useGoalsStore`): exposes an `activeRun` field that is non-null when a spawn is in progress; New Goal modal `Run` button is disabled while `activeRun != null` (per the `New Goal Modal Flow` requirement)
- Backend (`AppState.active_runs`): `spawn_goal` SHALL return `AppError::Invalid { field: "active_runs", message: "another goal run is already active" }` when invoked while `active_runs` is non-empty (and the existing key corresponds to a goal-mode run, not a `chat`/`query`/`fix` run which v1 does not spawn via app-workspace anyway).

This invariant applies per app session within a single vault; switching vaults (back to lobby then opening a different vault) does not carry the constraint across.

#### Scenario: Second spawn_goal during active run rejected at backend

- **WHEN** a goal run is currently active for vault `V` (an entry exists in `active_runs`) AND the frontend invokes `spawn_goal` with the same vault
- **THEN** the IPC call rejects with `AppError` having `kind: "invalid"`, `field: "active_runs"`, AND `message` containing the substring "already active"

#### Scenario: Spawn allowed after cancel completes

- **WHEN** a goal run is active AND `cancel_goal` is invoked AND the background thread observes the flag, kills the child, removes the entry from `active_runs`, AND emits a final `goal-stream` event signaling termination
- **THEN** a subsequent `spawn_goal` invocation succeeds AND a new run id is returned
