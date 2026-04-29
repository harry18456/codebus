# agent-console Specification

## Purpose

TBD - created by archiving change 'agent-console-p0'. Update Purpose after archive.

## Requirements

### Requirement: Explorer console page mounts on `/explorer/{task_id}` route

The frontend SHALL expose a Nuxt page route at `web/app/pages/explorer/[task_id].vue` that renders the live Module 4 Explorer console. The route parameter `task_id` MUST match the regex `^explore_[0-9a-f]{8}$` declared by `sidecar-api.md` §三-bis. Passing a `task_id` that fails the regex MUST surface a route-level validation error and MUST NOT open an SSE connection.

The page MUST construct exactly one `useExplorerStream(task_id)` composable instance on `onMounted` and MUST close its SSE connection on `onBeforeUnmount`. Switching between two `/explorer/{task_id}` routes with different `task_id` values MUST close the previous connection before opening the next.

The page layout MUST place the Step Card timeline on the main stage and the existing `<AuditPanel :active-tab="'reasoning'" />` on the right rail; both elements MUST consume reactive state from the same `useExplorerStream` instance (no second EventSource).

#### Scenario: Valid task_id mounts and opens SSE

- **WHEN** the user navigates to `/explorer/explore_4f2a8b91`
- **THEN** the page MUST mount and call `useExplorerStream('explore_4f2a8b91')`
- **AND** exactly one EventSource connection MUST be opened against `GET /tasks/explore_4f2a8b91/events`

#### Scenario: Invalid task_id shape rejected before SSE opens

- **WHEN** the user navigates to `/explorer/not-a-task-id`
- **THEN** no EventSource MUST be opened
- **AND** the page MUST display a route-level validation error to the user

#### Scenario: Route change closes prior SSE before opening new one

- **WHEN** the user is on `/explorer/explore_aaaaaaaa` and navigates to `/explorer/explore_bbbbbbbb`
- **THEN** the EventSource bound to `explore_aaaaaaaa` MUST be closed before the new EventSource for `explore_bbbbbbbb` is opened
- **AND** the prior `useExplorerStream` instance's reactive state MUST be discarded

---
### Requirement: useExplorerStream is the single SSE dispatch entry for the console

The composable `web/app/composables/useExplorerStream.ts` SHALL wrap exactly one `useSseTask(task_id)` instance and dispatch its events to four reactive surfaces consumed by the console UI: `stepBuckets`, `progress`, `coverageBanner`, and `budgetBanner`. The composable MUST also expose `auditRows: Ref<AuditRow[]>` so the existing `AuditPanel` reasoning tab can consume reasoning events from the same dispatch entry without opening a second EventSource.

The composable MUST forward `useSseTask`'s `status` and `error` refs unchanged. It MUST expose a `done: Ref<boolean>` flag that flips to `true` exactly once when the SSE stream emits a `done` event.

Domain-specific dispatch rules:

1. `agent_thought` events MUST upsert into `stepBuckets[event.step].thought = { text: event.thought, actions: event.action }`.
2. `agent_action_result` events MUST append to `stepBuckets[event.step].actions[]` an entry `{ tool, observation, tokens_used, isError }` where `isError = observation.startsWith('error:') || observation.toLowerCase().includes('traceback')`.
3. `judge_verdict` events MUST upsert into `stepBuckets[event.step].judge = { relevance, reason }`.
4. `progress` events with `phase === 'exploring'` MUST overwrite `progress.value = { current, total }`.
5. `coverage_gaps` events MUST overwrite `coverageBanner.value = event` (latest-only).
6. `budget_warning` events MUST set `budgetBanner.value[event.kind] = event` (per-kind latched; existing entries for other kinds preserved).
7. `agent_thought` / `agent_action_result` / `judge_verdict` events MUST also push a stringified row into `auditRows` (rolling window capped at 200 entries).

The composable MUST NOT instantiate a second `EventSource`; all subscribers (timeline, progress, banners, audit panel) share the one connection opened by the inner `useSseTask`.

#### Scenario: Single EventSource serves timeline + audit panel

- **WHEN** a page mounts `useExplorerStream('explore_4f2a8b91')` and renders both `<ConsoleTimeline />` and `<AuditPanel active-tab="reasoning" />` from the same instance
- **THEN** exactly one EventSource MUST be opened against the sidecar
- **AND** both components MUST observe the same reactive state derived from that connection

#### Scenario: agent_thought upserts thought, preserving prior actions

