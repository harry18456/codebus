# interactive-tutorial Specification

## Purpose

TBD - created by archiving change 'r-01-station-board'. Update Purpose after archive.

## Requirements

### Requirement: Station routing schema uses stable station id as URL key

The R-01 station-board frontend SHALL expose two route patterns for tutorials produced by the Module 5 Generator:

1. `/tutorial/{workspace_id}` — the MOC (table-of-contents) page rendered from `<workspace>/codebus-tutorials/{task_id}/tutorial.md`. (Nuxt file routing maps `pages/tutorial/[workspace_id]/index.vue` to this bare path; a trailing `/index` segment would route into `[station_id].vue` and fail the station_id regex.)
2. `/tutorial/{workspace_id}/{station_id}` — a single station page rendered from the corresponding `<workspace>/codebus-tutorials/{task_id}/stations/{station_id}.md`.

`{workspace_id}` MUST be the sidecar-derived `ws_<12-hex>` identifier produced by `auth.service.workspace_id_for_path` (returned to the frontend in `GrantResponse.workspace_id`). `{station_id}` MUST be the D-029 stable station id of the form `s\d{2}-[a-z0-9-]{1,40}(-\d+)?` (matching the regex enforced in `kb-growth` and `module-5-generator` capabilities). Numeric station indices MUST NOT appear in the URL path because indices drift when stations are reordered after regeneration.

The page setup logic MUST refuse to render a station page when the `station_id` URL parameter does not match the regex; the response MUST be a friendly error view that links back to the MOC. Direct paste of an unlocked-but-completed `station_id` URL MUST be allowed (already-completed stations are revisitable; see `Unlock logic` Requirement).

#### Scenario: MOC route renders tutorial.md as the index page

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5`
- **THEN** the page MUST load `route.json` from `<workspace>/codebus-tutorials/{task_id}/route.json` and render `tutorial.md` (the MOC) as the body
- **AND** every station name in the MOC MUST be a hyperlink targeting `/tutorial/{workspace_id}/{station_id}` using the `station_id` value from `route.json.stations[*].station_id`

#### Scenario: Station route loads the matching markdown file by stable id

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5/s02-mqtt-client`
- **THEN** the page MUST resolve `s02-mqtt-client.md` via `route.json.stations[*]` lookup (matching `station_id`) and load `<workspace>/codebus-tutorials/{task_id}/stations/s02-mqtt-client.md`
- **AND** the rendered content MUST include the body of the markdown file with frontmatter stripped (frontmatter is consumed separately by `<StationLayout>`)

#### Scenario: Numeric index in URL path is rejected

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5/2` (numeric index)
- **THEN** the route handler MUST refuse to render the station page (regex mismatch)
- **AND** the page MUST display a friendly error linking back to the MOC at `/tutorial/{workspace_id}`

#### Scenario: Invalid station_id format triggers safe fallback

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5/foo-bar` (does not match `s\d{2}-[a-z0-9-]+` regex)
- **THEN** the route handler MUST refuse to render the page and display the same friendly error linking back to the MOC

