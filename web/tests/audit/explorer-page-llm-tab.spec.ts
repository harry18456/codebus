import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { reactive, ref, nextTick } from 'vue'
import { lastEventSource } from '../setup'
import fixture from './fixtures/llm-calls.json'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: globalThis.fetch
  })
}))

const fakeRoute = reactive({
  params: { task_id: 'explore_4f2a8b91' },
  query: { ws_path: '/abs/ws' } as Record<string, string>
})
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

import ExplorerPage from '~/pages/explorer/[task_id].vue'

beforeEach(() => {
  invokeMock.mockReset()
  fakeRoute.params.task_id = 'explore_4f2a8b91'
  fakeRoute.query = { ws_path: '/abs/ws' }
})

describe('explorer page LLM tab integration', () => {
  it('llm tab binds useAuditJsonl rows + live-tails llm_call SSE events', async () => {
    invokeMock.mockResolvedValueOnce(fixture)
    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    // Switch to llm tab.
    const llmTab = wrapper.find('button[data-tab="llm"]')
    expect(llmTab.exists()).toBe(true)
    await llmTab.trigger('click')
    await nextTick()

    // AuditPanel rows should equal fixture entries.
    const rows = wrapper.findAll('[data-testid="audit-row"]')
    expect(rows).toHaveLength(fixture.length)

    // Now live-tail a new llm_call event via the SSE stream and watch
    // the AuditPanel grow.
    const es = lastEventSource()
    es._emit('llm_call', {
      request_id: 'req_live_99',
      role: 'chat',
      module: 'qa_agent',
      model: 'gpt-4o-mini',
      tokens: { prompt: 10, completion: 5 },
      cost_usd: 0.0001,
      latency_ms: 100
    })
    await nextTick()
    await nextTick()

    const grown = wrapper.findAll('[data-testid="audit-row"]')
    expect(grown).toHaveLength(fixture.length + 1)
    wrapper.unmount()
  })

  it('row click in llm tab opens the LlmCallInspector overlay', async () => {
    invokeMock.mockResolvedValueOnce(fixture)
    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    await wrapper.find('button[data-tab="llm"]').trigger('click')
    await nextTick()

    const rows = wrapper.findAll('[data-testid="audit-row"]')
    await rows[0]!.trigger('click')
    await nextTick()

    // The inspector aside should now render (matching its 4-tab strip).
    expect(wrapper.findAll('button[data-tab="wire"]')).toHaveLength(1)
    expect(wrapper.findAll('button[data-tab="response"]')).toHaveLength(1)
    wrapper.unmount()
  })

  it('missing ws_path: SSE still opens, llm tab shows ws_path required fallback', async () => {
    fakeRoute.query = {}
    invokeMock.mockResolvedValueOnce(fixture)
    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await nextTick()

    // SSE must still open (existing agent-console-p0 contract).
    expect(lastEventSource()).toBeDefined()

    // Switch to llm tab.
    await wrapper.find('button[data-tab="llm"]').trigger('click')
    await nextTick()

    // AuditPanel for llm tab should show the ws_path required fallback.
    expect(wrapper.text()).toContain('ws_path')
    wrapper.unmount()
  })
})
