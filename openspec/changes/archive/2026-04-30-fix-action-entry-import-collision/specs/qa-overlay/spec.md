## MODIFIED Requirements

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

The `ActionEntry` referenced above MUST be imported from `web/app/types/agent-action.ts` — the single canonical type module shared with `web/app/composables/useExplorerStream.ts`. The composable file MUST NOT declare its own `export interface ActionEntry`; both `useQaSession.ts` and `useExplorerStream.ts` MUST consume the same exported `ActionEntry` symbol from `web/app/types/agent-action.ts` so Nuxt auto-import sees one definition (the duplicate-export warning during `cargo tauri dev` MUST disappear).

`start(prompt, originatingStationId)` MUST:

1. Append a new `QaTurn` to `turns.value` with `status: 'pending'`, fresh `id` (`turn_<Date.now()>`), and the supplied prompt + station id.
2. Issue `POST /qa` via `useSidecar().fetch('/qa', { ... })` with body `{ workspace_root, question, originating_station_id }`. The `workspace_root` MUST be obtained from a yet-to-be-established convention (P0 reads `?ws_path=` query param via the caller passing it through; if absent, `start()` MUST reject with `Error("missing ws_path")` and set the turn status to `'error'` without spawning).
3. On 202 Accepted, set the turn's `taskId` to the returned `task_id`, transition status to `'streaming'`, instantiate `useSseTask(task_id)`, and watch its events.
4. On 409 `TASK_IN_FLIGHT`, set the turn status to `'error'` with `{ code: 'TASK_IN_FLIGHT', message: ... }`. The composable MUST NOT retry.
5. On other non-2xx, set turn status to `'error'` with the response code/message.

The composable MUST NOT open more than one EventSource per concurrently in-flight turn; sequential turns are allowed but only after the previous turn's SSE has emitted `done` or `error`. Within a single turn, the inner `useSseTask` instance is the **only** SSE dispatch entry — components consuming `turns` MUST NOT instantiate their own EventSource for the same `task_id`.

SSE event dispatch rules (per active turn):

1. `rag_hits` event MUST set `turn.ragHits = event.hits`.
2. `agent_thought` event MUST upsert `turn.reactSteps[event.step].thought = { text: event.thought }`.
3. `agent_action_result` event MUST append into `turn.reactSteps[event.step].actions[]` an `ActionEntry` value imported from `web/app/types/agent-action.ts` (`{ tool, observation, tokens_used, isError }` with `isError = observation.startsWith('error:') || observation.toLowerCase().includes('traceback')` — same heuristic the `useExplorerStream` dispatcher applies, both consuming the same canonical type).
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

#### Scenario: ActionEntry is imported from canonical type module — no duplicate export warning

- **WHEN** the frontend dev server (`cargo tauri dev` or `npm run dev`) starts and Nuxt auto-import scan completes
- **THEN** the auto-importer MUST NOT log `Duplicated imports "ActionEntry"` warning
- **AND** `web/app/composables/useQaSession.ts` MUST NOT contain `export interface ActionEntry`
- **AND** `web/app/composables/useQaSession.ts` MUST contain `import type { ActionEntry } from '~/types/agent-action'` (or an equivalent path resolving to the same module)
- **AND** `web/app/types/agent-action.ts` MUST be the only source file declaring `export interface ActionEntry`
