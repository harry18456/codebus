import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick, type Ref } from 'vue'
import qaStreamFixture from './fixtures/qa-stream.json'

// Mock useSidecar so start() can drive POST /qa through a controllable spy.
const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

// Mock useSseTask so we can drive the SSE event chain via a controllable
// reactive `events` ref. vi.hoisted lets the mock factory close over our
// shared spy + ref without TDZ issues from the hoisted vi.mock.
const sseEventsRef: Ref<Array<{ type: string; data: unknown }>> = ref([])
const sseCloseSpy = vi.fn()
const { useSseTaskMock } = vi.hoisted(() => ({
  useSseTaskMock: vi.fn()
}))
vi.mock('~/composables/useSseTask', () => ({
  useSseTask: useSseTaskMock
}))
useSseTaskMock.mockImplementation(() => ({
  events: sseEventsRef,
  status: ref('open' as const),
  error: ref(null),
  close: sseCloseSpy
}))

import { useQaSession, _resetForTest } from '~/composables/useQaSession'

function jsonResponse(body: unknown, init: { status: number } = { status: 202 }): Response {
  return new Response(JSON.stringify(body), {
    status: init.status,
    headers: { 'Content-Type': 'application/json' }
  })
}

beforeEach(() => {
  fetchMock.mockReset()
  sseCloseSpy.mockReset()
  useSseTaskMock.mockClear()
  sseEventsRef.value = []
  _resetForTest()
})

async function flush(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
}

describe('useQaSession singleton + start() flow', () => {
  it('two callers receive the same singleton state (Object.is)', () => {
    const a = useQaSession()
    const b = useQaSession()
    expect(Object.is(a.turns, b.turns)).toBe(true)
    expect(Object.is(a.open, b.open)).toBe(true)
    expect(Object.is(a.currentTaskId, b.currentTaskId)).toBe(true)
    expect(Object.is(a.status, b.status)).toBe(true)
  })

  it('start() appends a pending turn before POST /qa resolves', async () => {
    let resolveFetch: ((value: Response) => void) | null = null
    fetchMock.mockReturnValue(
      new Promise<Response>((resolve) => {
        resolveFetch = resolve
      })
    )
    const api = useQaSession()
    const p = api.start('why atomic write?', 's03-production')
    await nextTick()
    expect(api.turns.value).toHaveLength(1)
    expect(api.turns.value[0]!.question).toBe('why atomic write?')
    expect(api.turns.value[0]!.originatingStationId).toBe('s03-production')
    expect(api.turns.value[0]!.status).toBe('pending')
    expect(api.turns.value[0]!.taskId).toBeNull()
    resolveFetch!(jsonResponse({ task_id: 'qa_a1b2c3d4' }))
    await p
  })

  it('409 TASK_IN_FLIGHT marks the turn errored without spawning SSE', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ detail: { code: 'TASK_IN_FLIGHT', message: 'busy' } }, { status: 409 })
    )
    const api = useQaSession()
    await api.start('hi', null)
    await flush()
    const turn = api.turns.value[0]!
    expect(turn.status).toBe('error')
    expect(turn.error?.code).toBe('TASK_IN_FLIGHT')
    // useSseTask MUST NOT have been called for the 409 path.
    expect(useSseTaskMock.mock.calls.length).toBe(0)
  })

  it('rag_hits event populates the active turn ragHits', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: 'qa_abcd1234' }))
    const api = useQaSession()
    await api.start('q?', null)
    await flush()
    const ragHitsEvent = qaStreamFixture.find((e) => e.type === 'rag_hits')!
    sseEventsRef.value = [ragHitsEvent]
    await flush()
    expect(api.turns.value[0]!.ragHits).toHaveLength(3)
    expect(api.turns.value[0]!.ragHits![0]!.score).toBe(0.71)
  })

  it('kb_growth dedup by entry_id', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: 'qa_dedup0001' }))
    const api = useQaSession()
    await api.start('q?', null)
    await flush()
    const kbGrowthEvent = qaStreamFixture.find((e) => e.type === 'kb_growth')!
    sseEventsRef.value = [kbGrowthEvent, kbGrowthEvent]
    await flush()
    expect(api.turns.value[0]!.kbGrowth).toHaveLength(1)
  })

  it('turns FIFO cap at 50', async () => {
    // Each start() consumes one Response; mock implementation returns
    // a fresh Response per call so .json() doesn't fail on the second use.
    fetchMock.mockImplementation(async () => jsonResponse({ task_id: 'qa_capxxxxx' }))
    const api = useQaSession()
    for (let i = 0; i < 60; i++) {
      // Inject a synthetic done event so each turn finishes and the next start() is allowed.
      sseEventsRef.value = []
      await api.start(`prompt_${i}`, null)
      await flush()
      sseEventsRef.value = [{ type: 'done', data: {} }]
      await flush()
    }
    expect(api.turns.value).toHaveLength(50)
    // First surviving turn is the 11th (prompt_10)
    expect(api.turns.value[0]!.question).toBe('prompt_10')
  })

  it('done event flips status to done exactly once', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: 'qa_doneonce0' }))
    const api = useQaSession()
    await api.start('q?', null)
    await flush()
    sseEventsRef.value = [{ type: 'done', data: {} }]
    await flush()
    expect(api.turns.value[0]!.status).toBe('done')
    // Track turn reference to ensure no re-dispatch creates a new object
    const before = api.turns.value[0]
    sseEventsRef.value = [
      { type: 'done', data: {} },
      { type: 'done', data: {} }
    ]
    await flush()
    expect(api.turns.value[0]).toBe(before)
    expect(api.turns.value[0]!.status).toBe('done')
  })

  it('_resetForTest clears module-level state between tests', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: 'qa_resetxxx0' }))
    const api = useQaSession()
    await api.start('residual?', null)
    await flush()
    expect(api.turns.value.length).toBeGreaterThan(0)
    _resetForTest()
    expect(api.turns.value).toHaveLength(0)
    expect(api.open.value).toBe(false)
    expect(api.currentTaskId.value).toBeNull()
  })
})
