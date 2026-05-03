// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Scenario: Save without keyring write does not update config
//
// Confirm flow contract: keyring_set IPC FIRST, upsertProvider only on
// keyring success. The api_key MUST NOT cross the sidecar wire.

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick } from 'vue'

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

import ProviderEditModal from '~/components/settings/ProviderEditModal.vue'
import { _resetUseProviderConfigForTest } from '~/composables/useProviderConfig'

beforeEach(() => {
  fetchMock.mockReset()
  invokeMock.mockReset()
  _resetUseProviderConfigForTest()
})

async function fillValidForm(wrapper: ReturnType<typeof mount>) {
  await wrapper.get('[data-testid="provider-edit-id"]').setValue('openai-test')
  await wrapper.get('[data-testid="provider-edit-model"]').setValue('gpt-4o-mini')
  await wrapper
    .get('[data-testid="provider-edit-base-url"]')
    .setValue('https://api.openai.com/v1')
  await wrapper
    .get('[data-testid="provider-edit-api-key"]')
    .setValue('sk-test-key')
}

describe('<ProviderEditModal>', () => {
  it('renders four fields plus api_key reveal toggle', () => {
    const wrapper = mount(ProviderEditModal, { props: { open: true } })
    expect(wrapper.find('[data-testid="provider-edit-id"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="provider-edit-type"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="provider-edit-model"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="provider-edit-base-url"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="provider-edit-api-key"]').exists()).toBe(true)
    expect(
      wrapper.find('[data-testid="provider-edit-api-key-toggle"]').exists()
    ).toBe(true)
  })

  it('api key reveal toggle flips input type between password and text', async () => {
    const wrapper = mount(ProviderEditModal, { props: { open: true } })
    const input = wrapper.get('[data-testid="provider-edit-api-key"]')
    expect(input.attributes('type')).toBe('password')
    await wrapper
      .get('[data-testid="provider-edit-api-key-toggle"]')
      .trigger('click')
    expect(
      wrapper.get('[data-testid="provider-edit-api-key"]').attributes('type')
    ).toBe('text')
  })

  it('Confirm calls keyring_set FIRST, then upsertProvider via fetch', async () => {
    invokeMock.mockResolvedValueOnce({ ok: true })
    fetchMock.mockResolvedValueOnce(new Response('', { status: 204 }))

    const wrapper = mount(ProviderEditModal, { props: { open: true } })
    await fillValidForm(wrapper)
    await wrapper.get('[data-testid="provider-edit-confirm"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))

    expect(invokeMock).toHaveBeenCalledTimes(1)
    const [cmd, args] = invokeMock.mock.calls[0]!
    expect(cmd).toBe('keyring_set')
    expect((args as Record<string, unknown>).providerId).toBe('openai-test')
    expect((args as Record<string, unknown>).apiKey).toBe('sk-test-key')

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [path, init] = fetchMock.mock.calls[0]!
    expect(path).toBe('/settings/providers')
    expect((init as RequestInit).method).toBe('POST')
    const body = JSON.parse((init as RequestInit).body as string)
    expect('api_key' in body).toBe(false)
  })

  it('keyring failure aborts: upsertProvider must NOT be called', async () => {
    invokeMock.mockResolvedValueOnce({
      ok: false,
      code: 'KEYRING_BACKEND_ERROR'
    })

    const wrapper = mount(ProviderEditModal, { props: { open: true } })
    await fillValidForm(wrapper)
    await wrapper.get('[data-testid="provider-edit-confirm"]').trigger('click')
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))

    expect(fetchMock).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="provider-edit-error"]').exists()).toBe(
      true
    )
  })
})
