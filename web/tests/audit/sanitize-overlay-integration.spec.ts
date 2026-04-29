import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { reactive, ref, nextTick } from 'vue'
import sanitizeFixtureRaw from './fixtures/sanitize-audit.jsonl?raw'
import llmFixture from './fixtures/llm-calls.json'

const SANITIZE_FIXTURE = sanitizeFixtureRaw
  .split('\n')
  .filter((l) => l.trim().length > 0)
  .map((l) => JSON.parse(l))

// Shared invoke mock used by both Explorer and Station page tests. Routes
// `read_audit_jsonl` based on `auditKind` and ignores other commands so
// the station page bootstrap (file IPC) can fail silently without
// blocking the audit-rail integration assertions.
const invokeMock = vi.fn(async (cmd: string, args: Record<string, unknown>) => {
  if (cmd === 'read_audit_jsonl') {
    if (args.auditKind === 'sanitize') return SANITIZE_FIXTURE
    if (args.auditKind === 'llm') return llmFixture
    return []
  }
  // Other commands (read_tutorial_file / list_tutorial_tasks /
  // write_progress_file) are not relevant to the audit-rail integration;
  // throw a benign error so bootstrap takes the "no tutorial" branch
  // without crashing.
  throw new Error(`unmocked command: ${cmd}`)
})
vi.mock('@tauri-apps/api/core', () => {
  return {
    __esModule: true,
    invoke: (...args: unknown[]) =>
      invokeMock(...(args as Parameters<typeof invokeMock>)),
    default: {
      invoke: (...args: unknown[]) =>
        invokeMock(...(args as Parameters<typeof invokeMock>))
    }
  }
})

// Stub sidecar fetch so SanitizerAuditInspector's `useSanitizerRules`
// loadOnce() (fired the first time the overlay mounts) does not race
// the network. Returning an empty rules registry keeps the inspector
// rendering with the "no rule registry entry" fallback.
async function stubSidecarFetch(input: RequestInfo | URL): Promise<Response> {
  const url = typeof input === 'string' ? input : input.toString()
  if (url.includes('/sanitizer/rules')) {
    return new Response(
      JSON.stringify({ rules_version: '2026-04-20-1', rules: [] }),
      { status: 200, headers: { 'Content-Type': 'application/json' } }
    )
  }
  return new Response('not-mocked', { status: 404 })
}
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: stubSidecarFetch
  })
}))

const fakeRoute = reactive({
  params: {} as Record<string, string>,
  query: {} as Record<string, string>
})
vi.mock('vue-router', () => ({
  useRoute: () => fakeRoute,
  useRouter: () => ({ push: vi.fn() })
}))

beforeEach(() => {
  invokeMock.mockClear()
  fakeRoute.params = {}
  fakeRoute.query = {}
})

import ExplorerPage from '~/pages/explorer/[task_id].vue'
import StationPage from '~/pages/tutorial/[workspace_id]/[station_id].vue'

async function flush(): Promise<void> {
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
  await new Promise((r) => setTimeout(r, 0))
  await nextTick()
}

describe('sanitize-overlay integration: Explorer page', () => {
  it('sanitize tab row click opens SanitizerAuditInspector and not LlmCallInspector', async () => {
    fakeRoute.params = { task_id: 'explore_4f2a8b91' }
    fakeRoute.query = { ws_path: '/abs/ws' }

    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await flush()

    // Switch to sanitize tab and click first row.
    await wrapper.find('button[data-tab="sanitize"]').trigger('click')
    await flush()
    const rows = wrapper.findAll('[data-testid="audit-row"]')
    expect(rows.length).toBeGreaterThan(0)
    await rows[0]!.trigger('click')
    await flush()

    // SanitizerAuditInspector mounted (data-component identifies it).
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(true)
    // LlmCallInspector aside is not active — its 4-tab strip should be absent.
    expect(wrapper.findAll('button[data-tab="wire"]').length).toBe(0)
    wrapper.unmount()
  })

  it('llm tab row click opens LlmCallInspector and SanitizerAuditInspector closes', async () => {
    fakeRoute.params = { task_id: 'explore_4f2a8b91' }
    fakeRoute.query = { ws_path: '/abs/ws' }

    const wrapper = mount(ExplorerPage, { attachTo: document.body })
    await flush()

    // First open sanitize inspector.
    await wrapper.find('button[data-tab="sanitize"]').trigger('click')
    await flush()
    await wrapper.findAll('[data-testid="audit-row"]')[0]!.trigger('click')
    await flush()
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(true)

    // Switch to llm tab — sanitize inspector closes.
    await wrapper.find('button[data-tab="llm"]').trigger('click')
    await flush()
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(false)

    const llmRows = wrapper.findAll('[data-testid="audit-row"]')
    expect(llmRows.length).toBeGreaterThan(0)
    await llmRows[0]!.trigger('click')
    await flush()
    // LlmCallInspector renders its 4-tab strip when active.
    expect(wrapper.findAll('button[data-tab="wire"]').length).toBe(1)
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(false)
    wrapper.unmount()
  })
})

describe('sanitize-overlay integration: Station page', () => {
  it('sanitize tab row click opens SanitizerAuditInspector at page root', async () => {
    fakeRoute.params = {
      workspace_id: 'ws_demo',
      station_id: 's01-overview'
    }
    fakeRoute.query = { ws_path: '/abs/ws' }

    const wrapper = mount(StationPage, { attachTo: document.body })
    await flush()

    // Sanitize is the default audit tab on the station page.
    const rows = wrapper.findAll('[data-testid="audit-row"]')
    expect(rows.length).toBeGreaterThan(0)
    await rows[0]!.trigger('click')
    await flush()

    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(true)
    // LlmCallInspector aside isn't open — its 4-tab strip absent.
    expect(wrapper.findAll('button[data-tab="wire"]').length).toBe(0)
    wrapper.unmount()
  })

  it('llm tab row click opens LlmCallInspector and SanitizerAuditInspector closes', async () => {
    fakeRoute.params = {
      workspace_id: 'ws_demo',
      station_id: 's01-overview'
    }
    fakeRoute.query = { ws_path: '/abs/ws' }

    const wrapper = mount(StationPage, { attachTo: document.body })
    await flush()

    // Open sanitize inspector first.
    await wrapper.findAll('[data-testid="audit-row"]')[0]!.trigger('click')
    await flush()
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(true)

    await wrapper.find('button[data-tab="llm"]').trigger('click')
    await flush()
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(false)

    const llmRows = wrapper.findAll('[data-testid="audit-row"]')
    expect(llmRows.length).toBeGreaterThan(0)
    await llmRows[0]!.trigger('click')
    await flush()
    expect(wrapper.findAll('button[data-tab="wire"]').length).toBe(1)
    expect(
      wrapper.find('[data-component="SanitizerAuditInspector"]').exists()
    ).toBe(false)
    wrapper.unmount()
  })
})
