import { ref, watch, type Ref } from 'vue'
import { useSidecar } from './useSidecar'
import { useSseTask, type SseTaskApi, type SseEvent } from './useSseTask'
import type { ActionEntry } from '~/types/agent-action'

// useQaSession — module-level singleton driving the Q&A drawer overlay.
//
// Spec: openspec/changes/qa-overlay-p0/specs/qa-overlay/spec.md
//   "useQaSession is a module-level singleton with one SSE dispatch entry"
//
// Per design Decision 1: state lives at module scope; every call to
// `useQaSession()` returns references to the SAME ref instances. Layout-level
// `<QAOverlay>` and inline `<QAEntry>` mdc trigger therefore share one truth
// table — drawer can't drift from session state.

const TURNS_CAP = 50

export interface RagHit {
  score: number
  file_path: string
  line_start: number
  line_end: number
  snippet: string
  related_stations: string[]
}

export interface ReactStep {
  step: number
  thought?: { text: string }
  actions: ActionEntry[]
}

export interface KbGrowthEvent {
  entry_id: string
  source: string
  related_stations: string[]
  originating_station_id: string | null
  reason?: string
}

export interface Citation {
  file_path: string
  line_start: number
  line_end: number
  related_stations: string[]
}

export interface QaTurn {
  id: string
  question: string
  originatingStationId: string | null
  taskId: string | null
  ragHits: RagHit[] | null
  reactSteps: ReactStep[]
  kbGrowth: KbGrowthEvent[]
  answer: { text: string; citations: Citation[] } | null
  status: 'pending' | 'streaming' | 'done' | 'error'
  error?: { code: string; message: string }
}

export interface UseQaSessionApi {
  open: Ref<boolean>
  turns: Ref<QaTurn[]>
  currentTaskId: Ref<string | null>
  status: Ref<'idle' | 'pending' | 'streaming' | 'done' | 'error'>
  error: Ref<Error | null>
  start: (prompt: string, originatingStationId: string | null) => Promise<void>
  openDrawer: () => void
  close: () => void
}

// Module-scope singleton state. `useQaSession()` returns reference to these
// same refs so cross-component callers stay in lockstep.
const _state = {
  open: ref<boolean>(false),
  turns: ref<QaTurn[]>([]),
  currentTaskId: ref<string | null>(null),
  status: ref<'idle' | 'pending' | 'streaming' | 'done' | 'error'>('idle'),
  error: ref<Error | null>(null)
}

// Aggregated SSE event stream — accumulates ALL events received by this
// session across turns. `useAuditJsonl(workspaceRoot, 'kb_growth', { liveTailFromQaSession })`
// subscribes to this via the conventional `__sseEvents` slot. Persisting
// across turns lets the kb_growth tab observe a continuous append stream
// even as the underlying per-turn `useSseTask` instance is replaced on
// each new question.
const _aggregatedSseEvents: Ref<SseEvent[]> = ref([])

let _sse: SseTaskApi | null = null
let _sseStopWatch: (() => void) | null = null
// Track which event indices have already been dispatched per turn so a
// reactive ref reset (sseEventsRef.value = [...]) doesn't replay from 0.
let _sseCursor = 0
// Track which turn id we're currently dispatching events into. The cursor
// resets whenever the bound turn changes; assigning a new turn happens only
// from start(), so the cursor moves forward monotonically per turn.
let _currentTurnId: string | null = null
// Track whether `done` has already flipped status for the bound turn so a
// repeated `done` event doesn't re-dispatch.
let _doneFlippedForTurn: string | null = null

function disposeSse(): void {
  if (_sseStopWatch !== null) {
    _sseStopWatch()
    _sseStopWatch = null
  }
  if (_sse !== null) {
    _sse.close()
    _sse = null
  }
  _sseCursor = 0
  _currentTurnId = null
  _doneFlippedForTurn = null
}

function freshTurnId(): string {
  return `turn_${Date.now()}_${Math.floor(Math.random() * 1e6)}`
}

function findTurn(id: string): QaTurn | undefined {
  return _state.turns.value.find((t) => t.id === id)
}

function applyEvent(turnId: string, event: { type: string; data: unknown }): void {
  const turn = findTurn(turnId)
  if (turn === undefined) return

  if (event.type === 'rag_hits') {
    const data = event.data as { hits?: RagHit[] }
    turn.ragHits = Array.isArray(data?.hits) ? data.hits : []
    return
  }

  if (event.type === 'agent_thought') {
    const data = event.data as { step?: number; thought?: string }
    if (typeof data.step !== 'number') return
    let step = turn.reactSteps.find((s) => s.step === data.step)
    if (!step) {
      step = { step: data.step, actions: [] }
      turn.reactSteps.push(step)
    }
    if (typeof data.thought === 'string') {
      step.thought = { text: data.thought }
    }
    return
  }

  if (event.type === 'agent_action_result') {
    const data = event.data as {
      step?: number
      tool?: string
      observation?: string
      tokens_used?: number
    }
    if (typeof data.step !== 'number') return
    let step = turn.reactSteps.find((s) => s.step === data.step)
    if (!step) {
      step = { step: data.step, actions: [] }
      turn.reactSteps.push(step)
    }
    const observation = typeof data.observation === 'string' ? data.observation : ''
    const isError = observation.startsWith('error:') || observation.includes('Traceback')
    step.actions.push({
      tool: typeof data.tool === 'string' ? data.tool : '',
      observation,
      tokens_used: typeof data.tokens_used === 'number' ? data.tokens_used : 0,
      isError
    })
    return
  }

  if (event.type === 'kb_growth') {
    const data = event.data as KbGrowthEvent
    if (typeof data?.entry_id !== 'string') return
    if (turn.kbGrowth.some((g) => g.entry_id === data.entry_id)) return
    turn.kbGrowth.push(data)
    return
  }

  if (event.type === 'qa_answer') {
    const data = event.data as { answer?: string; citations?: Citation[] }
    turn.answer = {
      text: typeof data.answer === 'string' ? data.answer : '',
      citations: Array.isArray(data.citations) ? data.citations : []
    }
    return
  }

  if (event.type === 'done') {
    if (_doneFlippedForTurn === turnId) return
    _doneFlippedForTurn = turnId
    turn.status = 'done'
    _state.status.value = 'done'
    return
  }

  if (event.type === 'error') {
    const data = event.data as { code?: string; message?: string }
    turn.status = 'error'
    turn.error = {
      code: typeof data?.code === 'string' ? data.code : 'STREAM_ERROR',
      message: typeof data?.message === 'string' ? data.message : 'unknown error'
    }
    _state.status.value = 'error'
  }
}

