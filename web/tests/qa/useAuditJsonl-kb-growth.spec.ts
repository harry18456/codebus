import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick, type Ref } from 'vue'
import kbGrowthFixture from './fixtures/kb-growth.json'
import qaStreamFixture from './fixtures/qa-stream.json'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

import { useAuditJsonl } from '~/composables/useAuditJsonl'
import type { UseQaSessionApi } from '~/composables/useQaSession'

interface FakeQaSession {
  __sseEvents: Ref<Array<{ type: string; data: unknown }>>
  open: Ref<boolean>
  turns: Ref<unknown[]>
  currentTaskId: Ref<string | null>
  status: Ref<'idle'>
  error: Ref<Error | null>
  start: ReturnType<typeof vi.fn>
  openDrawer: ReturnType<typeof vi.fn>
  close: ReturnType<typeof vi.fn>
}

function makeFakeSession(): FakeQaSession {
  return {
    __sseEvents: ref<Array<{ type: string; data: unknown }>>([]),
    open: ref(false),
    turns: ref([]),
    currentTaskId: ref(null),
    status: ref('idle'),
    error: ref(null),
    start: vi.fn(),
    openDrawer: vi.fn(),
    close: vi.fn()
  }
}

beforeEach(() => {
  invokeMock.mockReset()
})

describe('useAuditJsonl kb_growth live-tail from useQaSession', () => {
  it('appends kb_growth SSE events into entries (in addition to disk)', async () => {
    invokeMock.mockResolvedValueOnce(kbGrowthFixture)
    const session = makeFakeSession()
    const audit = useAuditJsonl('/abs/ws', 'kb_growth', {
      liveTailFromQaSession: session as unknown as UseQaSessionApi
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    const initial = audit.entries.value.length
    expect(initial).toBe(kbGrowthFixture.length)

    // Push two new kb_growth SSE events with distinct entry_ids
    session.__sseEvents.value.push({
      type: 'kb_growth',
      data: {
        entry_id: 'new1234a',
        source: 'src/foo.ts:1-10',
        related_stations: ['s07-overview'],
        originating_station_id: 's07-overview'
      }
    })
    await nextTick()
    session.__sseEvents.value.push({
      type: 'kb_growth',
      data: {
        entry_id: 'new5678b',
        source: 'src/bar.ts:20-40',
        related_stations: ['s07-overview'],
        originating_station_id: 's07-overview'
      }
    })
    await nextTick()
    expect(audit.entries.value.length).toBe(initial + 2)
  })

  it('dedups by entry_id when SSE event matches an existing disk entry', async () => {
    invokeMock.mockResolvedValueOnce(kbGrowthFixture)
    const session = makeFakeSession()
    const audit = useAuditJsonl('/abs/ws', 'kb_growth', {
      liveTailFromQaSession: session as unknown as UseQaSessionApi
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    const initial = audit.entries.value.length

    // 'a14f9c2e' exists in both kb-growth.json and qa-stream.json
    const dupEvent = qaStreamFixture.find(
      (e) => e.type === 'kb_growth' && (e.data as { entry_id?: string }).entry_id === 'a14f9c2e'
    )!
    session.__sseEvents.value.push(dupEvent)
    await nextTick()
    expect(audit.entries.value.length).toBe(initial)
  })

  it('ignores liveTailFromQaSession when kind is not kb_growth', async () => {
    invokeMock.mockResolvedValueOnce([])
    const session = makeFakeSession()
    const audit = useAuditJsonl('/abs/ws', 'tool', {
      liveTailFromQaSession: session as unknown as UseQaSessionApi
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    expect(audit.entries.value.length).toBe(0)

    session.__sseEvents.value.push({
      type: 'kb_growth',
      data: {
        entry_id: 'should_not_appear',
        source: 'x',
        related_stations: [],
        originating_station_id: null
      }
    })
    await nextTick()
    expect(audit.entries.value.length).toBe(0)
    expect(audit.error.value).toBeNull()
  })
})
