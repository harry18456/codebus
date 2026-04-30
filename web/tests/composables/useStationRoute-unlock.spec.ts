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
})

afterEach(() => {
  vi.clearAllMocks()
})

describe('Unlock rules with skipped_station_ids', () => {
  it('Station skip unlocks the next station', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: [],
        skipped_station_ids: ['s01-overview'],
        checkpoints: {},
        quizzes: {}
      })
    )
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    const unlocked = progress.unlockedStationIds(route).value
    expect(unlocked.has('s01-overview')).toBe(true)
    expect(unlocked.has('s02-mqtt-client')).toBe(true)
    expect(unlocked.has('s03-storage')).toBe(false)
  })

  it('Skipped station revisitable via canVisitStation (URL paste)', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: [],
        skipped_station_ids: ['s01-overview'],
        checkpoints: {},
        quizzes: {}
      })
    )
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    // Skipped station MUST remain reachable (revisitability).
    expect(progress.canVisitStation('s01-overview', route).value).toBe(true)
    // Unlocked next station also reachable.
    expect(progress.canVisitStation('s02-mqtt-client', route).value).toBe(true)
    // Station 3 is still locked (not in completed nor skipped, and 2 not done).
    expect(progress.canVisitStation('s03-storage', route).value).toBe(false)
  })

  it('Locked station: URL paste denied when not in completed nor skipped', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: [],
        skipped_station_ids: [],
        checkpoints: {},
        quizzes: {}
      })
    )
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    // Only s01 is unlocked initially. s02 / s03 are locked, neither in
    // completed nor skipped → URL paste should be denied.
    expect(progress.canVisitStation('s02-mqtt-client', route).value).toBe(false)
    expect(progress.canVisitStation('s03-storage', route).value).toBe(false)
  })

  it('Mixed completed + skipped: union unlocks subsequent stations transitively', async () => {
    readSpy.mockResolvedValueOnce(
      JSON.stringify({
        current_station_id: null,
        completed_station_ids: ['s01-overview'],
        skipped_station_ids: ['s02-mqtt-client'],
        checkpoints: {
          's01-overview-check-1': { done: true, ts: '2026-01-01T00:00:00Z' }
        },
        quizzes: {}
      })
    )
    const progress = useTutorialProgress()
    await progress.loadProgress(WS, TID)
    const route = fakeRoute(['s01-overview', 's02-mqtt-client', 's03-storage'])
    progress.setRoute(route)
    const unlocked = progress.unlockedStationIds(route).value
    expect(unlocked.has('s03-storage')).toBe(true)
  })
})