#### Scenario: Index page falls back to latest task when ?task query missing

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5` without any `?task=...` query parameter
- **THEN** the page MUST scan `<workspace>/codebus-tutorials/*/` for existing task directories
- **AND** if exactly one task directory exists, the page MUST select that task implicitly without showing a selector UI
- **AND** if multiple task directories exist, the page MUST select the one with the most recent `tutorial.md` frontmatter `generated_at` timestamp (falling back to directory mtime when frontmatter is missing)
- **AND** if zero task directories exist, the page MUST render the empty CTA defined in the `MOC renders tutorial.md as the index page` Requirement instead of any error view

#### Scenario: Index page honors explicit ?task query when valid

- **WHEN** the user navigates to `/tutorial/ws_a3f2b1c8d4e5?task=generate_a3f2b1c8`
- **AND** the value matches the regex `^generate_[0-9a-f]{8}$`
- **AND** the directory `<workspace>/codebus-tutorials/generate_a3f2b1c8/` exists
- **THEN** the page MUST load that task's `tutorial.md` and `route.json` directly without scanning sibling directories
- **WHEN** the value matches the regex but the directory does not exist
- **THEN** the page MUST fall through to the implicit-latest scan as if the query were absent (same scenario as above)


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: Three mdc interactive components with strict prop contracts

The frontend SHALL register exactly three custom components under `web/app/components/content/` so that `@nuxtjs/mdc` auto-mounts them when rendering Generator output: `<Checkpoint>`, `<Quiz>`, and `<QAEntry>`. The component prop signatures SHALL exactly match the contracts specified below; loose typing or runtime shape coercion MUST NOT be used.

**`<Checkpoint id="...">` props (TypeScript)**
```typescript
interface CheckpointProps {
  id: string  // MUST match /^station-\d+-check$/ or /^s\d+-check-\d+$/
}
```

The component MUST render the slotted markdown checkbox list (`- [ ]` items) as interactive checkboxes. Each checkbox tick MUST call `useTutorialProgress().setCheckpoint(id, item_index, checked)`. When all items are checked, the Checkpoint as a whole is "passed" and the component MUST emit a visual indicator (e.g., a checkmark badge).

**`<Quiz id="..." correct="...">` props (TypeScript)**
```typescript
interface QuizProps {
  id: string                 // MUST match /^s\d+-q\d+$/
  correct: 'a' | 'b' | 'c' | 'd'  // TypeScript Literal union — generator MUST produce one of these
}
```

The component MUST render the slotted markdown options (`- a) ...`, `- b) ...`, etc.) as radio buttons. On submission, the component MUST call `useTutorialProgress().setQuizAnswer(id, selectedOption)`; if `selectedOption === correct`, the answer is recorded as correct. Wrong answers MUST allow retry (no answer reveal); `attempts` MUST increment on every submit.

**`<QAEntry prompt="...">` props (TypeScript)**
```typescript
interface QAEntryProps {
  prompt: string  // pre-filled question text passed to the Q&A Agent on click
}
```

The component MUST render as a clickable button with the slot content as its label. Clicking MUST navigate to the Q&A Agent (Module 8) with `prompt` pre-filled; in P0, this is a `router.push('/qa?prompt=' + encodeURIComponent(prompt))` placeholder route, with the actual Q&A page wiring deferred to a follow-up change.

#### Scenario: Checkpoint full-tick records progress and emits indicator

- **WHEN** the user ticks every item in `<Checkpoint id="station-2-check">`
- **THEN** every tick MUST call `useTutorialProgress().setCheckpoint('station-2-check', item_index, true)`
- **AND** the component MUST render a visual "passed" indicator after the last tick
- **AND** the corresponding `progress.checkpoints['station-2-check']` MUST be `{ done: true, ts: <ISO 8601 UTC> }`

#### Scenario: Quiz wrong answer allows retry without revealing correct option

- **WHEN** the user submits answer "a" to `<Quiz id="s2-q1" correct="b">`
- **THEN** the component MUST display "再試一次" feedback
- **AND** the component MUST NOT reveal that the correct answer is "b"
- **AND** `progress.quizzes['s2-q1']` MUST be `{ answer: 'a', correct: false, attempts: 1 }` (or attempts incremented from prior value)
- **WHEN** the user resubmits with answer "b"
- **THEN** the component MUST mark the quiz as passed and `progress.quizzes['s2-q1']` MUST become `{ answer: 'b', correct: true, attempts: 2 }`

#### Scenario: TypeScript rejects invalid Quiz correct value at compile time

- **WHEN** a station markdown contains `<Quiz id="s2-q1" correct="e">` (literal "e" not in `'a' | 'b' | 'c' | 'd'`)
- **THEN** `npm run typecheck` MUST fail with a Literal type mismatch error
- **AND** the offending markdown file path SHOULD appear in the error trail (or be discoverable via Generator output validation logs)

#### Scenario: QAEntry navigates to Q&A page with pre-filled prompt

- **WHEN** the user clicks `<QAEntry prompt="這段 retry 策略為什麼不會產生重複扣款？">`
- **THEN** the component MUST call `router.push('/qa?prompt=' + encodeURIComponent('這段 retry 策略為什麼不會產生重複扣款？'))`
- **AND** the QAEntry MUST NOT itself fetch any sidecar endpoint (the Q&A page handles all IPC; QAEntry is a navigation trigger only)


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: progress.json schema and single-writer path

The frontend SHALL persist tutorial progress to `<workspace>/codebus-tutorials/{task_id}/progress.json`. The file SHALL be the **canonical source of truth** for tutorial completion state; localStorage / sessionStorage / IndexedDB MUST NOT cache this state.

**Schema (TypeScript)**
```typescript
interface TutorialProgress {
  current_station_id: string | null         // null when no station has been visited yet
  completed_station_ids: string[]           // ordered by completion time
  checkpoints: Record<string, {             // key MUST match /^station-\d+-check$/ or /^s\d+-check-\d+$/
    done: boolean
    ts: string                              // ISO 8601 UTC, set when done flipped to true
  }>
  quizzes: Record<string, {                 // key MUST match /^s\d+-q\d+$/
    answer: string                          // last submitted answer
    correct: boolean
    attempts: number                        // monotonically incremented
  }>
}
```

The composable `useTutorialProgress` SHALL be the **only** writer of `progress.json`. All UI components mutating progress MUST go through `useTutorialProgress().setCheckpoint(...)`, `.setQuizAnswer(...)`, or `.setCurrentStation(...)`; no other code path SHALL invoke the underlying Tauri `write_progress_file` command directly. This is enforced by source-level grep in defensive tests.

The composable MUST debounce writes (~500 ms) to avoid IPC thrash on rapid checkbox toggling, and MUST flush immediately on `beforeunload` browser event so that progress survives App close.

#### Scenario: progress.json absent on first visit creates empty schema

- **WHEN** the user lands on a tutorial route for the first time and `progress.json` does not yet exist
- **THEN** `useTutorialProgress` MUST initialize in-memory state to `{ current_station_id: null, completed_station_ids: [], checkpoints: {}, quizzes: {} }`
- **AND** the file MUST NOT be created until the first state-mutating call (e.g., `setCheckpoint`)

#### Scenario: Single-writer invariant enforced by source grep

- **WHEN** the test suite greps `web/app/` for `\.writeProgressFile\(` (the public method on `useTutorialFiles`)
- **THEN** matches MUST be found only inside `web/app/composables/useTutorialProgress.ts`
- **AND** the underlying Tauri `invoke('write_progress_file', ...)` call MAY appear inside `web/app/composables/useTutorialFiles.ts` as the IPC wrapper, but no other file MAY invoke it directly nor call `useTutorialFiles().writeProgressFile(...)`
- **AND** any other file calling `writeProgressFile` MUST cause the test to fail

#### Scenario: localStorage / sessionStorage / cookies never store progress data

- **WHEN** the test suite greps `web/app/composables/useTutorialProgress.ts` (and any related files) for `localStorage` / `sessionStorage` / `IndexedDB` / `document.cookie` references
- **THEN** zero matches MUST be found

#### Scenario: beforeunload flushes pending debounced writes

- **WHEN** the user toggles a checkpoint and immediately closes the App (within the 500 ms debounce window)
- **THEN** `useTutorialProgress` MUST register a `beforeunload` listener that flushes the pending state to `progress.json` synchronously
- **AND** the file content after re-opening the App MUST reflect the latest user action (no progress lost)


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: Unlock logic gates next-station access on completion

The frontend SHALL compute the set of unlocked station ids client-side as a Vue `computed` derivation of `route.json` and the in-memory `progress.json` state. No server-side / sidecar call MUST be involved in unlock determination.

**Algorithm**:
1. The first station in `route.json.stations` is unconditionally unlocked.
2. For each station `S` in `route.json.stations` order: if `S.station_id` is unlocked AND `is_complete(S, progress)` is true, then unlock the next station in the order. Stop scanning when a non-unlocked station is encountered.
3. `is_complete(S, progress)`: every id in `S.required_checks` MUST be either (a) `progress.checkpoints[id].done === true`, or (b) `progress.quizzes[id].correct === true`.

**Already-completed station revisitability**: a station id present in `progress.completed_station_ids` MUST be reachable via direct URL paste regardless of unlock state; the unlock logic ONLY gates "navigation forward to a never-before-visited station." Already-completed stations render in read-only review mode (interactions still recorded, but no further unlock side-effects).

#### Scenario: First station always unlocked on fresh visit

- **WHEN** the user lands on `/tutorial/ws_xxx/index` with empty `progress.json` (first visit)
- **THEN** `route.json.stations[0].station_id` MUST appear in the unlocked set
- **AND** `route.json.stations[1].station_id` MUST NOT appear in the unlocked set (until station 0 is completed)

#### Scenario: Station completion unlocks the next station

- **WHEN** the user completes all `required_checks` for `route.json.stations[0]` (every Checkpoint and Quiz id passes)
- **THEN** `route.json.stations[1].station_id` MUST appear in the unlocked set
- **AND** the user MUST be able to navigate to `/tutorial/ws_xxx/{station_id_of_index_1}` without seeing the locked-state error

#### Scenario: Locked station URL paste shows lock screen

- **WHEN** the user pastes `/tutorial/ws_xxx/{station_id_of_index_3}` into the URL while only stations 0 and 1 are unlocked AND station_id_of_index_3 is NOT in `progress.completed_station_ids`
- **THEN** the page MUST render a "this station is locked" view linking back to the current station or MOC
- **AND** the user MUST NOT see the station markdown content

#### Scenario: Already-completed station revisitable via URL paste

- **WHEN** `route.json.stations[0].station_id` is in `progress.completed_station_ids` AND the user pastes that URL while currently on station 2
- **THEN** the page MUST render the station markdown content (allowed despite the unlock-forward window having moved past)
- **AND** the page MUST render in "review mode" (a visible indicator e.g. badge that this station is already complete)


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: frontmatter parser drives StationLayout shell

The frontend SHALL parse station markdown frontmatter using `gray-matter` (or an equivalent strict YAML frontmatter parser) and pass the parsed object as props to `<StationLayout>`. The frontmatter schema is defined by `module-5-generator` capability and includes (at minimum) `schema_version`, `station_id`, `station_index`, `title`, `duration_minutes`, `workspace_type`, `repo_name`, `task`, `generated_at`, `related_stations`, `required_checks`, and `degraded`.

`<StationLayout>` SHALL render at least the following from frontmatter:
- `title` as the station heading
- `station_index` and total station count (`route.json.stations.length`) as a "第 N / M 站" progress indicator
- `duration_minutes` as a duration badge
- `degraded === true` MUST trigger a visible warning badge ("本站產出失敗，請重跑")

The frontmatter parsing logic MUST NOT throw on missing optional fields; missing required fields (`station_id`, `title`) MUST trigger a fallback "本站 frontmatter 損毀" error view linking back to MOC (consistent with R-1 risk mitigation in design.md).

#### Scenario: StationLayout renders title and station_index from frontmatter

- **WHEN** a station markdown loads with frontmatter `{ station_id: "s02-mqtt-client", station_index: 2, title: "MQTT Client", duration_minutes: 15 }` and `route.json.stations.length === 5`
- **THEN** `<StationLayout>` MUST render the heading "MQTT Client"
- **AND** the progress indicator MUST display "第 2 / 5 站"
- **AND** a duration badge MUST display "15 分鐘"

#### Scenario: degraded frontmatter triggers warning badge

- **WHEN** a station markdown loads with frontmatter `{ ..., degraded: true }`
- **THEN** `<StationLayout>` MUST render a visible warning badge with text "本站產出失敗，請重跑" or equivalent
- **AND** the warning MUST be visually distinct (e.g., warning-color background) from regular station chrome

#### Scenario: Missing required frontmatter field triggers safe fallback

- **WHEN** a station markdown loads but `gray-matter` parses frontmatter without `station_id` or `title`
- **THEN** the page MUST render a "本站 frontmatter 損毀" error view linking back to the MOC
- **AND** the page MUST NOT crash or render partial content


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: MOC renders tutorial.md as the index page

The MOC page (`/tutorial/{workspace_id}`) SHALL load `<workspace>/codebus-tutorials/{task_id}/tutorial.md` and render it through `@nuxtjs/mdc`. The rendered MOC MUST display:

1. The list of stations from `route.json.stations` (each with title and duration)
2. A hyperlink per station targeting `/tutorial/{workspace_id}/{station_id}` (URL fragment uses stable station id, never numeric index)
3. A visual "locked / unlocked / completed" badge per station derived from the unlock logic (Requirement above)
4. Inline `<QAEntry>` components if Generator inserted them at "值得延伸探索" sections

The MOC page MUST NOT contain any direct file-path references that bypass `useTutorialFiles().readTutorialFile`; all file reads MUST go through the Tauri command path-validation layer.

#### Scenario: MOC station links use stable station id

- **WHEN** the MOC page renders for a workspace with `route.json.stations = [{station_id: "s01-overview", ...}, {station_id: "s02-mqtt-client", ...}]`
- **THEN** the rendered HTML MUST contain `<a href="/tutorial/{workspace_id}/s01-overview">` and `<a href="/tutorial/{workspace_id}/s02-mqtt-client">`
- **AND** the rendered HTML MUST NOT contain `<a href="/tutorial/{workspace_id}/0">` or any numeric-index link form

#### Scenario: MOC visualizes unlock state per station

- **WHEN** the MOC page renders with `progress.completed_station_ids = ["s01-overview"]` and `route.json.stations.length === 5`
- **THEN** the entry for `s01-overview` MUST render with a "completed" badge
- **AND** the entry for `s02-mqtt-client` MUST render with an "unlocked / current" badge
- **AND** the entries for `s03-...` through `s05-...` MUST render with a "locked" badge

#### Scenario: MOC file reads route through path-validated Tauri command

- **WHEN** the MOC page mounts
- **THEN** the file load MUST go through `useTutorialFiles().readTutorialFile(workspace_root, "codebus-tutorials/{task_id}/tutorial.md")`
- **AND** the path MUST NOT be passed to a raw `fetch('file://...')` or any other non-path-validated read mechanism

#### Scenario: Empty workspace shows generate CTA instead of error

- **WHEN** the MOC page mounts on a workspace where `<workspace>/codebus-tutorials/` is absent or empty (no task directories)
- **THEN** the page MUST render an empty-state CTA panel with the heading "此 workspace 尚無已產出的教材"
- **AND** the panel MUST contain instructions naming the `POST /generate` endpoint as the next step (with a debug-friendly curl example)
- **AND** the panel MUST NOT render an error icon, "load failed" wording, or any framing that suggests a fault
- **AND** the panel MUST NOT itself invoke `POST /generate` (the empty CTA only displays guidance; triggering generation belongs to a future workspace dashboard change in step 28+)


<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->

---
### Requirement: Sub-page navigation within station markdown

`<StationContent>` SHALL split the station markdown body on `^###\s+` headings into ordered chunks. The chunks SHALL be rendered one at a time as slides; chunk 0 SHALL be displayed on initial page mount. The component SHALL register page-level keyboard listeners so that `ArrowDown` / `PageDown` advance to the next chunk and `ArrowUp` / `PageUp` retreat to the previous chunk; the listeners MUST NOT trigger when the active focus target is an `<input>`, `<textarea>`, or `[contenteditable]` element. A visible progress indicator (e.g., "第 N / M 頁") MUST appear at the bottom of the content area.

The chunk index MUST NOT be encoded in the URL; URLs remain station-level (the `{station_id}` segment is the only D-029 stable id that participates in routing). When the user navigates to a different station via `<StationNav>`, the chunk index MUST reset to 0.

A station markdown without any `^###\s+` headings counts as a single chunk; in that case the keyboard listeners and progress indicator MUST still mount but operate as no-ops with the indicator showing "第 1 / 1 頁".

#### Scenario: Initial mount shows chunk 0

- **WHEN** the user navigates to a station page whose markdown body contains three `### ...` sections
- **THEN** the rendered content area MUST display only the markdown between the start of body and the first `###` heading boundary that follows it (chunk 0)
- **AND** the bottom progress indicator MUST display "第 1 / 3 頁"

#### Scenario: ArrowDown advances chunk index

- **WHEN** chunk 0 is currently displayed and the user presses `ArrowDown` (focus is on the document body, not an input)
- **THEN** the rendered content area MUST switch to display chunk 1 (the markdown between the first and second `###` boundaries)
- **AND** the progress indicator MUST update to "第 2 / 3 頁"

#### Scenario: ArrowUp on chunk 0 is a no-op

- **WHEN** chunk 0 is currently displayed and the user presses `ArrowUp`
- **THEN** the rendered content MUST remain on chunk 0
- **AND** the progress indicator MUST remain "第 1 / 3 頁"

#### Scenario: ArrowDown on last chunk is a no-op

- **WHEN** the last chunk is currently displayed and the user presses `ArrowDown`
- **THEN** the rendered content MUST remain on the last chunk (does not wrap or advance)

#### Scenario: Keyboard listener does not trigger when focus is on an input

- **WHEN** the user focuses a `<Quiz>` radio input or any `<input>` / `<textarea>` / `[contenteditable]` element and presses `ArrowDown`
- **THEN** the chunk index MUST NOT change
- **AND** the keyboard event MUST be allowed to reach the focused control (the page-level listener MUST NOT call `preventDefault` in this case)

#### Scenario: Cross-station navigation resets chunk index

- **WHEN** the user is on chunk 2 of station `s02-mqtt-client` and clicks `s03-broker-selection` in `<StationNav>`
- **THEN** the new station page MUST mount with chunk index 0 displayed
- **AND** the progress indicator MUST reflect the new station's chunk count

#### Scenario: Station markdown without ### headings counts as one chunk

- **WHEN** a station markdown body contains zero `^###\s+` headings
- **THEN** the rendered content area MUST display the entire body as chunk 0
- **AND** the progress indicator MUST display "第 1 / 1 頁"
- **AND** keyboard `ArrowDown` / `ArrowUp` MUST be received but result in no chunk change
- **AND** the path MUST NOT be passed to a raw `fetch('file://...')` or any other non-path-validated read mechanism

<!-- @trace
source: r-01-station-board
updated: 2026-04-28
code:
  - .spectra.yaml
-->
