import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

import InterventionConfirmModal from '~/components/intervention/InterventionConfirmModal.vue'
import {
  useIntervention,
  _resetForTest
} from '~/composables/useIntervention'

beforeEach(() => {
  _resetForTest()
})

afterEach(() => {
  _resetForTest()
})

describe('InterventionConfirmModal render & interaction', () => {
  it('renders nothing when pendingAction is null', () => {
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    expect(useIntervention().pendingAction.value).toBeNull()
    // No dim layer / aside / dialog should be present.
    expect(wrapper.findAll('[data-testid="intervention-modal"]')).toHaveLength(0)
    expect(wrapper.findAll('[data-testid="intervention-modal-dim"]')).toHaveLength(0)
    wrapper.unmount()
  })

  it('skip kind renders skip-specific copy', async () => {
    const api = useIntervention()
    api.requestSkip({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      onConfirm: vi.fn()
    })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    expect(wrapper.find('[data-testid="intervention-modal"]').exists()).toBe(true)
    const text = wrapper.text()
    expect(text).toContain('跳過此站')
    wrapper.unmount()
  })

  it('regen kind renders regen-specific copy', async () => {
    const api = useIntervention()
    api.requestRegen({
      stationId: 's03-storage',
      stationTitle: 'Storage',
      taskId: 'generate_abc12345',
      workspaceRoot: 'D:/projects/demo',
      onConfirm: vi.fn()
    })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    expect(wrapper.find('[data-testid="intervention-modal"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('重生會覆蓋')
    wrapper.unmount()
  })

  it('switch kind renders switch-specific copy', async () => {
    const api = useIntervention()
    api.requestSwitchWorkspace({ onConfirm: vi.fn() })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    expect(wrapper.find('[data-testid="intervention-modal"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('進度按 workspace 路徑分開保存')
    wrapper.unmount()
  })

  it('confirm button click invokes useIntervention().confirm()', async () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSkip({
      stationId: 's02',
      stationTitle: 's2',
      onConfirm
    })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    await wrapper.find('[data-testid="intervention-confirm"]').trigger('click')
    await nextTick()
    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(api.pendingAction.value).toBeNull()
    wrapper.unmount()
  })

  it('cancel button click clears pendingAction without onConfirm', async () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSkip({
      stationId: 's02',
      stationTitle: 's2',
      onConfirm
    })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    await wrapper.find('[data-testid="intervention-cancel"]').trigger('click')
    await nextTick()
    expect(onConfirm).not.toHaveBeenCalled()
    expect(api.pendingAction.value).toBeNull()
    wrapper.unmount()
  })

  it('dim-layer click cancels (without invoking onConfirm)', async () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSwitchWorkspace({ onConfirm })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    await wrapper
      .find('[data-testid="intervention-modal-dim"]')
      .trigger('click')
    await nextTick()
    expect(onConfirm).not.toHaveBeenCalled()
    expect(api.pendingAction.value).toBeNull()
    wrapper.unmount()
  })

  it('aside click does NOT cancel (stopPropagation)', async () => {
    const api = useIntervention()
    const onConfirm = vi.fn()
    api.requestSwitchWorkspace({ onConfirm })
    const wrapper = mount(InterventionConfirmModal, { attachTo: document.body })
    await nextTick()
    await wrapper.find('[data-testid="intervention-modal"]').trigger('click')
    await nextTick()
    expect(onConfirm).not.toHaveBeenCalled()
    expect(api.pendingAction.value).not.toBeNull()
    wrapper.unmount()
  })
})
