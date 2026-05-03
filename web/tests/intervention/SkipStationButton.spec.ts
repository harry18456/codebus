import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

const readSpy = vi.fn()
const writeSpy = vi.fn()
vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: readSpy,
    writeProgressFile: writeSpy,
    listTutorialTasks: vi.fn()
  })
}))

import SkipStationButton from '~/components/intervention/SkipStationButton.vue'
import { useIntervention, _resetUseInterventionForTest } from '~/composables/useIntervention'
import { useTutorialProgress } from '~/composables/useTutorialProgress'

const WS = 'D:/projects/demo'
const TID = 'generate_abcd1234'

beforeEach(() => {
  readSpy.mockReset()
  writeSpy.mockReset()
  writeSpy.mockResolvedValue(undefined)
  _resetUseInterventionForTest()
})

afterEach(() => {
  _resetUseInterventionForTest()
})

async function loadProgressFromJson(json: object): Promise<void> {
  readSpy.mockResolvedValueOnce(JSON.stringify(json))
  await useTutorialProgress().loadProgress(WS, TID)
}

describe('SkipStationButton render & interaction', () => {
  it('does not render when station is already in completed_station_ids', async () => {
    await loadProgressFromJson({
      current_station_id: 's02',
      completed_station_ids: ['s02-mqtt-client'],
      skipped_station_ids: [],
      checkpoints: {},
      quizzes: {}
    })
    const wrapper = mount(SkipStationButton, {
      props: {
        stationId: 's02-mqtt-client',
        stationTitle: 'MQTT Client'
      }
    })
    await nextTick()
    expect(wrapper.find('[data-testid="skip-station-button"]').exists()).toBe(false)
  })

  it('renders as inert with tooltip when station is in skipped_station_ids', async () => {
    await loadProgressFromJson({
      current_station_id: 's02-mqtt-client',
      completed_station_ids: [],
      skipped_station_ids: ['s02-mqtt-client'],
      checkpoints: {},
      quizzes: {}
    })
    const wrapper = mount(SkipStationButton, {
      props: {
        stationId: 's02-mqtt-client',
        stationTitle: 'MQTT Client'
      }
    })
    await nextTick()
    const btn = wrapper.find('[data-testid="skip-station-button"]')
    expect(btn.exists()).toBe(true)
    expect(btn.attributes('title') ?? '').toContain('本站已跳過')
    // Click is no-op: no pendingAction set
    await btn.trigger('click')
    await nextTick()
    expect(useIntervention().pendingAction.value).toBeNull()
  })

  it('never-visited station click sets pendingAction kind=skip with payload', async () => {
    await loadProgressFromJson({
      current_station_id: 's02-mqtt-client',
      completed_station_ids: [],
      skipped_station_ids: [],
      checkpoints: {},
      quizzes: {}
    })
    const wrapper = mount(SkipStationButton, {
      props: {
        stationId: 's02-mqtt-client',
        stationTitle: 'MQTT Client'
      }
    })
    await nextTick()
    await wrapper.find('[data-testid="skip-station-button"]').trigger('click')
    await nextTick()
    const action = useIntervention().pendingAction.value
    expect(action).not.toBeNull()
    expect(action?.kind).toBe('skip')
    expect(action?.payload).toMatchObject({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client'
    })
  })
})
