// Backs SHALL clauses in
// openspec/changes/provider-settings-and-onboarding/specs/provider-settings/spec.md
//   Requirement: Embedding switch goes through destructive confirm modal
//
// The modal is dumb (props in / events out): the switch flow happens
// in the parent (`<RoleBindingTable>`). These tests verify shape.

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import EmbeddingChangeConfirmModal from '~/components/settings/EmbeddingChangeConfirmModal.vue'

describe('<EmbeddingChangeConfirmModal>', () => {
  it('renders chunk count + estimated rebuild duration', () => {
    const wrapper = mount(EmbeddingChangeConfirmModal, {
      props: {
        open: true,
        newProviderId: 'openai-embed-large',
        currentChunkCount: 1500
      }
    })
    expect(
      wrapper.get('[data-testid="embedding-confirm-chunks"]').text()
    ).toContain('1500')
    expect(
      wrapper.get('[data-testid="embedding-confirm-eta"]').text()
    ).toMatch(/min/)
  })

  it('Cancel emits cancel without confirm', async () => {
    const wrapper = mount(EmbeddingChangeConfirmModal, {
      props: {
        open: true,
        newProviderId: 'openai-embed-large',
        currentChunkCount: 1500
      }
    })
    await wrapper.get('[data-testid="embedding-confirm-cancel"]').trigger('click')
    expect(wrapper.emitted('cancel')).toBeTruthy()
    expect(wrapper.emitted('confirm')).toBeFalsy()
  })

  it('Confirm emits confirm', async () => {
    const wrapper = mount(EmbeddingChangeConfirmModal, {
      props: {
        open: true,
        newProviderId: 'openai-embed-large',
        currentChunkCount: 1500
      }
    })
    await wrapper.get('[data-testid="embedding-confirm-confirm"]').trigger('click')
    expect(wrapper.emitted('confirm')).toBeTruthy()
  })

  it('hidden when open is false', () => {
    const wrapper = mount(EmbeddingChangeConfirmModal, {
      props: {
        open: false,
        newProviderId: null,
        currentChunkCount: 0
      }
    })
    expect(wrapper.find('[data-testid="embedding-confirm-modal"]').exists()).toBe(
      false
    )
  })
})
