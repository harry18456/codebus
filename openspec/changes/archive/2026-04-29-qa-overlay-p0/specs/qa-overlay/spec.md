## ADDED Requirements

### Requirement: `useQaSession` is a module-level singleton with one SSE dispatch entry

The frontend SHALL expose a composable `web/app/composables/useQaSession.ts` whose internal reactive state lives at module scope (NOT inside the `useQaSession()` function body). Every caller of `useQaSession()` MUST receive the SAME `Ref` instances for `open`, `turns`, `currentTaskId`, `status`, and `error`. The composable MUST NOT instantiate per-call state — calling `useQaSession()` from two components MUST yield identical references (verifiable via `Object.is`).

The composable's API MUST equal:

```ts
interface UseQaSessionApi {
  open: Ref<boolean>
  turns: Ref<QaTurn[]>
  currentTaskId: Ref<string | null>
  status: Ref<'idle' | 'pending' | 'streaming' | 'done' | 'error'>
  error: Ref<Error | null>
  start: (prompt: string, originatingStationId: string | null) => Promise<void>
  openDrawer: () => void
  close: () => void
}

interface QaTurn {
  id: string                                    // turn_<timestamp_ms>
  question: string
  originatingStationId: string | null
  taskId: string | null                         // qa_<8hex>; null until POST /qa resolves
  ragHits: RagHit[] | null                      // populated on rag_hits SSE
  reactSteps: { step: number; thought?: { text: string }; actions: ActionEntry[] }[]
  kbGrowth: KbGrowthEvent[]
  answer: { text: string; citations: Citation[] } | null
  status: 'pending' | 'streaming' | 'done' | 'error'
  error?: { code: string; message: string }
}
```

`start(prompt, originatingStationId)` MUST:

1. Append a new `QaTurn` to `turns.value` with `status: 'pending'`, fresh `id` (`turn_<Date.now()>`), and the supplied prompt + station id.
2. Issue `POST /qa` via `useSidecar().fetch('/qa', { ... })` with body `{ workspace_root, question, originating_station_id }`. The `workspace_root` MUST be obtained from a yet-to-be-established convention (P0 reads `?ws_path=` query param via the caller passing it through; if absent, `start()` MUST reject with `Error("missing ws_path")` and set the turn status to `'error'` without spawning).
3. On 202 Accepted, set the turn's `taskId` to the returned `task_id`, transition status to `'streaming'`, instantiate `useSseTask(task_id)`, and watch its events.
4. On 409 `TASK_IN_FLIGHT`, set the turn status to `'error'` with `{ code: 'TASK_IN_FLIGHT', message: ... }`. Do NOT retry.
5. On other non-2xx, set turn status to `'error'` with the response code/message.

The composable MUST NOT open more than one EventSource per concurrently in-flight turn; sequential turns are allowed but only after the previous turn's SSE has emitted `done` or `error`. Within a single turn, the inner `useSseTask` instance is the **only** SSE dispatch entry — components consuming `turns` MUST NOT instantiate their own EventSource for the same `task_id`.

SSE event dispatch rules (per active turn):

1. `rag_hits` event MUST set `turn.ragHits = event.hits`.
2. `agent_thought` event MUST upsert `turn.reactSteps[event.step].thought = { text: event.thought }`.
3. `agent_action_result` event MUST append into `turn.reactSteps[event.step].actions[]` an entry shaped like `agent-console-p0`'s `ActionEntry` (`{ tool, observation, tokens_used, isError }` with the same `error:` / `traceback` heuristic for `isError`).
4. `kb_growth` event MUST append into `turn.kbGrowth[]`. Dedup by `entry_id` — if an entry with the same id already exists, the event MUST NOT push a duplicate.
5. `qa_answer` event MUST set `turn.answer = { text: event.answer, citations: event.citations }`.
6. `done` event MUST flip `turn.status` to `'done'` exactly once; subsequent `done` events on the same turn MUST NOT cause re-flip or re-dispatch.
7. `error` event MUST flip `turn.status` to `'error'` and populate `turn.error`.

The `turns.value` array MUST be capped at 50 entries (FIFO eviction) to bound memory under exploratory long sessions.

#### Scenario: Two callers receive the same singleton state

- **WHEN** component A calls `useQaSession()` and component B (mounted separately) calls `useQaSession()`
- **THEN** `Object.is(a.turns, b.turns)` MUST be true
- **AND** `Object.is(a.open, b.open)` MUST be true

#### Scenario: start() appends a pending turn before POST /qa resolves

- **WHEN** `start("why atomic write?", "s03-production")` is invoked (POST /qa pending)
- **THEN** `turns.value` MUST gain exactly one new entry whose `question === "why atomic write?"`, `originatingStationId === "s03-production"`, `status === "pending"`, `taskId === null`

