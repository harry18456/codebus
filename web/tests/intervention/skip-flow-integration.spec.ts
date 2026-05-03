import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const readSpy = vi.fn()
const writeSpy = vi.fn()
vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: readSpy,
    writeProgressFile: writeSpy,
    listTutorialTasks: vi.fn()
  })
}))

import {
  useIntervention,
  _resetUseInterventionForTest as resetIntervention
} from '~/composables/useIntervention'
import { useTutorialProgress } from '~/composables/useTutorialProgress'
import type { RouteJson } from '~/composables/useStationRoute'

const WS = 'D:/projects/demo'
const TID = 'generate_abcd1234'

function fakeRoute(stationIds: string[]): RouteJson {
  return {
    schema_version: 1,
    task_id: TID,
    workspace_root: WS,
    workspace_type: 'folder',
    repo_name: 'demo',
    title: 'demo',
    duration_total_minutes: stationIds.length * 5,
    stations: stationIds.map((id, idx) => ({
      station_id: id,
      index: idx + 1,
      title: `Station ${idx}`,
      slug: id.split('-').slice(1).join('-'),
      duration: 5,
      file_path: `stations/${id}.md`,
      stable_id: id,
      related_files: [],
      related_stations: [],
      required_checks: [`${id}-check-1`],
      degraded: false
    }))
  } as unknown as RouteJson
}

beforeEach(() => {
  readSpy.mockReset()
  writeSpy.mockReset()
  writeSpy.mockResolvedValue(undefined)
  resetIntervention()
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe('skip flow integration: requestSkip → confirm → progress + nav', () => {
  it('confirm of mid-route skip writes progress.skipped_station_ids and navigates to next station', async () => {
    readSpy.mockRejectedValueOnce(new Error('ENOENT'))
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    progress.setRoute(route)
    progress.setCurrentStation('s02-mqtt-client')

    const navTo = vi.fn()
    const intervention = useIntervention()
    intervention.requestSkip({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      onConfirm: () => {
        progress.markStationSkipped('s02-mqtt-client')
        // Page-level: navigate to next station
        const idx = route.stations.findIndex(
          (s) => s.station_id === 's02-mqtt-client'
        )
        const next = route.stations[idx + 1]
        if (next) navTo(`/tutorial/ws_xxx/${next.station_id}`)
        else navTo('/tutorial/ws_xxx')
      }
    })
    await intervention.confirm()
    await vi.runAllTimersAsync()

    expect(progress.state.value.skipped_station_ids).toContain('s02-mqtt-client')
    expect(progress.state.value.current_station_id).toBeNull()
    expect(navTo).toHaveBeenCalledWith('/tutorial/ws_xxx/s03-storage')
    // Persisted to disk
    expect(writeSpy).toHaveBeenCalled()
    const persisted = JSON.parse(
      writeSpy.mock.calls.at(-1)![2] as string
    ) as { skipped_station_ids: string[] }
    expect(persisted.skipped_station_ids).toContain('s02-mqtt-client')
  })

  it('confirm of last-station skip navigates back to MOC', async () => {
    readSpy.mockRejectedValueOnce(new Error('ENOENT'))
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    progress.setRoute(route)
    progress.setCurrentStation('s03-storage')

    const navTo = vi.fn()
    const intervention = useIntervention()
    intervention.requestSkip({
      stationId: 's03-storage',
      stationTitle: 'Storage',
      onConfirm: () => {
        progress.markStationSkipped('s03-storage')
        const idx = route.stations.findIndex(
          (s) => s.station_id === 's03-storage'
        )
        const next = route.stations[idx + 1]
        if (next) navTo(`/tutorial/ws_xxx/${next.station_id}`)
        else navTo('/tutorial/ws_xxx')
      }
    })
    await intervention.confirm()
    await vi.runAllTimersAsync()

    expect(progress.state.value.skipped_station_ids).toContain('s03-storage')
    expect(navTo).toHaveBeenCalledWith('/tutorial/ws_xxx')
  })
})
