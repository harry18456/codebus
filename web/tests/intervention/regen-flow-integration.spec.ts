import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ref, type Ref } from 'vue'

// Mock useSidecar for POST /generate
const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

// Mock useSseTask
const sseEventsRef: Ref<Array<{ type: string; data: unknown }>> = ref([])
const sseCloseSpy = vi.fn()
const { useSseTaskMock } = vi.hoisted(() => ({ useSseTaskMock: vi.fn() }))
vi.mock('~/composables/useSseTask', () => ({
  useSseTask: useSseTaskMock
}))
useSseTaskMock.mockImplementation(() => ({
  events: sseEventsRef,
  status: ref('open' as const),
  error: ref(null),
  close: sseCloseSpy
}))

// Mock useTutorialFiles for re-read after regen
const readSpy = vi.fn()
vi.mock('~/composables/useTutorialFiles', () => ({
  useTutorialFiles: () => ({
    readTutorialFile: readSpy,
    writeProgressFile: vi.fn(),
    listTutorialTasks: vi.fn()
  })
}))

import {
  useIntervention,
  _resetUseInterventionForTest as resetIntervention
} from '~/composables/useIntervention'

function jsonResponse(body: unknown, status = 202): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' }
  })
}

beforeEach(() => {
  fetchMock.mockReset()
  sseCloseSpy.mockReset()
  useSseTaskMock.mockClear()
  sseEventsRef.value = []
  readSpy.mockReset()
  resetIntervention()
})

afterEach(() => {
  resetIntervention()
})

describe('regen flow integration: requestRegen → confirm → POST /generate + SSE', () => {
  it('confirm of regen triggers POST /generate with target_stations and reads back markdown', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ task_id: 'generate_abcd1234' })
    )
    readSpy.mockResolvedValueOnce('# regenerated body')

    const intervention = useIntervention()
    const onComplete = vi.fn()

    intervention.requestRegen({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      taskId: 'generate_abcd1234',
      workspaceRoot: 'D:/projects/demo',
      onConfirm: async () => {
        // Page-level wiring exercised by this test: POST /generate with
        // target_stations, attach SSE, on done re-read markdown.
        const sidecar = (await import('~/composables/useSidecar')).useSidecar()
        const res = await sidecar.fetch('/generate', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            workspace_root: 'D:/projects/demo',
            task: 'regen',
            stations: [],
            target_stations: ['s02-mqtt-client']
          })
        })
        const body = (await res.json()) as { task_id: string }
        const sse = (await import('~/composables/useSseTask')).useSseTask(
          body.task_id
        )
        // Wait for the SSE 'done' event then re-read markdown
        const stop = vi.fn()
        const cancel = setInterval(async () => {
          if (sse.events.value.some((e) => e.type === 'done')) {
            clearInterval(cancel)
            stop()
            const files = (
              await import('~/composables/useTutorialFiles')
            ).useTutorialFiles()
            await files.readTutorialFile(
              'D:/projects/demo',
              'codebus-tutorials/generate_abcd1234/stations/s02-mqtt-client.md'
            )
            onComplete()
          }
        }, 1)
      }
    })
    await intervention.confirm()

    // POST /generate sent with the right body
    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [path, init] = fetchMock.mock.calls[0]!
    expect(path).toBe('/generate')
    const reqBody = JSON.parse((init as { body: string }).body)
    expect(reqBody.target_stations).toEqual(['s02-mqtt-client'])

    // Simulate SSE done; the polling closure picks it up.
    sseEventsRef.value = [{ type: 'done', data: {} }]
    // Wait a couple of ticks for the setInterval poll to fire
    await new Promise((resolve) => setTimeout(resolve, 30))

    expect(useSseTaskMock).toHaveBeenCalledWith('generate_abcd1234')
    expect(readSpy).toHaveBeenCalledWith(
      'D:/projects/demo',
      'codebus-tutorials/generate_abcd1234/stations/s02-mqtt-client.md'
    )
    expect(onComplete).toHaveBeenCalled()
  })

  it('cancel of regen does NOT trigger POST /generate', async () => {
    const intervention = useIntervention()
    const fetchInOnConfirm = vi.fn()
    intervention.requestRegen({
      stationId: 's02-mqtt-client',
      stationTitle: 'MQTT Client',
      taskId: 'generate_abcd1234',
      workspaceRoot: 'D:/projects/demo',
      onConfirm: () => {
        fetchInOnConfirm()
      }
    })
    intervention.cancel()
    expect(fetchInOnConfirm).not.toHaveBeenCalled()
    expect(fetchMock).not.toHaveBeenCalled()
  })
})