#### Scenario: 409 TASK_IN_FLIGHT marks the turn errored without spawning SSE

- **WHEN** `start(...)` is invoked while another sidecar task is running and the sidecar returns 409 with body `{"detail": {"code": "TASK_IN_FLIGHT"}}`
- **THEN** the appended turn's `status` MUST equal `'error'`
- **AND** `turn.error.code` MUST equal `"TASK_IN_FLIGHT"`
- **AND** no `EventSource` MUST be opened (verifiable via `getOpenedEventSources().length` unchanged from baseline)

#### Scenario: rag_hits event populates the active turn's ragHits

- **WHEN** an active turn has `taskId === "qa_abc12345"` and a `rag_hits` SSE event arrives with `{hits: [{score:0.71, ...}, {score:0.69, ...}]}`
- **THEN** `turn.ragHits.length` MUST equal 2
- **AND** the array MUST equal the event's `hits` payload

#### Scenario: kb_growth dedup by entry_id

- **WHEN** two `kb_growth` events arrive with the same `entry_id: "a14f9c2e"` for the same turn
- **THEN** `turn.kbGrowth.length` MUST equal 1 after both events dispatch

#### Scenario: turns FIFO cap at 50

- **WHEN** 60 sequential `start(prompt_i, null)` calls have all completed
- **THEN** `turns.value.length` MUST equal 50
- **AND** the first 10 turns (oldest) MUST have been evicted (`turns.value[0].id` MUST equal `turn_<timestamp_of_11th_call>`)

#### Scenario: done event flips status exactly once

- **WHEN** an active turn is in `status: 'streaming'` and a `done` SSE event arrives
- **THEN** `turn.status` MUST become `'done'`
- **AND** subsequent `done` events MUST NOT re-flip status or re-dispatch state

---

### Requirement: `<QAOverlay>` drawer renders Q&A turns and listens for keyboard shortcuts

The frontend SHALL ship `web/app/components/qa/QAOverlay.vue` as a drawer overlay component that subscribes to `useQaSession()` and renders the drawer when `open.value === true`.

When `open.value === false`, the component MUST render zero `<aside>` elements (i.e., `v-if` at root, NOT `v-show`).

When `open.value === true`, the component MUST render an `<aside>` overlay positioned `right-0 top-0 bottom-0` with width exactly `480px` (Tailwind `w-[480px]` or equivalent). Width MUST NOT be user-resizable in P0.

The overlay MUST also render a half-transparent dim layer behind the aside (`bg-surface-0/60` or similar) covering the rest of the viewport; clicking the dim layer MUST call `useQaSession().close()`. Clicking inside the aside MUST NOT close it.

The aside MUST contain three regions in order:

1. **Header**: title `Q&A · Module 8`; session badge showing the current `currentTaskId.value` (or em-dash when null); origin chip showing `📍 {originatingStationId}` when the most recent turn carries one (hidden when null); close button emitting `useQaSession().close()`.
2. **Body**: scrollable region rendering one `<QaTurnCard>` per turn in `turns.value` (insertion order — newest at bottom). When `turns.value.length === 0`, render a placeholder copy "Cmd+K 開始問問題" or equivalent.
3. **Composer**: input field for the next question; send button disabled when `turns.value[turns.length-1]?.status` is `'pending'` or `'streaming'` (or when the input is empty / whitespace-only); a meta strip showing `Pass 3 sanitize on` (literal copy), `session add {addToKbCount} / 20`, `budget {currentSteps} / 10 步 · ${cost}`. Pressing Enter in the input MUST trigger send when send is enabled.

Keyboard shortcuts:

- `Cmd+K` (Mac) or `Ctrl+K` (Windows / Linux) MUST open the drawer (call `useQaSession().openDrawer()`); when already open, the shortcut MUST be a no-op (no toggle, no close).
- `Escape` MUST close the drawer when `open.value === true`.

These listeners MUST live on `window` in the layout host (registered by the parent layout that mounts `<QAOverlay>`), NOT inside the component itself, so the shortcuts work even when the drawer is closed (Cmd+K must be able to OPEN it).

#### Scenario: Closed drawer renders nothing

- **WHEN** `useQaSession().open.value === false` and `<QAOverlay>` is mounted
- **THEN** the rendered DOM MUST contain zero `<aside>` elements

#### Scenario: Cmd+K opens the drawer when closed

- **WHEN** the drawer is closed and a `keydown` with `metaKey: true, key: "k"` fires on `window`
- **THEN** `open.value` MUST become `true`
- **AND** the next render MUST contain an `<aside>` element

