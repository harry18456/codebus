// Backs SHALL clauses in
// openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE
//   Requirement: Onramp state survives navigation away from entry page
//
// Pipeline shape (4 sidecar tasks behind 2 user clicks):
//   start(path)         → POST /scan?stream=true → on done → GET /tasks/<id>/result → POST /kb/build → on done → phase 'scan-complete'
//   triggerGenerate()   → POST /explore         → on done → GET /tasks/<id>/result → POST /generate → on done → phase 'ready'
//
// The mock useSseTask returns a fresh events ref on every call so each
// chain step gets its own SSE stream — mirrors the production behavior
// where a new useSseTask binds a new EventSource per task_id.

import { describe, expect, it, vi, beforeEach } from 'vitest'
import { ref, nextTick, type Ref } from 'vue'

const fetchMock = vi.fn()
vi.mock('~/composables/useSidecar', () => ({
  useSidecar: () => ({
    bearer: ref('test-bearer'),
    baseUrl: ref('http://127.0.0.1:9999'),
    ready: ref(true),
    fetch: (...args: unknown[]) => fetchMock(...args)
  })
}))

interface MockSse {
  events: Ref<Array<{ type: string; data: unknown }>>
  status: Ref<'open'>
  error: Ref<Error | null>
  close: ReturnType<typeof vi.fn>
}

const sseInstances: MockSse[] = []
const useSseTaskMock = vi.hoisted(() => vi.fn())

vi.mock('~/composables/useSseTask', () => ({
  useSseTask: useSseTaskMock
}))

useSseTaskMock.mockImplementation(() => {
  const inst: MockSse = {
    events: ref([]),
    status: ref('open'),
    error: ref(null),
    close: vi.fn()
  }
  sseInstances.push(inst)
  return inst
})

import {
  useWorkspaceOnramp,
  ONRAMP_DEFAULT_TASK,
  _resetForTest
} from '~/composables/useWorkspaceOnramp'

function jsonResponse(body: unknown, status = 202): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' }
  })
}

async function flush(): Promise<void> {
  for (let i = 0; i < 4; i += 1) {
    await new Promise((r) => setTimeout(r, 0))
    await nextTick()
  }
}

beforeEach(() => {
  fetchMock.mockReset()
  useSseTaskMock.mockClear()
  sseInstances.length = 0
  _resetForTest()
})

const FIXTURE_PATH = 'C:\\Users\\harry\\Code\\demo'
const FIXTURE_WS_ID = 'ws_b3e6cc56defb' // matches workspace-id parity fixture
const SCAN_TASK_ID = 'scan_a1b2c3d4'
const KB_TASK_ID = 'kb_e5f6a7b8'
const EXPLORE_TASK_ID = 'explore_11223344'
const GENERATE_TASK_ID = 'generate_55667788'

