// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Index page redirects to onboarding when LLM dependencies are not configured
// AND
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Entry page exposes folder-picker workspace onramp
//     Scenario: Entry page renders onramp UI when both LLM lanes are ready

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick } from 'vue'

const replaceSpy = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({ replace: replaceSpy, push: vi.fn() })
}))

const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

// Stub useWorkspaceOnramp so the entry page does not try to drive a
// real chain; we only care about whether the onramp UI renders here.
vi.mock('~/composables/useWorkspaceOnramp', async (importOriginal) => {
  const original = (await importOriginal()) as Record<string, unknown>
  const phase = ref<string>('idle')
  const workspaceId = ref<string | null>(null)
  const pickedPath = ref<string | null>(null)
  const progressEvents = ref<unknown[]>([])
  const errorMsg = ref<string | null>(null)
  const errorCode = ref<string | null>(null)
  const activeTaskId = ref<string | null>(null)
  return {
    ...original,
    useWorkspaceOnramp: () => ({
      phase,
      workspaceId,
      pickedPath,
      progressEvents,
      errorMsg,
      errorCode,
      activeTaskId,
      start: vi.fn(),
      triggerGenerate: vi.fn(),
      retry: vi.fn()
    })
  }
})

import IndexPage from '~/pages/index.vue'

beforeEach(() => {
  replaceSpy.mockClear()
  fetchMock.mockReset()
})

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' }
  })
}

async function settle(): Promise<void> {
  await nextTick()
  await new Promise((r) => setTimeout(r, 10))
  await nextTick()
}

describe('/ index page', () => {
  it('redirects to /onboarding/welcome when llm_chat is not-configured', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        dependency: { llm_chat: 'not-configured', llm_embed: 'ready' }
      })
    )
    const wrapper = mount(IndexPage)
    await settle()
    expect(replaceSpy).toHaveBeenCalledWith('/onboarding/welcome')
    expect(wrapper.find('[data-testid="onramp-folder-picker"]').exists()).toBe(false)
  })

  it('redirects to /onboarding/welcome when llm_embed is not-configured', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        dependency: { llm_chat: 'ready', llm_embed: 'not-configured' }
      })
    )
    const wrapper = mount(IndexPage)
    await settle()
    expect(replaceSpy).toHaveBeenCalledWith('/onboarding/welcome')
    expect(wrapper.find('[data-testid="onramp-folder-picker"]').exists()).toBe(false)
  })

  it('renders onramp surface (folder picker + onramp card) when both lanes are ready', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        dependency: { llm_chat: 'ready', llm_embed: 'ready' }
      })
    )
    const wrapper = mount(IndexPage)
    await settle()
    expect(replaceSpy).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="onramp-folder-picker"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="workspace-onramp-card"]').exists()).toBe(true)
  })
})
