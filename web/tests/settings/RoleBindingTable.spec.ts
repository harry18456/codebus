// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Role binding change propagates via hot-swap
//   Requirement: Embedding switch goes through destructive confirm modal

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

import RoleBindingTable from '~/components/settings/RoleBindingTable.vue'
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
      id: 'anthropic-claude',
      type: 'openai_chat',
      model: 'claude-haiku',
      base_url: 'https://api.anthropic.com/v1'
    },
    {
      id: 'openai-embed-3',
      type: 'openai_embedding',
      model: 'text-embedding-3-small',
      base_url: 'https://api.openai.com/v1'
    },
    {
      id: 'openai-embed-large',
      type: 'openai_embedding',
      model: 'text-embedding-3-large',
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

describe('<RoleBindingTable>', () => {
  it('renders four rows, one per role', () => {
    seed()
    const wrapper = mount(RoleBindingTable)
    const rows = wrapper.findAll('[data-testid="role-binding-row"]')
    expect(rows.length).toBe(4)
    expect(rows.map((r) => r.attributes('data-role'))).toEqual([
      'reasoning',
      'judge',
      'chat',
      'embed'
    ])
  })

  it('embed dropdown lists only embedding-typed providers', () => {
    seed()
    const wrapper = mount(RoleBindingTable)
    const embedSelect = wrapper.get('[data-testid="role-binding-select-embed"]')
    const optionValues = embedSelect
      .findAll('option')
      .map((o) => o.attributes('value'))
      .filter((v) => v && v !== '')
    expect(new Set(optionValues)).toEqual(
      new Set(['openai-embed-3', 'openai-embed-large'])
    )
  })

  it('non-embed change applies immediately via setBinding (no confirm modal)', async () => {
    seed()
    fetchMock.mockResolvedValueOnce(new Response('', { status: 204 }))
    const wrapper = mount(RoleBindingTable)
    const select = wrapper.get('[data-testid="role-binding-select-reasoning"]')
    await select.setValue('anthropic-claude')
    await nextTick()
    await new Promise((r) => setTimeout(r, 10))

    expect(
      wrapper.find('[data-testid="embedding-confirm-modal"]').exists()
    ).toBe(false)
    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [path, init] = fetchMock.mock.calls[0]!
    expect(path).toBe('/settings/bindings')
    expect((init as RequestInit).method).toBe('PUT')
    const body = JSON.parse((init as RequestInit).body as string)
    expect(body).toEqual({ reasoning: 'anthropic-claude' })
  })

  it('embed change opens the destructive confirm modal first', async () => {
    seed()
    const wrapper = mount(RoleBindingTable)
    const select = wrapper.get('[data-testid="role-binding-select-embed"]')
    await select.setValue('openai-embed-large')
    await nextTick()

    expect(
      wrapper.find('[data-testid="embedding-confirm-modal"]').exists()
    ).toBe(true)
    // Binding must NOT have flipped yet.
    expect(fetchMock).not.toHaveBeenCalled()
  })
})