#### Scenario: Cmd+K is no-op when already open

- **WHEN** the drawer is already open and a `keydown` with `metaKey: true, key: "k"` fires
- **THEN** `open.value` MUST remain `true`
- **AND** no toggle / close behavior MUST occur

#### Scenario: Escape closes the open drawer

- **WHEN** the drawer is open and `Escape` is pressed
- **THEN** `open.value` MUST become `false`

#### Scenario: Click on dim layer closes the drawer

- **WHEN** the drawer is open and the user clicks on the half-transparent dim layer outside the aside
- **THEN** `open.value` MUST become `false`

#### Scenario: Click inside the aside does not close

- **WHEN** the drawer is open and the user clicks inside the aside region
- **THEN** `open.value` MUST remain `true`

#### Scenario: Send button disabled while previous turn is streaming

- **WHEN** the most recent turn's status is `'streaming'`
- **THEN** the send button MUST have a `disabled` attribute

#### Scenario: Empty turns shows the placeholder copy

- **WHEN** `turns.value.length === 0` and the drawer is open
- **THEN** the body region MUST contain text matching the literal substring `Cmd+K`

---

### Requirement: `<QaTurnCard>` renders four phases per turn

The frontend SHALL ship `web/app/components/qa/QaTurnCard.vue` accepting `defineProps<{ turn: QaTurn }>()` and rendering four phases in fixed visual order:

1. **User message** — always rendered; contains `turn.question`.
2. **RAG hits** — rendered when `turn.ragHits !== null`; contains a header line "① RAG 探查" and a list of hit cards. Each hit card MUST display `{file_path}:{line_start}-{line_end}`, the snippet (truncated to 120 chars), score (formatted to 2 decimals), and a station chip per `related_stations` entry. When `turn.ragHits === null`, this phase section MUST NOT render.
3. **ReAct steps** — rendered when `turn.reactSteps.length > 0`; contains a header line "② ReAct loop" and a list of step rows. Each step shows `step {n} · {tool}` and either the thought text or the action observation (or both, in arrival order). When `turn.reactSteps.length === 0`, this section MUST NOT render.
4. **Answer** — rendered when `turn.answer !== null`; contains the answer prose (with paragraph breaks) and a `<QaCitations>` citation row. When `turn.answer === null`, this section MUST NOT render.

The card MUST also surface `turn.status` via a small badge: `pending` → grey "等候中…"; `streaming` → accent "進行中…" with pulse animation; `done` → green dot (no text); `error` → red "錯誤" with `turn.error.message` shown below.

A `<QaKbGrowthBlock>` (rendered inside the ReAct steps section, only when `turn.kbGrowth.length > 0`) MUST display each `kb_growth` event with `entry_id`, `source`, `related_stations` chips, and a "為什麼值得沉澱" reason field if present; rollback button MUST NOT render in P0.

#### Scenario: All four phases render when turn is complete

- **WHEN** a turn has `ragHits` non-null with 2 hits, `reactSteps` with 1 step, `answer` non-null with 2 citations
- **THEN** the rendered DOM MUST contain user message, RAG hits header, ReAct steps header, and answer regions in that order

#### Scenario: Empty ragHits hides the RAG section

- **WHEN** a turn has `ragHits === null`
- **THEN** no RAG hits header MUST render
- **AND** the ReAct steps and answer sections (if present) MUST still render

#### Scenario: Streaming status shows pulse badge

- **WHEN** `turn.status === 'streaming'`
- **THEN** the rendered DOM MUST contain an element with both `data-status="streaming"` (or equivalent) and a Tailwind class triggering pulse animation

#### Scenario: Error status surfaces error message

- **WHEN** `turn.status === 'error'` and `turn.error.message === "QA_FAILED: budget exhausted"`
- **THEN** the rendered DOM MUST contain the literal substring `"budget exhausted"`

#### Scenario: kb_growth block omits rollback button in P0

- **WHEN** a turn has `kbGrowth.length > 0`
- **THEN** the rendered DOM MUST NOT contain any element with text matching `/rollback|↶/i`
- **AND** the kb growth event metadata (`entry_id`, `source`, `related_stations`) MUST still render

---

### Requirement: `<QaCitations>` renders citation list with station emit

The frontend SHALL ship `web/app/components/qa/QaCitations.vue` accepting `defineProps<{ citations: Citation[] }>()` and emitting `(e: 'navigate-to-station', stationId: string) => void` on station chip clicks.

For each citation, the component MUST render:

- A file:line line `{file_path}:{line_start}-{line_end}` (P0: NOT clickable — file open in side panel is Phase 2).
- A station chip per entry in `citations[i].related_stations` (`📍 {station_id}`); clicking a station chip MUST emit `navigate-to-station` with that station id.

