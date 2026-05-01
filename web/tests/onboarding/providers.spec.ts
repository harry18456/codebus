// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-onboarding/spec.md
//   Scenario: Providers page next disabled until both forms valid
//   Scenario: Chat keyring failure aborts before embedding
//   Scenario: Successful submission routes to done

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick } from 'vue'

const pushSpy = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: pushSpy })
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

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args)
}))

import Providers from '~/pages/onboarding/providers.vue'
import { _resetForTest } from '~/composables/useProviderConfig'

beforeEach(() => {
  pushSpy.mockClear()
  fetchMock.mockReset()
  invokeMock.mockReset()
  _resetForTest()
})

async function fillBoth(wrapper: ReturnType<typeof mount>): Promise<void> {
  await wrapper.get('[data-testid="onboarding-chat-id"]').setValue('openai-default')
  await wrapper.get('[data-testid="onboarding-chat-model"]').setValue('gpt-4o-mini')
  await wrapper
    .get('[data-testid="onboarding-chat-base-url"]')
    .setValue('https://api.openai.com/v1')
  await wrapper
    .get('[data-testid="onboarding-chat-api-key"]')
    .setValue('sk-chat-key')
  await wrapper
    .get('[data-testid="onboarding-embed-id"]')
    .setValue('openai-embed-3')
  await wrapper
    .get('[data-testid="onboarding-embed-model"]')
    .setValue('text-embedding-3-small')
  await wrapper
    .get('[data-testid="onboarding-embed-base-url"]')
    .setValue('https://api.openai.com/v1')
  await wrapper
    .get('[data-testid="onboarding-embed-api-key"]')
    .setValue('sk-embed-key')
}

describe('/onboarding/providers', () => {
  it('renders both forms with four fields each', () => {
    const wrapper = mount(Providers)
    expect(wrapper.find('[data-testid="onboarding-form-chat"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onboarding-form-embed"]').exists()).toBe(
      true
    )
    expect(wrapper.find('[data-testid="onboarding-chat-id"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onboarding-chat-model"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onboarding-chat-base-url"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onboarding-chat-api-key"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="onboarding-embed-id"]').exists()).toBe(true)
  })

  it('Next disabled with chat-only filled, enabled when both filled', async () => {
    const wrapper = mount(Providers)
    const next = wrapper.get('[data-testid="onboarding-providers-next"]')

    await wrapper.get('[data-testid="onboarding-chat-api-key"]').setValue('sk')
    expect((next.element as HTMLButtonElement).disabled).toBe(true)

    await fillBoth(wrapper)
    expect((next.element as HTMLButtonElement).disabled).toBe(false)
  })

  it('chat keyring failure aborts before embedding keyring_set', async () => {
    invokeMock.mockResolvedValueOnce({ ok: false, code: 'KEYRING_BACKEND_ERROR' })
    const wrapper = mount(Providers)
    await fillBoth(wrapper)
    await wrapper.get('[data-testid="onboarding-providers-next"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))

    // Only ONE invoke call (the chat keyring) — embedding aborted.
    expect(invokeMock).toHaveBeenCalledTimes(1)
    expect(invokeMock.mock.calls[0]![0]).toBe('keyring_set')
    expect(
      wrapper.find('[data-testid="onboarding-providers-error"]').exists()
    ).toBe(true)
    expect(pushSpy).not.toHaveBeenCalled()
  })

  it('successful submission orders keyring → upsert × 2 → setBinding × 4 → route done', async () => {
    invokeMock
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce({ ok: true })
    fetchMock.mockResolvedValue(new Response('', { status: 204 }))

    const wrapper = mount(Providers)
    await fillBoth(wrapper)
    await wrapper.get('[data-testid="onboarding-providers-next"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 30))

    // 2 keyring_set invokes
    expect(invokeMock).toHaveBeenCalledTimes(2)
    expect(invokeMock.mock.calls[0]![0]).toBe('keyring_set')
    expect(invokeMock.mock.calls[1]![0]).toBe('keyring_set')
    expect(
      (invokeMock.mock.calls[0]![1] as Record<string, unknown>).providerId
    ).toBe('openai-default')
    expect(
      (invokeMock.mock.calls[1]![1] as Record<string, unknown>).providerId
    ).toBe('openai-embed-3')

    // 2 upsertProvider POSTs + 4 setBinding PUTs
    const fetchCalls = fetchMock.mock.calls.map(([path, init]) => ({
      path,
      method: (init as RequestInit).method
    }))
    const posts = fetchCalls.filter(
      (c) => c.method === 'POST' && c.path === '/settings/providers'
    )
    const puts = fetchCalls.filter(
      (c) => c.method === 'PUT' && c.path === '/settings/bindings'
    )
    expect(posts.length).toBe(2)
    expect(puts.length).toBe(4)
    expect(pushSpy).toHaveBeenCalledWith('/onboarding/done')
  })
})