- **WHEN** events arrive in order `agent_action_result(step=3, tool=read_file)`, `agent_thought(step=3)`, `agent_action_result(step=3, tool=trace_import)`
- **THEN** `stepBuckets.value.get(3).thought` MUST be set from the `agent_thought` payload
- **AND** `stepBuckets.value.get(3).actions` MUST be a length-2 array containing both action results in arrival order

#### Scenario: agent_action_result observation is flagged as error via heuristic

- **WHEN** an `agent_action_result` event arrives with `observation: "error: file not found"`
- **THEN** the corresponding entry pushed into `stepBuckets[step].actions` MUST have `isError === true`

#### Scenario: progress overwrite is monotonic-friendly

- **WHEN** two `progress` events with `phase: "exploring"` arrive in sequence (`current=2, total=5` then `current=3, total=5`)
- **THEN** `progress.value` MUST equal `{ current: 3, total: 5 }`
- **AND** the prior snapshot MUST be discarded (no history retained)

#### Scenario: coverage_gaps shows latest only

- **WHEN** two `coverage_gaps` events arrive (`round=0` with two gaps, then `round=1` with one gap)
- **THEN** `coverageBanner.value` MUST equal the second event payload
- **AND** no array of past banners MUST be retained

#### Scenario: budget_warning latches per kind

- **WHEN** a `budget_warning` event with `kind: "tokens"` arrives, then a second event with `kind: "steps"` arrives
- **THEN** `budgetBanner.value.tokens` MUST equal the first event payload
- **AND** `budgetBanner.value.steps` MUST equal the second event payload
- **AND** neither MUST overwrite the other

#### Scenario: auditRows rolling window caps at 200 entries

- **WHEN** 250 reasoning-class events (`agent_thought` / `agent_action_result` / `judge_verdict`) arrive in sequence
- **THEN** `auditRows.value.length` MUST equal 200
- **AND** the oldest 50 entries MUST have been evicted (FIFO)

#### Scenario: done event flips done flag exactly once

- **WHEN** the SSE stream emits a single `done` event
- **THEN** `done.value` MUST become `true`
- **AND** subsequent `done` events MUST NOT cause `done.value` to flip back or repeat side effects

---
### Requirement: StepCard renders ReAct three beats in arrival order

The `web/app/components/console/StepCard.vue` component SHALL accept a single `bucket: StepBucket` prop and render up to three sections in fixed visual order: THINK, ACT, JUDGE. Each section MUST be hidden when its corresponding bucket field is absent (e.g., `judge` not yet arrived).

The ACT section MUST render every entry in `bucket.actions[]`, including entries where `isError === true`. Failed entries MUST receive a visually distinct treatment (red accent border or red badge) but MUST NOT be hidden, fulfilling the `agent-explorer-spec.md §七` requirement that "失敗的嘗試也要顯示".

When `entry.tokens_used > 0` the card MUST display the formatted token count; when `entry.tokens_used === 0` the card MUST display the literal string `—` (em dash) instead of `0 tokens` to avoid misleading the user about per-tool attribution status (P0 placeholder per `explorer-sse` Requirement 2).

`observation` text exceeding 500 characters MUST already arrive truncated (sidecar contract); the StepCard MUST render a "…" suffix indicator when the observation reaches the 500-character ceiling.

#### Scenario: All three beats render when bucket is complete

- **WHEN** `<StepCard :bucket="{ step: 3, thought: {...}, actions: [{...}], judge: {...} }" />` mounts
- **THEN** the rendered DOM MUST contain a THINK section, an ACT section with one entry, and a JUDGE section, in that order

#### Scenario: Missing judge hides the JUDGE section

- **WHEN** `<StepCard :bucket="{ step: 4, thought: {...}, actions: [{...}] }" />` mounts (no `judge` field)
- **THEN** no JUDGE section MUST be rendered
- **AND** the THINK and ACT sections MUST still render

#### Scenario: Failed action renders with error styling but stays visible

- **WHEN** the bucket contains an action entry with `isError: true` and `observation: "error: file not found"`
- **THEN** that entry MUST be visible in the ACT section
- **AND** it MUST carry a `data-state="error"` attribute (or equivalent error class) on its row element

#### Scenario: tokens_used 0 renders em dash placeholder

- **WHEN** an action entry has `tokens_used: 0`
- **THEN** the rendered token cell MUST contain the literal string `—`
- **AND** MUST NOT contain the literal string `0 tokens` or `$0`