The component MUST NOT render anything when `citations.length === 0`.

#### Scenario: Empty citations renders nothing

- **WHEN** `citations.length === 0`
- **THEN** the rendered DOM MUST contain zero citation rows / station chips

#### Scenario: Station chip click emits navigate-to-station

- **WHEN** the user clicks a station chip with `data-station-id="s03-production"`
- **THEN** the component MUST emit `navigate-to-station` with payload `"s03-production"`

#### Scenario: file:line is not clickable in P0

- **WHEN** the rendered DOM is inspected for a citation entry
- **THEN** the file:line element MUST NOT be wrapped in an `<a>` or `<button>` element
- **AND** clicking the file:line MUST NOT emit any event

---

### Requirement: `<QAEntry>` mdc element invokes `useQaSession` imperatively

The existing `web/app/components/content/QAEntry.vue` mdc-auto-imported component SHALL replace its placeholder `router.push('/qa?prompt=...')` implementation with an imperative call to `useQaSession().start(props.prompt, inject<string | null>('currentStationId', null))`.

The component MUST preserve:

- Its prop shape: `defineProps<{ prompt: string }>()`.
- Its template structure: a `<button>` with the same Tailwind classes as the existing implementation (so R-01 station markdown rendering does not visually shift).
- Its mdc-auto-import contract: the file MUST remain at `web/app/components/content/QAEntry.vue` so `@nuxtjs/mdc` continues to surface it as `<QAEntry>` in markdown.

The component MUST NOT directly call any sidecar endpoint (it goes through `useQaSession`); MUST NOT subscribe to any SSE; MUST NOT render any modal / drawer / overlay of its own. The `frontend-shell` invariant "QAEntry MUST NOT itself fetch any sidecar endpoint; it is a navigation trigger only" remains satisfied — the imperative call is a navigation trigger into the `useQaSession` composable, which is the actual fetch boundary.

#### Scenario: Click invokes useQaSession.start with prompt and injected station id

- **WHEN** `<QAEntry prompt="why atomic write?" />` is mounted with `provide('currentStationId', 's03-production')` in an ancestor and the user clicks the button
- **THEN** `useQaSession().start` MUST be called exactly once with arguments `("why atomic write?", "s03-production")`

#### Scenario: Missing currentStationId provide falls back to null

- **WHEN** `<QAEntry prompt="hi" />` is mounted without any `provide('currentStationId', ...)` ancestor
- **THEN** `useQaSession().start` MUST be called with `("hi", null)`
- **AND** no error MUST be raised

#### Scenario: QAEntry does not call router.push

- **WHEN** the user clicks the button
- **THEN** `useRouter().push` MUST NOT be invoked
- **AND** the URL MUST NOT change

---

### Requirement: `useAuditJsonl` supports kb_growth live-tail from useQaSession

The composable `useAuditJsonl(workspaceRoot, kind, opts?)` introduced by `llm-call-inspector-p0` SHALL accept an additional optional field on `opts`: `liveTailFromQaSession?: UseQaSessionApi`. When provided AND `kind === "kb_growth"`, the composable MUST watch the supplied session's SSE event chain and append every `kb_growth` event payload into `entries`, with the same `entry_id`-based dedup rule as the llm `request_id` dedup.

For kinds other than `"kb_growth"`, the `liveTailFromQaSession` option MUST be ignored at runtime (no error, no-op). The existing `liveTailFromExplorerStream` option for `kind === "llm"` MUST continue to work unchanged.

#### Scenario: kb_growth live-tail appends QA SSE events into entries

- **WHEN** `useAuditJsonl('/abs/ws', 'kb_growth', { liveTailFromQaSession: session })` is constructed and `session` later receives two `kb_growth` SSE events with distinct `entry_id` values
- **THEN** `entries.value.length` MUST equal `<initial disk count> + 2`
- **AND** the two new entries MUST be appended at the end (timestamp-ascending preserved)

#### Scenario: Dedup by entry_id prevents disk + SSE double-push

- **WHEN** the disk load contains an entry with `entry_id: "a14f9c2e"` AND a subsequent `kb_growth` SSE event arrives with the same `entry_id`
- **THEN** `entries.value.length` MUST NOT increase

#### Scenario: liveTailFromQaSession ignored when kind is not kb_growth

- **WHEN** `useAuditJsonl('/abs/ws', 'tool', { liveTailFromQaSession: session })` is constructed and `session` receives `kb_growth` events
- **THEN** `entries.value` MUST NOT receive any new entries from those events
- **AND** no error MUST be raised
