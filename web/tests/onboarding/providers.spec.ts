// Backs SHALL clauses in
// openspec/changes/phase7-onboarding-polish/specs/provider-onboarding/spec.md
//   Scenario: Providers page next disabled until both forms valid
//   Scenario: Chat keyring failure aborts before embedding
//   Scenario: Successful submission routes to done
//   Scenario: Providers page renders contextual ToS link per type

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
import { _resetUseProviderConfigForTest } from '~/composables/useProviderConfig'

beforeEach(() => {
  pushSpy.mockClear()
  fetchMock.mockReset()
  invokeMock.mockReset()
  _resetUseProviderConfigForTest()
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

  it('renders contextual ToS link per type (default openai_chat / openai_embedding)', () => {
    const wrapper = mount(Providers)
    // Scenario: Providers page renders contextual ToS link per type
    // chat form default type = 'openai_chat' → OpenAI ToS URL
    const chatTos = wrapper.find('[data-testid="onboarding-chat-tos-link"]')
    expect(chatTos.exists()).toBe(true)
    expect(chatTos.attributes('href')).toBe(
      'https://openai.com/policies/terms-of-use/'
    )
    // embed form default type = 'openai_embedding' → OpenAI ToS URL
    const embedTos = wrapper.find('[data-testid="onboarding-embed-tos-link"]')
    expect(embedTos.exists()).toBe(true)
    expect(embedTos.attributes('href')).toBe(
      'https://openai.com/policies/terms-of-use/'
    )
  })

  it('successful submission orders keyring → upsert × 2 → setBinding × 4 → push → healthz ready → route done', async () => {
    invokeMock
      .mockResolvedValueOnce({ ok: true }) // keyring_set chat
      .mockResolvedValueOnce({ ok: true }) // keyring_set embed
      .mockResolvedValueOnce(undefined) // push_startup_config_cmd
    // First six fetches are 2 POSTs + 4 PUTs (settings mutations).
    // The seventh is the healthz verification — must report both LLM
    // lanes as `ready` for the wizard to advance.
    fetchMock
      .mockResolvedValueOnce(new Response('', { status: 204 })) // POST provider chat
      .mockResolvedValueOnce(new Response('', { status: 204 })) // POST provider embed
      .mockResolvedValueOnce(new Response('', { status: 204 })) // PUT reasoning
      .mockResolvedValueOnce(new Response('', { status: 204 })) // PUT judge
      .mockResolvedValueOnce(new Response('', { status: 204 })) // PUT chat
      .mockResolvedValueOnce(new Response('', { status: 204 })) // PUT embed
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            dependency: { llm_chat: 'ready', llm_embed: 'ready' }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      ) // GET healthz

    const wrapper = mount(Providers)
    await fillBoth(wrapper)
    await wrapper.get('[data-testid="onboarding-providers-next"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 30))

    // 2 keyring_set + 1 push_startup_config_cmd = 3 IPC invokes
    expect(invokeMock).toHaveBeenCalledTimes(3)
    expect(invokeMock.mock.calls[0]![0]).toBe('keyring_set')
    expect(invokeMock.mock.calls[1]![0]).toBe('keyring_set')
    expect(
      (invokeMock.mock.calls[0]![1] as Record<string, unknown>).providerId
    ).toBe('openai-default')
    expect(
      (invokeMock.mock.calls[1]![1] as Record<string, unknown>).providerId
    ).toBe('openai-embed-3')

    // 2 upsertProvider POSTs + 4 setBinding PUTs + 1 healthz GET = 7
    const fetchCalls = fetchMock.mock.calls.map(([path, init]) => ({
      path,
      method: (init as RequestInit | undefined)?.method ?? 'GET'
    }))
    const posts = fetchCalls.filter(
      (c) => c.method === 'POST' && c.path === '/settings/providers'
    )
    const puts = fetchCalls.filter(
      (c) => c.method === 'PUT' && c.path === '/settings/bindings'
    )
    expect(posts.length).toBe(2)
    expect(puts.length).toBe(4)
    expect(fetchCalls).toContainEqual({ path: '/healthz', method: 'GET' })

    expect(invokeMock.mock.calls[2]![0]).toBe('push_startup_config_cmd')
    expect(invokeMock.mock.calls[2]![1]).toEqual({
      providerIds: ['openai-default', 'openai-embed-3']
    })

    expect(pushSpy).toHaveBeenCalledWith('/onboarding/done')
  })

  it('push_startup_config_cmd failure stops the flow (does NOT route to done)', async () => {
    invokeMock
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce({ ok: true })
      .mockRejectedValueOnce(new Error('TRANSPORT_ERROR'))
    fetchMock.mockResolvedValue(new Response('', { status: 204 }))

    const wrapper = mount(Providers)
    await fillBoth(wrapper)
    await wrapper.get('[data-testid="onboarding-providers-next"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 30))

    // The done page MUST be a real success confirmation; push failure
    // means the keys never reached the sidecar so we stay on
    // /onboarding/providers and surface the error.
    expect(pushSpy).not.toHaveBeenCalledWith('/onboarding/done')
    expect(
      wrapper.find('[data-testid="onboarding-providers-error"]').exists()
    ).toBe(true)
  })

  it('healthz reporting not-configured stops the flow (does NOT route to done)', async () => {
    invokeMock
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce({ ok: true })
      .mockResolvedValueOnce(undefined)
    fetchMock
      .mockResolvedValueOnce(new Response('', { status: 204 })) // POST × 2
      .mockResolvedValueOnce(new Response('', { status: 204 }))
      .mockResolvedValueOnce(new Response('', { status: 204 })) // PUT × 4
      .mockResolvedValueOnce(new Response('', { status: 204 }))
      .mockResolvedValueOnce(new Response('', { status: 204 }))
      .mockResolvedValueOnce(new Response('', { status: 204 }))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            dependency: { llm_chat: 'not-configured', llm_embed: 'ready' }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      )

    const wrapper = mount(Providers)
    await fillBoth(wrapper)
    await wrapper.get('[data-testid="onboarding-providers-next"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 30))

    expect(pushSpy).not.toHaveBeenCalledWith('/onboarding/done')
    const err = wrapper.get('[data-testid="onboarding-providers-error"]')
    expect(err.text()).toContain('Sidecar 仍未就緒')
  })
})
