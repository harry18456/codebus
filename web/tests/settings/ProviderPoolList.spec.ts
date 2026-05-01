// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Provider pool CRUD touches keyring and config (UI surface)

import { mount } from '@vue/test-utils'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref } from 'vue'

const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ ok: true })
}))

import ProviderPoolList from '~/components/settings/ProviderPoolList.vue'
import {
  _resetForTest,
  useProviderConfig
} from '~/composables/useProviderConfig'

beforeEach(() => {
  fetchMock.mockReset()
  _resetForTest()
})

function seed() {
  const config = useProviderConfig()
  config.providers.value = [
    {
      id: 'openai-default',
      type: 'openai_chat',
      model: 'gpt-4o-mini',
      base_url: 'https://api.openai.com/v1'
    },
    {
      id: 'openai-embed-3',
      type: 'openai_embedding',
      model: 'text-embedding-3-small',
      base_url: 'https://api.openai.com/v1'
    }
  ]
  config.bindings.value = {
    reasoning: 'openai-default',
    judge: 'openai-default',
    chat: 'openai-default',
    embed: 'openai-embed-3'
  }
}

describe('<ProviderPoolList>', () => {
  it('renders one row per provider with edit / delete buttons', () => {
    seed()
    const wrapper = mount(ProviderPoolList)
    const rows = wrapper.findAll('[data-testid="provider-pool-row"]')
    expect(rows.length).toBe(2)
    for (const row of rows) {
      expect(row.find('[data-testid="provider-pool-edit"]').exists()).toBe(true)
      expect(row.find('[data-testid="provider-pool-delete"]').exists()).toBe(true)
    }
  })

  it('Add provider button opens the edit modal', async () => {
    seed()
    const wrapper = mount(ProviderPoolList)
    expect(wrapper.find('[data-testid="provider-edit-modal"]').exists()).toBe(
      false
    )
    await wrapper.get('[data-testid="provider-pool-add"]').trigger('click')
    expect(wrapper.find('[data-testid="provider-edit-modal"]').exists()).toBe(
      true
    )
  })

  it('delete button on a bound provider shows the blocking message', async () => {
    seed()
    const wrapper = mount(ProviderPoolList)
    const rows = wrapper.findAll('[data-testid="provider-pool-row"]')
    const boundRow = rows[0]!
    expect(boundRow.attributes('data-provider-id')).toBe('openai-default')
    await boundRow.get('[data-testid="provider-pool-delete"]').trigger('click')
    expect(
      wrapper.find('[data-testid="provider-pool-delete-blocked"]').exists()
    ).toBe(true)
    // No DELETE call must have left for the sidecar.
    expect(fetchMock).not.toHaveBeenCalled()
  })
})