function bindSseTask(taskId: string, turnId: string): void {
  _sse = useSseTask(taskId)
  _sseCursor = 0
  _currentTurnId = turnId
  _doneFlippedForTurn = null

  _sseStopWatch = watch(
    () => _sse?.events.value.length ?? 0,
    (len) => {
      while (_sseCursor < len) {
        const ev = _sse?.events.value[_sseCursor]
        _sseCursor += 1
        if (ev) {
          applyEvent(turnId, ev)
          // Mirror into aggregated stream so cross-turn audit consumers
          // (useAuditJsonl kb_growth live-tail) see one append-only stream.
          _aggregatedSseEvents.value.push(ev)
        }
      }
    },
    { immediate: true }
  )
}

function appendTurn(turn: QaTurn): void {
  _state.turns.value.push(turn)
  while (_state.turns.value.length > TURNS_CAP) {
    _state.turns.value.shift()
  }
}

async function start(
  prompt: string,
  originatingStationId: string | null
): Promise<void> {
  // Tear down any prior SSE binding before starting the next turn so two
  // EventSources never coexist (per spec: "MUST NOT open more than one
  // EventSource per concurrently in-flight turn").
  disposeSse()

  const id = freshTurnId()
  const turn: QaTurn = {
    id,
    question: prompt,
    originatingStationId,
    taskId: null,
    ragHits: null,
    reactSteps: [],
    kbGrowth: [],
    answer: null,
    status: 'pending'
  }
  appendTurn(turn)
  _state.status.value = 'pending'

  // Sidecar request. workspace_root is left empty here — caller layer is
  // responsible for binding it via the SSE subscription wiring (P0 carries
  // workspace through `useSseTask` on its query string). Empty body keeps
  // start() decoupled from the routing layer; a future Phase 2 change will
  // route ws_path explicitly per design Open Question.
  const sidecar = useSidecar()
  let res: Response
  try {
    res = await sidecar.fetch('/qa', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        question: prompt,
        originating_station_id: originatingStationId
      })
    })
  } catch (err) {
    const live = findTurn(id)
    if (live) {
      live.status = 'error'
      live.error = {
        code: 'NETWORK_ERROR',
        message: err instanceof Error ? err.message : String(err)
      }
    }
    _state.status.value = 'error'
    return
  }

  if (!res.ok) {
    let detail: { code?: string; message?: string } | undefined
    try {
      const body = (await res.json()) as { detail?: typeof detail }
      detail = body?.detail
    } catch {
      detail = undefined
    }
    const live = findTurn(id)
    if (live) {
      live.status = 'error'
      live.error = {
        code: detail?.code ?? `HTTP_${res.status}`,
        message: detail?.message ?? `POST /qa failed with status ${res.status}`
      }
    }
    _state.status.value = 'error'
    return
  }

  const body = (await res.json()) as { task_id?: string }
  if (typeof body?.task_id !== 'string') {
    const live = findTurn(id)
    if (live) {
      live.status = 'error'
      live.error = { code: 'BAD_RESPONSE', message: 'POST /qa response missing task_id' }
    }
    _state.status.value = 'error'
    return
  }

  const live = findTurn(id)
  if (live) {
    live.taskId = body.task_id
    live.status = 'streaming'
  }
  _state.currentTaskId.value = body.task_id
  _state.status.value = 'streaming'
  bindSseTask(body.task_id, id)
}

function openDrawer(): void {
  _state.open.value = true
}

function close(): void {
  _state.open.value = false
}

export function useQaSession(): UseQaSessionApi {
  // Aggregated SSE event stream is exposed via the conventional `__sseEvents`
  // slot so `useAuditJsonl(.., 'kb_growth', { liveTailFromQaSession })` can
  // tap it without reaching into module internals. Treat as private — the
  // typed UseQaSessionApi does NOT advertise this slot.
  const api = {
    open: _state.open,
    turns: _state.turns,
    currentTaskId: _state.currentTaskId,
    status: _state.status,
    error: _state.error,
    start,
    openDrawer,
    close
  } as UseQaSessionApi
  ;(api as unknown as { __sseEvents: Ref<SseEvent[]> }).__sseEvents =
    _aggregatedSseEvents
  return api
}

// Test-only export. Wipes the module-level singleton so vitest tests can
// run in isolation without leaked state. Production code MUST NOT call this.
export function _resetForTest(): void {
  disposeSse()
  _state.open.value = false
  _state.turns.value = []
  _state.currentTaskId.value = null
  _state.status.value = 'idle'
  _state.error.value = null
  _aggregatedSseEvents.value = []
}
