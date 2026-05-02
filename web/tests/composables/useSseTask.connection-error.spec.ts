import { describe, expect, it, vi } from 'vitest'
import { ref, nextTick } from 'vue'
import { lastEventSource } from '../setup'

// Stub useSidecar so the inner useSseTask sees ready=true with a valid
// bearer/baseUrl — without booting the real Tauri IPC handshake. Mock the
// `~` alias path; vitest.config.ts maps it to `app/` so this matches the
// relative `./useSidecar` import from useSseTask too.
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: globalThis.fetch
  })
}))

import { useSseTask } from '~/composables/useSseTask'

const TASK_ID = 'scan_deadbeef'

// Backs spec scenario "Named error listener ignores connection-level
// errors" in
// openspec/changes/sidecar-sse-named-events-and-error-listener-fix/specs/frontend-shell/spec.md
//
// Bug being locked down: the previous implementation registered `'error'`
// inside the catch-all NAMED_EVENT_TYPES loop, so EventSource's native
// connection-level error (a generic Event, not a MessageEvent) silently
// pushed a `{type:'error', data:'undefined'}` phantom into the events
// array. The composable's onerror reconnect path was correct, but the
// duplicate listener corrupted the events stream and downstream
// composables that read `ev.type === 'error'` (e.g. useWorkspaceOnramp)
// fired their error branch on every clean stream close.
describe('useSseTask connection-error / server-error distinction', () => {
  it('does NOT push an event when EventSource fires a connection-level error', async () => {
    const sse = useSseTask(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._simulateOpen()
    await nextTick()

    // Simulate the server cleanly closing the stream (or network drop):
    // browser EventSource fires a generic `Event('error')` to BOTH
    // `.onerror` and any `addEventListener('error', ...)` listeners.
    es._simulateError()
    await nextTick()

    const errorEvents = sse.events.value.filter((e) => e.type === 'error')
    expect(errorEvents).toHaveLength(0)
    sse.close()
  })

  it('DOES push an event when server emits event: error SSE message', async () => {
    const sse = useSseTask(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._simulateOpen()
    await nextTick()

    // Simulate sidecar emitting `event: error\ndata: {"code":"OOPS",...}`
    // — the FakeEventSource's `_emit(type, data)` dispatches a
    // MessageEvent to addEventListener listeners, mirroring how a real
    // EventSource delivers server-sent named events.
    es._emit('error', { code: 'OOPS', message: 'server says no' })
    await nextTick()

    const errorEvents = sse.events.value.filter((e) => e.type === 'error')
    expect(errorEvents).toHaveLength(1)
    expect(errorEvents[0]?.data).toEqual({ code: 'OOPS', message: 'server says no' })
    sse.close()
  })

  it('connection-level error still triggers onerror reconnect path', async () => {
    const sse = useSseTask(TASK_ID)
    await nextTick()
    const es = lastEventSource()
    es._simulateOpen()
    await nextTick()

    const closeSpy = vi.spyOn(es, 'close')

    // The composable's onerror handler MUST still fire (it calls
    // `es.close()` then schedules reconnect). Simulating the
    // connection-level error must invoke the reconnect path.
    es._simulateError()
    await nextTick()

    // close() called by onerror (reconnect path)
    expect(closeSpy).toHaveBeenCalled()
    sse.close()
  })
})
