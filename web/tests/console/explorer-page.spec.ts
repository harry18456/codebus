import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref, nextTick, reactive } from 'vue'
import { getOpenedEventSources, lastEventSource } from '../setup'

// Pin sidecar singleton ready=true so useSseTask opens immediately.
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: globalThis.fetch
  })
}))

// Reactive route stub so we can swap params.task_id mid-test (covers spec
// scenario "Route change closes prior SSE before opening new one").
const fakeRoute = reactive({ params: { task_id: '' }, query: {} })
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

import ExplorerPage from '~/pages/explorer/[task_id].vue'
import fixture from './fixtures/explorer-stream.json'

interface Envelope {
  type: string
  data: unknown
}

function replayFixture(events: Envelope[]): void {
  const es = lastEventSource()
  for (const ev of events) es._emit(ev.type, ev.data)
}

describe('explorer page integration', () => {
  it('opens exactly one EventSource and renders three StepCards from fixture', async () => {
    fakeRoute.params.task_id = 'explore_4f2a8b91'
    const wrapper = mount(ExplorerPage)
    await nextTick()

    expect(getOpenedEventSources()).toHaveLength(1)
    // Timeline is initially empty → placeholder shown.
    expect(wrapper.find('[data-testid="timeline-placeholder"]').exists()).toBe(true)

    replayFixture(fixture as Envelope[])
    await nextTick()
    await nextTick()

    const cards = wrapper.findAll('article[data-step]')
    expect(cards).toHaveLength(3)
    expect(cards[0]?.attributes('data-step')).toBe('1')
    expect(cards[1]?.attributes('data-step')).toBe('2')
    expect(cards[2]?.attributes('data-step')).toBe('3')

    // Done flag flipped after replay.
    expect(wrapper.text()).toContain('done')
    wrapper.unmount()
  })

  it('reasoning tab receives live audit rows from useExplorerStream', async () => {
    fakeRoute.params.task_id = 'explore_4f2a8b91'
    const wrapper = mount(ExplorerPage)
    await nextTick()
    replayFixture(fixture as Envelope[])
    await nextTick()
    await nextTick()

    // Reasoning rows = 3 thoughts + 4 action results + 3 judges = 10 rows
    // (steps 1+2+3 = 3 thoughts; step 1 = 1 act, step 2 = 2 acts, step 3 = 1 act = 4; 3 judges).
    const rows = wrapper.findAll('[class*="grid-cols-[56px_1fr_auto]"]')
    expect(rows.length).toBeGreaterThanOrEqual(3)
    // Spot-check that audit body mentions a known thought string from fixture.
    expect(wrapper.text()).toContain('list the src/ directory')
    wrapper.unmount()
  })

  it('non-reasoning tab receives an empty rows binding and shows empty-state placeholder', async () => {
    fakeRoute.params.task_id = 'explore_4f2a8b91'
    const wrapper = mount(ExplorerPage)
    await nextTick()
    replayFixture(fixture as Envelope[])
    await nextTick()

    // Click the "tool" tab to switch active tab.
    const toolTab = wrapper.find('button[data-tab="tool"]')
    expect(toolTab.exists()).toBe(true)
    await toolTab.trigger('click')
    await nextTick()

    // Empty-state placeholder MUST appear (AuditPanel renders data-empty="true"
    // when its rows array is empty).
    expect(wrapper.find('[data-empty="true"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('route change closes prior SSE before opening the next', async () => {
    fakeRoute.params.task_id = 'explore_aaaaaaaa'
    const wrapper = mount(ExplorerPage)
    await nextTick()

    expect(getOpenedEventSources()).toHaveLength(1)
    const first = lastEventSource()

    fakeRoute.params.task_id = 'explore_bbbbbbbb'
    await nextTick()
    await nextTick()

    expect(getOpenedEventSources()).toHaveLength(2)
    // Prior connection closed before new one wired up.
    expect(first.readyState).toBe(2 /* CLOSED */)
    wrapper.unmount()
  })

  it('invalid task_id rejects without opening EventSource', async () => {
    fakeRoute.params.task_id = 'not-a-task-id'
    const wrapper = mount(ExplorerPage)
    await nextTick()

    expect(getOpenedEventSources()).toHaveLength(0)
    expect(wrapper.find('[data-testid="invalid-task-id"]').exists()).toBe(true)
    wrapper.unmount()
  })
})
