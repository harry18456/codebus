import { ref, watch, type Ref } from 'vue'
import { useSidecar } from './useSidecar'

// SSE event surface for sidecar task channels. The composable only validates
// envelope structure (`type` is always a string); domain-specific payload
// shapes (e.g. `agent_thought`, `usage_delta`) are interpreted by callers.
export interface SseEvent {
  type: string
  data: unknown
}

export type SseStatus =
  | 'connecting'
  | 'open'
  | 'reconnecting'
  | 'closed'
  | 'error'

export interface SseTaskApi {
  events: Ref<SseEvent[]>
  status: Ref<SseStatus>
  error: Ref<Error | null>
  close: () => void
}

const TASK_ID_RE = /^(scan|kb|explore|generate|qa)_[0-9a-f]{8}$/
const BACKOFF_MS = [1000, 2000, 4000, 8000, 16000, 30000] as const
const EVENTS_CAP = 1000

// Known sidecar SSE event types — registered as named listeners so pages can
// observe them without each one wiring its own EventSource. The catch-all
// `message` handler picks up any event delivered without an `event:` line.
const NAMED_EVENT_TYPES = [
  'agent_thought',
  'agent_action_result',
  'judge_verdict',
  'coverage_gaps',
  'usage_delta',
  'llm_call',
  'progress',
  'budget_warning',
  'rag_hits',
  'kb_growth',
  'qa_answer',
  'done',
  'error'
] as const

export function useSseTask(taskId: string): SseTaskApi {
  const events: Ref<SseEvent[]> = ref([])
  const status: Ref<SseStatus> = ref('connecting')
  const error: Ref<Error | null> = ref(null)

  if (!TASK_ID_RE.test(taskId)) {
    status.value = 'error'
    error.value = new Error(
      `Invalid taskId "${taskId}"; must match ${TASK_ID_RE.source}`
    )
    return {
      events,
      status,
      error,
      close: () => {
        /* noop — no EventSource was opened */
      }
    }
  }

  const sidecar = useSidecar()
  let source: EventSource | null = null
  let backoffIndex = 0
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let manuallyClosed = false

  function appendEvent(ev: SseEvent): void {
    events.value.push(ev)
    while (events.value.length > EVENTS_CAP) {
      events.value.shift()
    }
  }

  function parseData(raw: string): unknown {
    try {
      return JSON.parse(raw)
    } catch {
      return raw
    }
  }

  function clearReconnectTimer(): void {
    if (reconnectTimer !== null) {
      clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
  }

  function scheduleReconnect(): void {
    if (manuallyClosed) {
      return
    }
    status.value = 'reconnecting'
    const delay = BACKOFF_MS[Math.min(backoffIndex, BACKOFF_MS.length - 1)]
    backoffIndex += 1
    clearReconnectTimer()
    reconnectTimer = setTimeout(open, delay ?? BACKOFF_MS[BACKOFF_MS.length - 1])
  }

  function attachListeners(es: EventSource): void {
    es.onopen = () => {
      status.value = 'open'
      backoffIndex = 0
      error.value = null
    }
    es.onerror = () => {
      es.close()
      if (source === es) {
        source = null
      }
      scheduleReconnect()
    }
    es.onmessage = (event: MessageEvent<string>) => {
      appendEvent({ type: 'message', data: parseData(event.data) })
    }
    for (const evType of NAMED_EVENT_TYPES) {
      es.addEventListener(evType, (event) => {
        const me = event as MessageEvent<string>
        appendEvent({ type: evType, data: parseData(me.data) })
      })
    }
  }

  function open(): void {
    if (manuallyClosed || source !== null) {
      return
    }
    if (!sidecar.ready.value || !sidecar.baseUrl.value || !sidecar.bearer.value) {
      return
    }
    status.value = 'connecting'
    const url = new URL(`${sidecar.baseUrl.value}/tasks/${taskId}/events`)
    // Browser-native EventSource cannot set custom headers, so the bearer
    // rides as a query parameter. The connection is loopback-only
    // (127.0.0.1); the bearer never crosses a network boundary, satisfying
    // CLAUDE.md invariant #5.
    url.searchParams.set('bearer', sidecar.bearer.value)
    source = new EventSource(url.toString())
    attachListeners(source)
  }

  const stopWatch = watch(
    [sidecar.ready, sidecar.bearer, sidecar.baseUrl],
    () => {
      if (source === null && !manuallyClosed) {
        open()
      }
    },
    { immediate: true }
  )

  function close(): void {
    manuallyClosed = true
    clearReconnectTimer()
    if (source !== null) {
      source.close()
      source = null
    }
    stopWatch()
    status.value = 'closed'
  }

  return { events, status, error, close }
}
