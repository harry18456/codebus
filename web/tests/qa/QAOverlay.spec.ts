import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref, nextTick, type Ref } from 'vue'

// Hoisted SSE mock so useQaSession's start path doesn't try to spawn a real
// EventSource during these component-level tests.
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
      new Response(JSON.stringify({ task_id: 'qa_overlay00' }), {
        status: 202,
        headers: { 'Content-Type': 'application/json' }
      })
    )
  })
}))

import QAOverlay from '~/components/qa/QAOverlay.vue'
import { useQaSession, _resetForTest } from '~/composables/useQaSession'

// Layout-mirror keyboard handler. Spec says listeners MUST live in the
// layout host, NOT in the component. Tests register the same handler inline
// to simulate the layout host.
function installLayoutShortcuts(): () => void {
  const session = useQaSession()
  const handler = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && session.open.value) {
      e.preventDefault()
      session.close()
      return
    }
    if ((e.metaKey || e.ctrlKey) && e.key === 'k' && !session.open.value) {
      e.preventDefault()
      session.openDrawer()
    }
  }
  window.addEventListener('keydown', handler)
  return () => window.removeEventListener('keydown', handler)
}

let teardownShortcuts: (() => void) | null = null

beforeEach(() => {
  _resetForTest()
  sseEventsRef.value = []
  teardownShortcuts = installLayoutShortcuts()
})

afterEach(() => {
  teardownShortcuts?.()
  teardownShortcuts = null
})

describe('QAOverlay drawer behavior', () => {
  it('renders nothing when closed (zero aside elements)', () => {
    const wrapper = mount(QAOverlay)
    expect(useQaSession().open.value).toBe(false)
    expect(wrapper.findAll('aside')).toHaveLength(0)
    wrapper.unmount()
  })

  it('Cmd+K opens the drawer when closed', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    expect(wrapper.findAll('aside')).toHaveLength(0)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))
    await nextTick()
    expect(useQaSession().open.value).toBe(true)
    expect(wrapper.findAll('aside').length).toBeGreaterThan(0)
    wrapper.unmount()
  })

  it('Cmd+K is no-op when drawer is already open', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    useQaSession().openDrawer()
    await nextTick()
    expect(useQaSession().open.value).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true }))
    await nextTick()
    // Still open, no toggle to close.
    expect(useQaSession().open.value).toBe(true)
    wrapper.unmount()
  })

  it('Escape closes the open drawer', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    useQaSession().openDrawer()
    await nextTick()
    expect(useQaSession().open.value).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await nextTick()
    expect(useQaSession().open.value).toBe(false)
    expect(wrapper.findAll('aside')).toHaveLength(0)
    wrapper.unmount()
  })

  it('clicking the dim layer closes the drawer', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    useQaSession().openDrawer()
    await nextTick()
    const dim = wrapper.find('[data-testid="qa-dim-layer"]')
    expect(dim.exists()).toBe(true)
    await dim.trigger('click')
    expect(useQaSession().open.value).toBe(false)
    wrapper.unmount()
  })

  it('clicking inside the aside does not close', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    useQaSession().openDrawer()
    await nextTick()
    const aside = wrapper.find('aside')
    expect(aside.exists()).toBe(true)
    await aside.trigger('click')
    expect(useQaSession().open.value).toBe(true)
    wrapper.unmount()
  })

  it('send button disabled while previous turn is streaming', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    const session = useQaSession()
    session.openDrawer()
    // Append a streaming turn manually.
    session.turns.value.push({
      id: 'turn_streaming',
      question: 'q?',
      originatingStationId: null,
      taskId: 'qa_aaaaaaaa',
      ragHits: null,
      reactSteps: [],
      kbGrowth: [],
      answer: null,
      status: 'streaming'
    })
    await nextTick()
    const sendBtn = wrapper.find('[data-testid="qa-send-button"]')
    expect(sendBtn.exists()).toBe(true)
    expect(sendBtn.attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  it('empty turns shows the Cmd+K placeholder copy', async () => {
    const wrapper = mount(QAOverlay, { attachTo: document.body })
    useQaSession().openDrawer()
    await nextTick()
    expect(useQaSession().turns.value).toHaveLength(0)
    expect(wrapper.text()).toContain('Cmd+K')
    wrapper.unmount()
  })
})