---
### Requirement: ProgressStrip mirrors progress events without computing stations

The `web/app/components/console/ProgressStrip.vue` component SHALL render the current Explorer step progress derived from the latest `progress` event with `phase: "exploring"`. The component MUST display:

1. A textual `step <current> / <total>` indicator (e.g., `step 4 / 5`).
2. A horizontal step grid where exactly `total` cells render; cells with index < `current - 1` MUST display a "done" treatment, the cell at index `current - 1` MUST display an "in progress" treatment, and remaining cells MUST display a "queued" treatment.

The component MUST NOT compute or display a "stations" counter. The mockup's `3 stations` text is out of scope (see proposal Non-Goals); the P0 strip displays only the `progress.current/total` numerics.

When no `progress` event has arrived yet, the strip MUST render a placeholder (empty grid + `step — / —`) rather than crashing on null state.

#### Scenario: ProgressStrip renders 5 cells when total is 5

- **WHEN** `progress.value = { current: 4, total: 5 }`
- **THEN** the rendered grid MUST contain exactly 5 cells
- **AND** cells 1-3 MUST carry the "done" treatment
- **AND** cell 4 MUST carry the "in progress" treatment
- **AND** cell 5 MUST carry the "queued" treatment
- **AND** the textual indicator MUST read `step 4 / 5`

#### Scenario: ProgressStrip placeholder when no progress event yet

- **WHEN** `progress.value === null`
- **THEN** the textual indicator MUST read `step — / —`
- **AND** the rendered grid MUST contain zero cells (or a single placeholder cell), and MUST NOT throw

#### Scenario: ProgressStrip ignores non-exploring phase

- **WHEN** `progress.value = { current: 10, total: 50, phase: "scanning" }` somehow leaks into the composable
- **THEN** the strip MUST treat the value as absent (placeholder)
- **AND** MUST NOT render the scanning numeric

---
### Requirement: CoverageBanner renders coverage_gaps and budget_warning events

The `web/app/components/console/CoverageBanner.vue` component SHALL render zero, one, or two banners total based on the current `coverageBanner` and `budgetBanner` reactive state from `useExplorerStream`. The component MUST NOT stack arbitrary numbers of banners; the visible set is bounded.

Display rules:

1. When `coverageBanner.value !== null`, render exactly one coverage banner whose copy summarises `gaps.length` and `skip_reason` (if present).
2. When `budgetBanner.value.steps` is set, render exactly one budget banner for the steps kind.
3. When `budgetBanner.value.tokens` is set AND `budgetBanner.value.steps` is unset, render exactly one budget banner for the tokens kind. (Steps takes priority when both kinds latched.)
4. When neither coverageBanner nor budgetBanner has any latched value, render nothing (component returns empty fragment).

The coverage banner MUST visually differentiate between the four `skip_reason` cases (`"no_gaps"`, `"budget_exhausted"`, `"max_depth_reached"`, `null` for `will_recurse=true`) — at minimum via a distinct text label per case.

#### Scenario: Both kinds latched displays only the steps banner

- **WHEN** `budgetBanner.value = { steps: {...}, tokens: {...} }` and `coverageBanner.value === null`
- **THEN** exactly one banner MUST be rendered
- **AND** that banner MUST carry the `data-kind="steps"` attribute (or equivalent)

#### Scenario: Coverage banner with no gaps differs from coverage banner with budget exhausted

- **WHEN** the rendered DOM is inspected for `coverageBanner.value.skip_reason === "no_gaps"` versus `"budget_exhausted"`
- **THEN** the two cases MUST render visibly distinct text labels (not the same string)

#### Scenario: All-null state renders nothing

- **WHEN** `coverageBanner.value === null` and `budgetBanner.value === { steps: undefined, tokens: undefined }`
- **THEN** the component MUST render zero banner DOM nodes (empty fragment or v-if false)

---
### Requirement: ConsoleTimeline iterates stepBuckets in step ascending order

The `web/app/components/console/ConsoleTimeline.vue` component SHALL accept the `stepBuckets: Map<number, StepBucket>` reactive ref from `useExplorerStream` and render one `<StepCard>` per bucket entry, sorted by step ascending. The component MUST use `:key="bucket.step"` so Vue can correctly re-use card DOM nodes when buckets are upserted by late-arriving events.

When `stepBuckets.size === 0`, the timeline MUST render an empty placeholder rather than nothing — minimum copy: a small text reading "等候 Explorer 開始決策…" or equivalent — so the user knows the SSE channel is alive.

