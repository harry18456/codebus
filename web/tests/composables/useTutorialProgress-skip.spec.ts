import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Mock the Tauri IPC layer so writeProgressFile / readTutorialFile drive
// through controllable spies. The composable is the only writer of
// progress.json — defensive grep tests guard this — so the IPC layer is
// the right cut point.
const readSpy = vi.fn()
const writeSpy = vi.fn()

vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: readSpy,
    writeProgressFile: writeSpy,
    listTutorialTasks: vi.fn()
  })
}))

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
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

async function flushTimers(): Promise<void> {
  await vi.runAllTimersAsync()
}

describe('useTutorialProgress skipped_station_ids schema + transitions', () => {
  it('initial state has empty skipped_station_ids list', async () => {
    readSpy.mockRejectedValueOnce(new Error('ENOENT'))
    const api = useTutorialProgress()
    await api.loadProgress(WS, TID)
    expect(api.state.value.skipped_station_ids).toEqual([])
  })

  it('reading legacy progress.json without skipped_station_ids field reads as empty list', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: ['s01-overview'],
        // no skipped_station_ids field — legacy format
        checkpoints: {},
        quizzes: {}
      })
    )
    const api = useTutorialProgress()
    await api.loadProgress(WS, TID)
    expect(api.state.value.skipped_station_ids).toEqual([])
    // No console error / no thrown exception expected.
    expect(api.state.value.completed_station_ids).toEqual(['s01-overview'])
  })

  it('markStationSkipped appends id to skipped_station_ids and triggers write', async () => {
    readSpy.mockRejectedValueOnce(new Error('ENOENT'))
    const api = useTutorialProgress()
    await api.loadProgress(WS, TID)
    api.markStationSkipped('s02-mqtt-client')
    expect(api.state.value.skipped_station_ids).toContain('s02-mqtt-client')
    await flushTimers()
    expect(writeSpy).toHaveBeenCalled()
    const payload = JSON.parse(writeSpy.mock.calls.at(-1)![2] as string)
    expect(payload.skipped_station_ids).toContain('s02-mqtt-client')
  })

  it('markStationSkipped is idempotent (calling twice does not duplicate id)', async () => {
    readSpy.mockRejectedValueOnce(new Error('ENOENT'))
    const api = useTutorialProgress()
    await api.loadProgress(WS, TID)
    api.markStationSkipped('s02')
    api.markStationSkipped('s02')
    expect(api.state.value.skipped_station_ids).toEqual(['s02'])
  })

  it('completing a previously-skipped station moves id from skipped → completed atomically', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: [],
        skipped_station_ids: ['s02-mqtt-client'],
        checkpoints: {},
        quizzes: {}
      })
    )
    const api = useTutorialProgress()
    await api.loadProgress(WS, TID)
    expect(api.state.value.skipped_station_ids).toEqual(['s02-mqtt-client'])

    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    api.setRoute(route)
    // Have to also complete s01 since the unlock chain stops on first
    // unlocked-but-incomplete entry; the watch only promotes prefix
    // stations, but the mutual-exclusion rule applies on EVERY
    // completed_station_ids transition. Check that s02 going from
    // skipped to completed (via its required_check) DOES remove from
    // skipped in the same write.
    api.setCheckpoint('s01-overview-check-1', 0, true)
    api.setCheckpoint('s02-mqtt-client-check-1', 0, true)
    await flushTimers()
    expect(api.state.value.completed_station_ids).toContain('s02-mqtt-client')
    expect(api.state.value.skipped_station_ids).not.toContain('s02-mqtt-client')
    // Persisted payload must reflect the mutual exclusion atomically.
    const lastWrite = writeSpy.mock.calls.at(-1)![2] as string
    const payload = JSON.parse(lastWrite) as {
      completed_station_ids: string[]
      skipped_station_ids: string[]
    }
    expect(payload.completed_station_ids).toContain('s02-mqtt-client')
    expect(payload.skipped_station_ids).not.toContain('s02-mqtt-client')
  })
})
