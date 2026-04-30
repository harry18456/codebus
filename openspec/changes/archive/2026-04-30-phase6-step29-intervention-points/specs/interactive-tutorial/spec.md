## MODIFIED Requirements

### Requirement: progress.json schema and single-writer path

The frontend SHALL persist tutorial progress to `<workspace>/codebus-tutorials/{task_id}/progress.json`. The file SHALL be the **canonical source of truth** for tutorial completion state; localStorage / sessionStorage / IndexedDB MUST NOT cache this state.

**Schema (TypeScript)**
```typescript
interface TutorialProgress {
  current_station_id: string | null         // null when no station has been visited yet
  completed_station_ids: string[]           // ordered by completion time
  skipped_station_ids: string[]             // ordered by skip time; mutually exclusive with completed_station_ids
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

The composable `useTutorialProgress` SHALL be the **only** writer of `progress.json`. All UI components mutating progress MUST go through `useTutorialProgress().setCheckpoint(...)`, `.setQuizAnswer(...)`, `.setCurrentStation(...)`, or `.markStationSkipped(...)`; no other code path SHALL invoke the underlying Tauri `write_progress_file` command directly. This is enforced by source-level grep in defensive tests.

The composable MUST debounce writes (~500 ms) to avoid IPC thrash on rapid checkbox toggling, and MUST flush immediately on `beforeunload` browser event so that progress survives App close.

The `skipped_station_ids` field is **additive** — `useTutorialProgress` MUST treat any `progress.json` lacking the field as `skipped_station_ids: []` on read (no migration step required, no error raised on missing field).

A station id MUST NOT appear in both `completed_station_ids` and `skipped_station_ids` at the same time. When a previously-skipped station is later completed (e.g., the user re-opens it and finishes its checkpoints), `useTutorialProgress` MUST remove the id from `skipped_station_ids` before adding it to `completed_station_ids` in the same write.

#### Scenario: progress.json absent on first visit creates empty schema

- **WHEN** the user lands on a tutorial route for the first time and `progress.json` does not yet exist
- **THEN** `useTutorialProgress` MUST initialize in-memory state to `{ current_station_id: null, completed_station_ids: [], skipped_station_ids: [], checkpoints: {}, quizzes: {} }`
- **AND** the file MUST NOT be created until the first state-mutating call (e.g., `setCheckpoint`, `markStationSkipped`)

#### Scenario: progress.json missing skipped_station_ids field reads as empty list

- **WHEN** `useTutorialProgress` reads a `progress.json` written before this field was introduced (object lacks `skipped_station_ids` key entirely)
- **THEN** in-memory state MUST initialize `skipped_station_ids` to `[]`
- **AND** the read MUST NOT raise an error or trigger a console warning
- **AND** the next state-mutating write MUST persist the field (so the on-disk file converges to the new schema after first interaction)

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

#### Scenario: Mutual exclusion between completed and skipped enforced on transition

- **WHEN** a station id `s03-foo` is currently in `progress.skipped_station_ids` AND the user later opens that station and completes all its `required_checks`
- **THEN** the next `progress.json` write MUST remove `s03-foo` from `skipped_station_ids` and add it to `completed_station_ids` in the same write
- **AND** `s03-foo` MUST NOT appear in both lists in any persisted intermediate state

### Requirement: Unlock logic gates next-station access on completion

The frontend SHALL compute the set of unlocked station ids client-side as a Vue `computed` derivation of `route.json` and the in-memory `progress.json` state. No server-side / sidecar call MUST be involved in unlock determination.

**Algorithm**:
1. The first station in `route.json.stations` is unconditionally unlocked.
2. For each station `S` in `route.json.stations` order: if `S.station_id` is unlocked AND `is_done(S, progress)` is true, then unlock the next station in the order. Stop scanning when a non-unlocked station is encountered.
3. `is_done(S, progress)`: returns true when `S.station_id` is present in `progress.completed_station_ids` OR `progress.skipped_station_ids`. (Skipping a station counts as "done enough" for the purpose of unlocking the next station; the user has explicitly indicated they do not need to learn this one.)
4. `is_completed(S, progress)`: every id in `S.required_checks` MUST be either (a) `progress.checkpoints[id].done === true`, or (b) `progress.quizzes[id].correct === true`. This stricter predicate is what `useTutorialProgress` watches before promoting a station id from "in progress" into `completed_station_ids`.

**Already-completed station revisitability**: a station id present in `progress.completed_station_ids` OR `progress.skipped_station_ids` MUST be reachable via direct URL paste regardless of unlock state; the unlock logic ONLY gates "navigation forward to a never-before-visited station." Already-completed stations render in read-only review mode (interactions still recorded, but no further unlock side-effects). Skipped stations render in normal mode (the user MAY want to actually learn the station now), and completing the station while in this mode MUST flip the id from `skipped_station_ids` to `completed_station_ids` per the schema mutual-exclusion rule.

#### Scenario: First station always unlocked on fresh visit

- **WHEN** the user lands on `/tutorial/ws_xxx/index` with empty `progress.json` (first visit)
- **THEN** `route.json.stations[0].station_id` MUST appear in the unlocked set
- **AND** `route.json.stations[1].station_id` MUST NOT appear in the unlocked set (until station 0 is done)

#### Scenario: Station completion unlocks the next station

- **WHEN** the user completes all `required_checks` for `route.json.stations[0]` (every Checkpoint and Quiz id passes)
- **THEN** `route.json.stations[1].station_id` MUST appear in the unlocked set
- **AND** the user MUST be able to navigate to `/tutorial/ws_xxx/{station_id_of_index_1}` without seeing the locked-state error

#### Scenario: Station skip unlocks the next station

- **WHEN** the user skips `route.json.stations[0]` via the skip button (so `route.json.stations[0].station_id` is in `progress.skipped_station_ids` but NOT in `progress.completed_station_ids`)
- **THEN** `route.json.stations[1].station_id` MUST appear in the unlocked set
- **AND** the user MUST be able to navigate to `/tutorial/ws_xxx/{station_id_of_index_1}` without seeing the locked-state error

#### Scenario: Locked station URL paste shows lock screen

- **WHEN** the user pastes `/tutorial/ws_xxx/{station_id_of_index_3}` into the URL while only stations 0 and 1 are unlocked AND station_id_of_index_3 is NOT in `progress.completed_station_ids` NOR `progress.skipped_station_ids`
- **THEN** the page MUST render a "this station is locked" view linking back to the current station or MOC
- **AND** the user MUST NOT see the station markdown content

#### Scenario: Already-completed station revisitable via URL paste in review mode

- **WHEN** `route.json.stations[0].station_id` is in `progress.completed_station_ids` AND the user pastes that URL while currently on station 2
- **THEN** the page MUST render the station markdown content (allowed despite the unlock-forward window having moved past)
- **AND** the page MUST render in "review mode" (a visible indicator e.g. badge that this station is already complete)

#### Scenario: Skipped station revisitable via URL paste in normal mode

- **WHEN** `route.json.stations[0].station_id` is in `progress.skipped_station_ids` AND the user pastes that URL while currently on station 2
- **THEN** the page MUST render the station markdown content (allowed despite the unlock-forward window having moved past)
- **AND** the page MUST render in normal mode (no review badge), so the user can complete its `required_checks` if they choose to learn it now
- **AND** completing the `required_checks` while on this page MUST trigger the schema mutual-exclusion transition (id moves from `skipped_station_ids` to `completed_station_ids`)

## ADDED Requirements

### Requirement: Skip station from station page marks station as skipped without completion

The frontend SHALL expose a "skip this station" affordance on every station page (`/tutorial/{workspace_id}/{station_id}`) that lets the user mark the current station as skipped without completing its `required_checks`. The affordance MUST go through a confirm modal before mutating `progress.json`, so accidental clicks do not silently change state.

The skip flow MUST:
1. Render a `<SkipStationButton>` (with text such as "↷ 跳過此站") in the station page header chrome
2. On click, call `useIntervention().requestSkip({ stationId, stationTitle })` which opens `<InterventionConfirmModal>` with copy explaining "跳過此站會解鎖下一站，但本站不會記為完成；隨時可重新進來學習"
3. On modal confirm, call `useTutorialProgress().markStationSkipped(stationId)` which appends the id to `progress.skipped_station_ids`, removes it from `current_station_id` if it was set, and triggers the standard debounced write
4. After the write commits, navigate to the next unlocked station (`route.json.stations[idx + 1].station_id`), or back to MOC if the skipped station was already the last in the route

The skip button MUST NOT render on stations whose id is already in `progress.completed_station_ids` (the user has finished it; skipping is meaningless) but MUST still render on stations in `progress.skipped_station_ids` as a no-op (to avoid layout flicker when the user re-enters a skipped station; clicking it shows a "本站已跳過" tooltip and is non-destructive).

#### Scenario: Skip button on never-visited station opens confirm modal

- **WHEN** the user is on `/tutorial/ws_xxx/s02-mqtt-client` (where `s02-mqtt-client` is unlocked but not in `completed_station_ids` or `skipped_station_ids`) and clicks the `<SkipStationButton>`
- **THEN** `<InterventionConfirmModal>` MUST render with confirm copy explaining the skip semantics
- **AND** `progress.json` MUST NOT yet be modified (state changes only on confirm)

#### Scenario: Confirm in skip modal writes progress and navigates forward

- **WHEN** the user confirms the skip modal for `s02-mqtt-client` (which is `route.json.stations[1]`)
- **THEN** `progress.skipped_station_ids` MUST contain `s02-mqtt-client` after the next debounced write completes
- **AND** the page MUST navigate to `/tutorial/ws_xxx/{route.json.stations[2].station_id}` automatically
- **AND** if `s02-mqtt-client` was the last station in `route.json.stations`, navigation MUST instead go to `/tutorial/ws_xxx` (MOC)

#### Scenario: Skip button on completed station does not render

- **WHEN** the user navigates to a station whose id is already in `progress.completed_station_ids`
- **THEN** `<SkipStationButton>` MUST NOT render in the station page header chrome
- **AND** no skip-related DOM nodes MUST be present

#### Scenario: Skip button on skipped station is non-destructive

- **WHEN** the user re-enters a station whose id is in `progress.skipped_station_ids` and clicks `<SkipStationButton>`
- **THEN** the click MUST NOT open the confirm modal nor mutate `progress.json`
- **AND** the button MUST display a "本站已跳過" tooltip (or equivalent inert affordance)
