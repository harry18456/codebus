import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, h, nextTick, ref, type Ref } from 'vue'
import qaStreamFixture from './fixtures/qa-stream.json'

// SSE mock — events ref is shared with useQaSession (the composable
// installs a watcher on the same ref returned here).
const sseEventsRef: Ref<Array<{ type: string; data: unknown }>> = ref([])
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
  close: vi.fn()
}))

vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: vi.fn(async () =>
      new Response(JSON.stringify({ task_id: 'qa_integ001' }), {
        status: 202,
        headers: { 'Content-Type': 'application/json' }
      })
    )
  })
}))

// Tauri invoke mock — used by useAuditJsonl when explorer page mounts.
const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

import { reactive } from 'vue'
const fakeRoute = reactive<{ params: Record<string, string>; query: Record<string, string> }>(
  { params: {}, query: {} }
)
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

import DefaultLayout from '~/layouts/default.vue'
import ExplorerPage from '~/pages/explorer/[task_id].vue'
import { useQaSession, _resetUseQaSessionForTest } from '~/composables/useQaSession'

beforeEach(() => {
  _resetUseQaSessionForTest()
  sseEventsRef.value = []
  invokeMock.mockReset()
  fakeRoute.params = {}
  fakeRoute.query = {}
})

async function flush(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
}

function withLayout(slotMarkup: string) {
  return defineComponent({
    setup() {
      return () =>
        h(DefaultLayout, null, {
          default: () => h('div', { 'data-testid': 'fake-page', innerHTML: slotMarkup })
        })
    }
  })
}

describe('QA overlay layout-level integration', () => {
  it('Cmd+K opens drawer from inside the layout host', async () => {
    const Host = withLayout('<p>fake station page</p>')
    const wrapper = mount(Host, { attachTo: document.body })
    await flush()
    expect(useQaSession().open.value).toBe(false)
    expect(wrapper.findAll('aside[data-component="QAOverlay"]')).toHaveLength(0)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))
    await nextTick()
    expect(useQaSession().open.value).toBe(true)
    expect(
      wrapper.findAll('aside[data-component="QAOverlay"]').length
    ).toBeGreaterThan(0)
    wrapper.unmount()
  })

  it('SSE event chain fills the active turn four phases', async () => {
    const Host = withLayout('<p>fake station page</p>')
    const wrapper = mount(Host, { attachTo: document.body })
    await flush()

    // Open drawer + start a turn.
    const session = useQaSession()
    session.openDrawer()
    await session.start('why atomic write?', 's03-production')
    await flush()

    // Inject SSE events one by one (mimicking sidecar push order).
    for (const event of qaStreamFixture) {
      sseEventsRef.value = [...sseEventsRef.value, event]
      await flush()
    }

    const turn = session.turns.value[0]!
    expect(turn.ragHits).not.toBeNull()
    expect(turn.ragHits!.length).toBeGreaterThan(0)
    expect(turn.reactSteps.length).toBeGreaterThan(0)
    expect(turn.kbGrowth.length).toBeGreaterThan(0)
    expect(turn.answer).not.toBeNull()
    expect(turn.status).toBe('done')

    // Drawer renders the four-phase content visibly.
    const text = wrapper.text()
    expect(text).toContain('① RAG 探查')
    expect(text).toContain('② ReAct loop')
    expect(text).toContain('③ 回答')
    expect(text).toContain('Atomic writes here use a temp-file + rename pattern')
    wrapper.unmount()
  })

  it('Escape closes the drawer mid-session', async () => {
    const Host = withLayout('<p>fake station page</p>')
    const wrapper = mount(Host, { attachTo: document.body })
    await flush()
    useQaSession().openDrawer()
    await nextTick()
    expect(useQaSession().open.value).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(useQaSession().open.value).toBe(false)
    expect(wrapper.findAll('aside[data-component="QAOverlay"]')).toHaveLength(0)
    wrapper.unmount()
  })
})

describe('Explorer page kb_growth tab live-tails QA SSE', () => {
  it('kb_growth tab gains a row when a kb_growth SSE event lands on useQaSession', async () => {
    fakeRoute.params = { task_id: 'explore_4f2a8b91' }
    fakeRoute.query = { ws_path: '/abs/ws' }
    invokeMock.mockResolvedValue([])

    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await flush()

    // Switch to kb_growth tab — initial disk is empty.
    const kbTab = wrapper.find('button[data-tab="kb_growth"]')
    await kbTab.trigger('click')
    await flush()
    expect(wrapper.findAll('[data-testid="audit-row"]')).toHaveLength(0)

    // Drive a kb_growth event through the singleton useQaSession aggregate
    // stream; useAuditJsonl(kb_growth) should observe it via __sseEvents.
    const session = useQaSession() as unknown as {
      __sseEvents: { value: Array<{ type: string; data: unknown }> }
    }
    session.__sseEvents.value.push({
      type: 'kb_growth',
      data: {
        entry_id: 'live_a14f9c2e',
        source: 'src/storage/atomic.ts:12-38',
        related_stations: ['s03-production'],
        originating_station_id: 's03-production'
      }
    })
    await flush()

    const rows = wrapper.findAll('[data-testid="audit-row"]')
    expect(rows.length).toBe(1)
    expect(rows[0]!.text()).toContain('live_a14f9c2e')
    wrapper.unmount()
  })
})
