import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick } from 'vue'
import fixture from './fixtures/llm-calls.json'

// Mock @tauri-apps/api/core::invoke before importing useAuditJsonl so
// the dynamic import inside the composable resolves to our stub.
const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

// Provide a controllable explorer-stream-like SSE event source via a
// reactive ref bag. The composable taps it via opts.liveTailFromExplorerStream.
import type { SseEvent, UseExplorerStreamApi } from '~/composables/useExplorerStream'

interface FakeStream {
  __sseEvents: ReturnType<typeof ref<SseEvent[]>>
}

function makeFakeStream(): UseExplorerStreamApi & FakeStream {
  const events = ref<SseEvent[]>([])
  return {
    __sseEvents: events,
    stepBuckets: ref(new Map()),
    progress: ref(null),
    coverageBanner: ref(null),
    budgetBanner: ref({}),
    auditRows: ref([]),
    status: ref('open'),
    error: ref(null),
    done: ref(false),
    close: () => {}
  } as unknown as UseExplorerStreamApi & FakeStream
}

import { useAuditJsonl } from '~/composables/useAuditJsonl'

beforeEach(() => {
  invokeMock.mockReset()
})

describe('useAuditJsonl', () => {
  it('initial load populates entries from Tauri command (kind=llm)', async () => {
    invokeMock.mockResolvedValueOnce(fixture)
    const audit = useAuditJsonl('/abs/ws', 'llm')
    await nextTick()
    await audit.reload // touch reload promise existence
    // Wait for invoke promise
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('read_audit_jsonl', {
      workspaceRoot: '/abs/ws',
      auditKind: 'llm'
    })
    expect(audit.entries.value.length).toBe(fixture.length)
    expect(audit.loading.value).toBe(false)
    expect(audit.error.value).toBeNull()
  })

  it('live-tail appends llm_call SSE events from explorer stream', async () => {
    invokeMock.mockResolvedValueOnce(fixture)
    const stream = makeFakeStream()
    const audit = useAuditJsonl('/abs/ws', 'llm', {
      liveTailFromExplorerStream: stream
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    const initialCount = audit.entries.value.length

    stream.__sseEvents.value!.push({
      type: 'llm_call',
      data: {
        request_id: 'req_live_a',
        role: 'chat',
        module: 'qa_agent',
        model: 'gpt-4o-mini',
        tokens: { prompt: 10, completion: 5 },
        cost_usd: 0.0001,
        latency_ms: 200
      }
    })
    await nextTick()
    stream.__sseEvents.value!.push({
      type: 'llm_call',
      data: {
        request_id: 'req_live_b',
        role: 'reasoning',
        module: 'explorer',
        model: 'gpt-4o-mini',
        tokens: { prompt: 50, completion: 20 },
        cost_usd: 0.0003,
        latency_ms: 480
      }
    })
    await nextTick()

    expect(audit.entries.value.length).toBe(initialCount + 2)
    expect(audit.entries.value.at(-2)?.request_id).toBe('req_live_a')
    expect(audit.entries.value.at(-1)?.request_id).toBe('req_live_b')
  })

  it('live-tail ignores non-llm kinds (no append, no error)', async () => {
    invokeMock.mockResolvedValueOnce([])
    const stream = makeFakeStream()
    const audit = useAuditJsonl('/abs/ws', 'sanitize', {
      liveTailFromExplorerStream: stream
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    expect(audit.entries.value.length).toBe(0)

    stream.__sseEvents.value!.push({
      type: 'llm_call',
      data: { request_id: 'req_x', role: 'chat', module: 'qa' }
    })
    await nextTick()

    expect(audit.entries.value.length).toBe(0)
    expect(audit.error.value).toBeNull()
  })

  it('dedup by request_id prevents disk + SSE double-push', async () => {
    invokeMock.mockResolvedValueOnce(fixture)
    const stream = makeFakeStream()
    const audit = useAuditJsonl('/abs/ws', 'llm', {
      liveTailFromExplorerStream: stream
    })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    const initialCount = audit.entries.value.length

    // Push event with same request_id as fixture[5] (req_dup_target).
    stream.__sseEvents.value!.push({
      type: 'llm_call',
      data: {
        request_id: 'req_dup_target',
        role: 'chat',
        module: 'qa_agent',
        model: 'gpt-4o-mini',
        tokens: { prompt: 100, completion: 50 },
        cost_usd: 0.0002,
        latency_ms: 600
      }
    })
    await nextTick()

    expect(audit.entries.value.length).toBe(initialCount)
  })

  it('E_AUDIT_TOO_LARGE surfaces as Error with code in message', async () => {
    invokeMock.mockRejectedValueOnce('E_AUDIT_TOO_LARGE')
    const audit = useAuditJsonl('/abs/ws', 'llm')
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    expect(audit.error.value).not.toBeNull()
    expect(audit.error.value?.message).toContain('E_AUDIT_TOO_LARGE')
    expect(audit.entries.value).toEqual([])
  })

  it('forwards reload() to re-invoke the Tauri command', async () => {
    invokeMock.mockResolvedValueOnce(fixture).mockResolvedValueOnce([fixture[0]])
    const audit = useAuditJsonl('/abs/ws', 'llm')
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    expect(audit.entries.value.length).toBe(fixture.length)

    await audit.reload()
    expect(invokeMock).toHaveBeenCalledTimes(2)
    expect(audit.entries.value.length).toBe(1)
  })
})
