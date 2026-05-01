// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Requirement: Index page redirects to onboarding when LLM dependencies are not configured

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

// AppShell mounts useSidecar etc. — replace with a stub since we
// only care about whether it renders or the redirect fires.
vi.mock('~/components/AppShell.vue', () => ({
  default: {
    name: 'AppShell',
    template: '<div data-testid="app-shell-stub" />'
  }
}))

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

describe('/ index page', () => {
  it('redirects to /onboarding/welcome when llm lane is not-configured', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        dependency: { llm_chat: 'not-configured', llm_embed: 'ready' }
      })
    )
    const wrapper = mount(IndexPage)
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))
    expect(replaceSpy).toHaveBeenCalledWith('/onboarding/welcome')
    expect(wrapper.find('[data-testid="app-shell-stub"]').exists()).toBe(false)
  })

  it('renders AppShell when both lanes are ready', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        status: 'ok',
        dependency: { llm_chat: 'ready', llm_embed: 'ready' }
      })
    )
    const wrapper = mount(IndexPage)
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))
    expect(replaceSpy).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="app-shell-stub"]').exists()).toBe(true)
  })
})