The timeline MUST NOT depend on flat event order or compute groupings client-side; bucket-fill in the composable is authoritative.

#### Scenario: Timeline renders cards in step ascending order regardless of arrival order

- **WHEN** events arrive such that `stepBuckets` ends up populated with steps `{2, 1, 3}` in insertion order
- **THEN** the rendered DOM MUST place the StepCard for step 1 above step 2, and step 2 above step 3

#### Scenario: Empty timeline shows waiting placeholder

- **WHEN** `stepBuckets.value.size === 0`
- **THEN** the timeline MUST render placeholder copy informing the user the stream is waiting for first events
- **AND** MUST NOT render any `<StepCard>` element

#### Scenario: Late-arriving event upserts existing card via stable key

- **WHEN** the timeline already shows step 2 (THINK only) and a late `judge_verdict(step=2)` event arrives
- **THEN** the same StepCard DOM node MUST update in place to show the JUDGE section
- **AND** Vue MUST NOT unmount and remount the card (verified via `:key` stability)

---
### Requirement: AuditPanel reasoning tab consumes useExplorerStream auditRows

The Explorer page SHALL pass `useExplorerStream(task_id).auditRows` to the existing `<AuditPanel :rows="..." />` prop when the active tab is `"reasoning"`. The integration MUST NOT modify `frontend-shell` capability's existing `AuditPanel` Requirements (seven-tab order, design tokens, prop shape); only the data binding at the page level is added.

When the user switches `AuditPanel` to a non-`"reasoning"` tab (e.g., `"sanitize"`, `"tool"`), the page-level binding MUST pass the appropriate row source for that tab, OR pass an empty array to indicate "no live binding for this tab in P0". P0 only wires the reasoning tab to live data; other tabs render the empty-state placeholder declared by `frontend-shell`.

#### Scenario: Reasoning tab receives live rows from useExplorerStream

- **WHEN** the page renders `<AuditPanel :active-tab="'reasoning'" :rows="reasoningRows" />` and three reasoning events flow through `useExplorerStream`
- **THEN** the AuditPanel reasoning tab MUST render exactly three row entries (subject to the rolling-window cap)
- **AND** each row's `body` MUST be a stringified summary of the corresponding SSE event

#### Scenario: Non-reasoning tabs receive empty rows in P0

- **WHEN** the user clicks the "tool" tab while the reasoning tab is showing live rows
- **THEN** the AuditPanel MUST render the empty-state placeholder for the tool tab
- **AND** `<AuditPanel :rows="..." />` MUST receive an empty array for tabs other than reasoning in P0

---
### Requirement: Vitest fixture covers a complete event sequence

The repository SHALL ship a vitest fixture at `web/tests/console/fixtures/explorer-stream.json` that contains a JSON array of SSE event envelopes (`{ type, data }`) covering at least: 3 step iterations of `agent_thought` + `agent_action_result` + `judge_verdict`, at least one `coverage_gaps` event, at least one `budget_warning` event, at least 3 `progress` events, and exactly one terminal `done` event.

The fixture MUST be derivable from the existing `tests/golden/demo-synthetic/reasoning_log.jsonl` at the sidecar layer (with non-reasoning-log-derivable events appended as deterministic placeholders). The fixture's purpose is to feed `useExplorerStream` and component tests without requiring a live sidecar; it MUST NOT be imported into production bundles.

The fixture MUST be valid JSON parseable by `vitest`'s default importer (`import fixture from './fixtures/explorer-stream.json'`).

#### Scenario: Fixture parses as JSON array of envelopes

- **WHEN** the fixture file is imported in a vitest test
- **THEN** the import MUST yield an array
- **AND** every element MUST have a string `type` field and a `data` field

#### Scenario: Fixture covers required event diversity

- **WHEN** the fixture array is iterated
- **THEN** at least one element MUST have `type === "coverage_gaps"`
- **AND** at least one element MUST have `type === "budget_warning"`
- **AND** at least three elements MUST have `type === "progress"`
- **AND** exactly one element MUST have `type === "done"`
- **AND** the `done` element MUST be the last element in the array

#### Scenario: Fixture is not bundled into production

- **WHEN** `npm run generate` produces the SPA bundle for Tauri
- **THEN** the bundle output MUST NOT contain the string contents of `web/tests/console/fixtures/explorer-stream.json`
- **AND** the fixture path MUST live under `web/tests/` so Nuxt's default routing/bundling rules exclude it