describe('useWorkspaceOnramp', () => {
  it('exports ONRAMP_DEFAULT_TASK constant equal to "認識整個 codebase"', () => {
    expect(ONRAMP_DEFAULT_TASK).toBe('認識整個 codebase')
  })

  it('exposes initial state phase=idle / workspaceId=null', () => {
    const onramp = useWorkspaceOnramp()
    expect(onramp.phase.value).toBe('idle')
    expect(onramp.workspaceId.value).toBeNull()
    expect(onramp.pickedPath.value).toBeNull()
    expect(onramp.errorMsg.value).toBeNull()
  })

  it('two callers receive the same singleton state (Object.is)', () => {
    const a = useWorkspaceOnramp()
    const b = useWorkspaceOnramp()
    expect(Object.is(a.phase, b.phase)).toBe(true)
    expect(Object.is(a.workspaceId, b.workspaceId)).toBe(true)
    expect(Object.is(a.pickedPath, b.pickedPath)).toBe(true)
  })

  it('start(path) derives workspaceId, POSTs /scan?stream=true, subscribes SSE → phase scanning', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()

    expect(onramp.workspaceId.value).toBe(FIXTURE_WS_ID)
    expect(onramp.pickedPath.value).toBe(FIXTURE_PATH)
    expect(onramp.phase.value).toBe('scanning')

    const calls = fetchMock.mock.calls
    expect(calls.length).toBe(1)
    expect(calls[0]![0]).toBe('/scan?stream=true')
    const init = calls[0]![1] as RequestInit
    expect(init.method).toBe('POST')
    const body = JSON.parse(init.body as string)
    expect(body).toEqual({
      workspace_root: FIXTURE_PATH,
      workspace_type: 'folder'
    })
    expect(useSseTaskMock).toHaveBeenCalledWith(SCAN_TASK_ID)
  })

  it('scan SSE done chains GET /tasks/<id>/result then POST /kb/build → phase indexing', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID })) // POST /scan?stream=true
      .mockResolvedValueOnce(
        jsonResponse(
          {
            workspace_root: FIXTURE_PATH,
            scan_started_at: '2026-05-02T00:00:00Z',
            scan_completed_at: '2026-05-02T00:00:01Z',
            files: [],
            symlinks: [],
            content_summary: {
              total_files: 0,
              kind_counts: {},
              language_counts: {},
              category_counts: {},
              dominant_category: 'mixed',
              dominant_languages: [],
              has_tests: false,
              has_docs: false,
              is_monorepo: false
            },
            stats: {
              total_files_walked: 0,
              total_files_included: 0,
              total_bytes_read: 0,
              duration_seconds: 0.5,
              quarantined_count: 0,
              skipped_count: 0
            }
          },
          200
        )
      ) // GET /tasks/<scan_id>/result
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID })) // POST /kb/build

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()

    // Emit scan SSE done event
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()

    expect(fetchMock.mock.calls.length).toBe(3)
    const taskResultCall = fetchMock.mock.calls[1]!
    expect(taskResultCall[0]).toBe(`/tasks/${SCAN_TASK_ID}/result`)
    const kbCall = fetchMock.mock.calls[2]!
    expect(kbCall[0]).toBe('/kb/build')
    const kbBody = JSON.parse((kbCall[1] as RequestInit).body as string)
    expect(kbBody.workspace_root).toBe(FIXTURE_PATH)
    expect(kbBody.scan_result).toBeDefined()
    expect(kbBody.scan_result.workspace_root).toBe(FIXTURE_PATH)

    expect(useSseTaskMock).toHaveBeenLastCalledWith(KB_TASK_ID)
    expect(onramp.phase.value).toBe('indexing')
  })

  it('kb-build SSE done transitions to scan-complete without triggering explore', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ workspace_root: FIXTURE_PATH }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID }))

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()
    sseInstances[1]!.events.value.push({ type: 'done', data: {} })
    await flush()

    expect(onramp.phase.value).toBe('scan-complete')
    // Only 3 calls so far; no /explore was issued automatically.
    expect(fetchMock.mock.calls.length).toBe(3)
  })

  it('triggerGenerate posts /explore with ONRAMP_DEFAULT_TASK → phase exploring', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ workspace_root: FIXTURE_PATH }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ task_id: EXPLORE_TASK_ID }))

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()
    sseInstances[1]!.events.value.push({ type: 'done', data: {} })
    await flush()

    await onramp.triggerGenerate()
    await flush()

    const exploreCall = fetchMock.mock.calls[3]!
    expect(exploreCall[0]).toBe('/explore')
    const exploreBody = JSON.parse((exploreCall[1] as RequestInit).body as string)
    expect(exploreBody).toEqual({
      workspace_root: FIXTURE_PATH,
      task: ONRAMP_DEFAULT_TASK
    })
    expect(onramp.phase.value).toBe('exploring')
    expect(useSseTaskMock).toHaveBeenLastCalledWith(EXPLORE_TASK_ID)
  })

  it('explore SSE done chains GET result then POST /generate with stations → phase generating', async () => {
    const stationsFixture = [
      { id: 's01-overview', title: 'Overview', focus: 'top-level', summary: 'x' }
    ]
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ workspace_root: FIXTURE_PATH }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ task_id: EXPLORE_TASK_ID }))
      .mockResolvedValueOnce(
        jsonResponse(
          {
            task: ONRAMP_DEFAULT_TASK,
            stations: stationsFixture,
            budget_steps_left: 0,
            budget_tokens_left: 0
          },
          200
        )
      )
      .mockResolvedValueOnce(jsonResponse({ task_id: GENERATE_TASK_ID }))

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()
    sseInstances[1]!.events.value.push({ type: 'done', data: {} })
    await flush()
    await onramp.triggerGenerate()
    await flush()
    sseInstances[2]!.events.value.push({ type: 'done', data: {} })
    await flush()

    const resultCall = fetchMock.mock.calls[4]!
    expect(resultCall[0]).toBe(`/tasks/${EXPLORE_TASK_ID}/result`)

    const generateCall = fetchMock.mock.calls[5]!
    expect(generateCall[0]).toBe('/generate')
    const generateBody = JSON.parse((generateCall[1] as RequestInit).body as string)
    expect(generateBody.workspace_root).toBe(FIXTURE_PATH)
    expect(generateBody.task).toBe(ONRAMP_DEFAULT_TASK)
    expect(generateBody.stations).toEqual(stationsFixture)

    expect(useSseTaskMock).toHaveBeenLastCalledWith(GENERATE_TASK_ID)
    expect(onramp.phase.value).toBe('generating')
  })

  it('generate SSE done transitions to ready', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ workspace_root: FIXTURE_PATH }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ task_id: EXPLORE_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ task: 'x', stations: [] }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: GENERATE_TASK_ID }))

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()
    sseInstances[1]!.events.value.push({ type: 'done', data: {} })
    await flush()
    await onramp.triggerGenerate()
    await flush()
    sseInstances[2]!.events.value.push({ type: 'done', data: {} })
    await flush()
    sseInstances[3]!.events.value.push({ type: 'done', data: {} })
    await flush()

    expect(onramp.phase.value).toBe('ready')
  })

  it('SSE error during scan transitions to phase error with errorMsg', async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({
      type: 'error',
      data: { code: 'SCAN_FAILED', message: 'permission denied' }
    })
    await flush()

    expect(onramp.phase.value).toBe('error')
    expect(onramp.errorMsg.value).toContain('permission denied')
    // Path / id remain visible so user does not have to re-pick.
    expect(onramp.workspaceId.value).toBe(FIXTURE_WS_ID)
    expect(onramp.pickedPath.value).toBe(FIXTURE_PATH)
  })

  it('retry() re-issues the failed POST without rolling back to an earlier phase', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID })) // first /scan?stream=true
      .mockResolvedValueOnce(jsonResponse({ task_id: 'scan_99887766' })) // retry /scan?stream=true

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({
      type: 'error',
      data: { code: 'SCAN_FAILED', message: 'oops' }
    })
    await flush()
    expect(onramp.phase.value).toBe('error')

    await onramp.retry()
    await flush()

    expect(fetchMock.mock.calls.length).toBe(2)
    expect(fetchMock.mock.calls[1]![0]).toBe('/scan?stream=true')
    expect(onramp.phase.value).toBe('scanning')
    expect(onramp.errorMsg.value).toBeNull()
  })

  it('non-2xx response on /scan transitions phase error', async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse(
        { detail: { code: 'SCANNER_WORKSPACE_INVALID', message: 'no dir' } },
        400
      )
    )
    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    expect(onramp.phase.value).toBe('error')
    expect(onramp.errorMsg.value).toContain('no dir')
    // SSE was never bound for the failed POST.
    expect(useSseTaskMock).not.toHaveBeenCalled()
  })

  it('chain step closes the previous SSE before binding the next', async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ task_id: SCAN_TASK_ID }))
      .mockResolvedValueOnce(jsonResponse({ workspace_root: FIXTURE_PATH }, 200))
      .mockResolvedValueOnce(jsonResponse({ task_id: KB_TASK_ID }))

    const onramp = useWorkspaceOnramp()
    await onramp.start(FIXTURE_PATH)
    await flush()
    sseInstances[0]!.events.value.push({ type: 'done', data: {} })
    await flush()

    expect(sseInstances[0]!.close).toHaveBeenCalledTimes(1)
    expect(sseInstances.length).toBe(2)
  })
})
