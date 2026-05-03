import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: vi.fn(),
    writeProgressFile: vi.fn(),
    listTutorialTasks: vi.fn()
  })
}))

import RegenStationButton from '~/components/intervention/RegenStationButton.vue'
import {
  useIntervention,
  _resetUseInterventionForTest as resetIntervention
} from '~/composables/useIntervention'

beforeEach(() => {
  resetIntervention()
})

afterEach(() => {
  resetIntervention()
})

describe('RegenStationButton render & interaction', () => {
  it('renders for a station regardless of degraded state (degraded=false)', () => {
    const wrapper = mount(RegenStationButton, {
      props: {
        stationId: 's02-mqtt-client',
        stationTitle: 'MQTT Client',
        taskId: 'generate_abcd1234',
        workspaceRoot: 'D:/projects/demo',
        degraded: false
      }
    })
    expect(wrapper.find('[data-testid="regen-station-button"]').exists()).toBe(true)
  })

  it('renders for a station regardless of degraded state (degraded=true)', () => {
    const wrapper = mount(RegenStationButton, {
      props: {
        stationId: 's02-mqtt-client',
        stationTitle: 'MQTT Client',
        taskId: 'generate_abcd1234',
        workspaceRoot: 'D:/projects/demo',
        degraded: true
      }
    })
    expect(wrapper.find('[data-testid="regen-station-button"]').exists()).toBe(true)
  })

  it('click sets pendingAction kind=regen with payload incl taskId + workspaceRoot', async () => {
    const wrapper = mount(RegenStationButton, {
      props: {
        stationId: 's03-storage',
        stationTitle: 'Storage',
        taskId: 'generate_abcd1234',
        workspaceRoot: 'D:/projects/demo'
      }
    })
    await wrapper.find('[data-testid="regen-station-button"]').trigger('click')
    await nextTick()
    const action = useIntervention().pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('regen')
    expect(action?.payload).toMatchObject({
      stationId: 's03-storage',
      stationTitle: 'Storage',
      taskId: 'generate_abcd1234',
      workspaceRoot: 'D:/projects/demo'
    })
  })
})
