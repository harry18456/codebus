import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { reactive, ref, nextTick } from 'vue'
import fixture from './fixtures/sanitize-audit.jsonl?raw'

// Tauri invoke spy. The page MUST only ever call `read_audit_jsonl` with
// `audit_kind: 'sanitize'`. Any other call is a spec violation.
const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

// Sidecar HTTP fetch spy. The page MUST only ever hit `/sanitizer/rules`.
// Other endpoints (`/auth/...`, `/explore`, `/qa`, ...) would violate the
// "Page does not call non-sanitize audit reads" spec scenario.
const fetchSpy = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchSpy(...args)
  })
}))

const fakeRoute = reactive({ params: {}, query: {} as Record<string, string> })
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

const SANITIZE_FIXTURE = fixture
  .split('\n')
  .filter((l) => l.trim().length > 0)
  .map((l) => JSON.parse(l))

beforeEach(() => {
  invokeMock.mockReset()
  fetchSpy.mockReset()
  fakeRoute.query = {}
})

import SanitizerAuditPage from '~/pages/audit/sanitizer.vue'

function jsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'Content-Type': 'application/json' }
  })
}

describe('/audit/sanitizer page integration', () => {
  it('valid workspace query renders inspector + banner with no station chrome', async () => {
    fakeRoute.query = { workspace: '/abs/ws-a' }
    invokeMock.mockResolvedValueOnce(SANITIZE_FIXTURE)
    fetchSpy.mockResolvedValue(
      jsonResponse({ rules_version: '2026-04-20-1', rules: [] })
    )

    const wrapper = mount(SanitizerAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    // Banner renders verbatim
    expect(wrapper.text()).toContain(
      'Audit metadata only · raw values are not retained per D-015.'
    )
    expect(wrapper.text()).toContain(
      'Placeholder reveal requires a future audit-unlock capability.'
    )
    // No R-01 station chrome — these should not render on the standalone page.
    const html = wrapper.html().toLowerCase()
    expect(html).not.toContain('data-component="stationnav"')
    expect(html).not.toContain('data-component="mocindex"')
    // The IPC was called with audit_kind: sanitize.
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock).toHaveBeenCalledWith('read_audit_jsonl', {
      workspaceRoot: '/abs/ws-a',
      auditKind: 'sanitize'
    })

    // Click the first row → inspector overlay surfaces a metadata row label.
    const rowEls = wrapper.findAll('[data-testid="sanitize-row"]')
    expect(rowEls.length).toBeGreaterThan(0)
    await rowEls[0]!.trigger('click')
    await nextTick()
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(true)
    wrapper.unmount()
  })

  it('missing workspace query renders empty state with no IPC and banner still visible', async () => {
    fakeRoute.query = {}
    const wrapper = mount(SanitizerAuditPage, { attachTo: document.body })
    await nextTick()

    expect(invokeMock).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="missing-workspace"]').exists()).toBe(
      true
    )
    expect(wrapper.text()).toContain(
      'Audit metadata only · raw values are not retained per D-015.'
    )
    wrapper.unmount()
  })

  it('only invokes Tauri read_audit_jsonl with audit_kind sanitize and only fetches /sanitizer/rules', async () => {
    fakeRoute.query = { workspace: '/abs/ws-a' }
    invokeMock.mockResolvedValueOnce(SANITIZE_FIXTURE)
    fetchSpy.mockResolvedValue(
      jsonResponse({ rules_version: '2026-04-20-1', rules: [] })
    )

    const wrapper = mount(SanitizerAuditPage, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()

    // No other IPC calls.
    for (const call of invokeMock.mock.calls) {
      expect(call[0]).toBe('read_audit_jsonl')
      expect((call[1] as { auditKind: string }).auditKind).toBe('sanitize')
    }

    // Sidecar fetch limited to /sanitizer/rules.
    for (const call of fetchSpy.mock.calls) {
      expect(call[0]).toBe('/sanitizer/rules')
    }

    wrapper.unmount()
  })
})
