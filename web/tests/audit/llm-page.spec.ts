import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { reactive, ref, nextTick } from 'vue'
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

const fakeRoute = reactive({ params: {}, query: {} as Record<string, string> })
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

import LlmAuditPage from '~/pages/audit/llm.vue'

beforeEach(() => {
  invokeMock.mockReset()
  fakeRoute.query = {}
})

describe('/audit/llm page integration', () => {
  it('missing ws_path renders error without invoking IPC', async () => {
    fakeRoute.query = {}
    const wrapper = mount(LlmAuditPage)
    await nextTick()
    expect(invokeMock).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="missing-ws-path"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('row click opens inspector with the clicked underlying entry', async () => {
    fakeRoute.query = { ws_path: '/abs/ws' }
    invokeMock.mockResolvedValueOnce(fixture)

    const wrapper = mount(LlmAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    // The list renders timestamp-descending (entries.length rows).
    const rowEls = wrapper.findAll('[data-testid="llm-row"]')
    expect(rowEls).toHaveLength(fixture.length)

    // Click the first displayed row (which is the LAST underlying entry — desc display).
    await rowEls[0]!.trigger('click')
    await nextTick()

    // Inspector should be mounted with active-index pointing at the clicked entry.
    const inspector = wrapper.find('aside')
    expect(inspector.exists()).toBe(true)
    // The first displayed row corresponds to underlying index = fixture.length - 1.
    // The fixture's last entry is request_id "req_dup_target".
    expect(wrapper.text()).toContain('req_dup_target')
    wrapper.unmount()
  })

  it('filter chip narrows the visible list', async () => {
    fakeRoute.query = { ws_path: '/abs/ws' }
    invokeMock.mockResolvedValueOnce(fixture)

    const wrapper = mount(LlmAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    expect(wrapper.findAll('[data-testid="llm-row"]')).toHaveLength(fixture.length)

    // Toggle on the role=judge chip. Fixture has exactly 1 judge entry.
    await wrapper.find('button[data-chip="role:judge"]').trigger('click')
    await nextTick()

    const filteredRows = wrapper.findAll('[data-testid="llm-row"]')
    expect(filteredRows).toHaveLength(1)
    expect(filteredRows[0]!.text()).toContain('judge')
    wrapper.unmount()
  })

  it('empty entries shows the empty state, no inspector', async () => {
    fakeRoute.query = { ws_path: '/abs/ws' }
    invokeMock.mockResolvedValueOnce([])

    const wrapper = mount(LlmAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    expect(wrapper.text()).toContain('no LLM calls in this workspace yet')
    expect(wrapper.find('aside').exists()).toBe(false)
    wrapper.unmount()
  })

  it('E_AUDIT_TOO_LARGE error renders dedicated copy', async () => {
    fakeRoute.query = { ws_path: '/abs/ws' }
    invokeMock.mockRejectedValueOnce('E_AUDIT_TOO_LARGE')

    const wrapper = mount(LlmAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    expect(wrapper.text()).toContain('audit too large')
    wrapper.unmount()
  })
})
