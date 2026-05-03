import { ref, watch, type Ref } from 'vue'
import { useSidecar } from './useSidecar'
import { useSseTask, type SseEvent, type SseTaskApi } from './useSseTask'
import { deriveWorkspaceId } from '~/utils/workspace-id'

// useWorkspaceOnramp — module-level singleton driving the entry-page
// folder-pick → 4-step sidecar pipeline → /tutorial navigation.
//
// Spec: openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//   Requirement: Workspace onramp drives scan, kb-build, explore, then generate via SSE
//   Requirement: Onramp state survives navigation away from entry page
//
// Per design Decision 5: pipeline is 4 sidecar tasks (scan → kb/build →
// explore → generate) chained behind 2 user clicks (pick + "+ 產生
// tutorial"). Per design Decision 6: explore + generate take a single
// fixed `task` string `ONRAMP_DEFAULT_TASK`.
//
// State lives at module scope so navigating to /settings or /audit/*
// and back to / does NOT tear down the in-flight SSE subscription —
// matches the singleton pattern from useQaSession / useIntervention.

export type OnrampPhase =
  | 'idle'
  | 'scanning'
  | 'indexing'
  | 'scan-complete'
  | 'exploring'
  | 'generating'
  | 'ready'
  | 'error'

export type OnrampAction = 'scan' | 'kb-build' | 'explore' | 'generate'

export const ONRAMP_DEFAULT_TASK = '認識整個 codebase'

export interface UseWorkspaceOnrampApi {
  phase: Ref<OnrampPhase>
  workspaceId: Ref<string | null>
  pickedPath: Ref<string | null>
  progressEvents: Ref<SseEvent[]>
  errorMsg: Ref<string | null>
  errorCode: Ref<string | null>
  activeTaskId: Ref<string | null>
  start: (absolutePath: string) => Promise<void>
  triggerGenerate: () => Promise<void>
  retry: () => Promise<void>
}

const _state = {
  phase: ref<OnrampPhase>('idle'),
  workspaceId: ref<string | null>(null),
  pickedPath: ref<string | null>(null),
  progressEvents: ref<SseEvent[]>([]),
  errorMsg: ref<string | null>(null),
  errorCode: ref<string | null>(null),
  activeTaskId: ref<string | null>(null)
}

// Intermediate results captured between chain steps.
let _scanResult: unknown = null
let _explorerState: { stations?: unknown[] } | null = null
// `lastFailedAction` lets retry() re-issue the POST that owned the
// failed task. Spec scenario "SSE error pauses onramp with retry
// affordance" mandates this — retry MUST NOT roll back to an earlier
// phase, just re-issue the same POST.
let _lastFailedAction: OnrampAction | null = null

// Active SSE binding. Mirrors `useQaSession::disposeSse` pattern —
// chain transitions tear down the previous binding before starting the
// next so two EventSources never coexist.
let _sse: SseTaskApi | null = null
let _sseStopWatch: (() => void) | null = null
let _sseCursor = 0

interface ChainCallbacks {
  onDone: () => void | Promise<void>
  onError: (code: string, message: string) => void
}

function disposeSse(): void {
  if (_sseStopWatch !== null) {
    _sseStopWatch()
    _sseStopWatch = null
  }
  if (_sse !== null) {
    _sse.close()
    _sse = null
  }
  _sseCursor = 0
}

function bindSseTask(taskId: string, callbacks: ChainCallbacks): void {
  disposeSse()
  _sse = useSseTask(taskId)
  _sseCursor = 0
  _state.progressEvents.value = []

  _sseStopWatch = watch(
    () => _sse?.events.value.length ?? 0,
    (len) => {
      while (_sseCursor < len) {
        const ev = _sse?.events.value[_sseCursor]
        _sseCursor += 1
        if (!ev) continue
        // Forward every event into the public progressEvents stream so
        // <OnrampProgress> can render counters / phases.
        _state.progressEvents.value.push(ev)
        if (ev.type === 'error') {
          const data = ev.data as { code?: string; message?: string } | undefined
          const code =
            typeof data?.code === 'string' ? data.code : 'STREAM_ERROR'
          const message =
            typeof data?.message === 'string'
              ? data.message
              : 'unknown SSE error'
          callbacks.onError(code, message)
          return
        }
        if (ev.type === 'done') {
          // Schedule onDone outside the cursor loop so a chained
          // bindSseTask call doesn't immediately re-fire the watcher
          // through a recursive `dispatch -> bind -> watch immediate`
          // path. (Kicking via Promise.resolve() defers to the next
          // microtask.)
          void Promise.resolve().then(() => callbacks.onDone())
          return
        }
      }
    },
    { immediate: true }
  )
}

interface PostJsonOk<T> {
  ok: true
  body: T
}
interface PostJsonErr {
  ok: false
  code: string
  message: string
}
type PostJsonResult<T> = PostJsonOk<T> | PostJsonErr

async function postJson<T>(
  url: string,
  body: unknown
): Promise<PostJsonResult<T>> {
  const sidecar = useSidecar()
  let res: Response
  try {
    res = await sidecar.fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
  } catch (err) {
    return {
      ok: false,
      code: 'NETWORK_ERROR',
      message: err instanceof Error ? err.message : String(err)
    }
  }
  if (!res.ok) {
    let detail: { code?: string; message?: string } | undefined
    try {
      const payload = (await res.json()) as { detail?: typeof detail }
      detail = payload?.detail
    } catch {
      detail = undefined
    }
    return {
      ok: false,
      code: detail?.code ?? `HTTP_${res.status}`,
      message:
        detail?.message ?? `${url} failed with status ${res.status}`
    }
  }
  return { ok: true, body: (await res.json()) as T }
}

async function getJson<T>(url: string): Promise<PostJsonResult<T>> {
  const sidecar = useSidecar()
  let res: Response
  try {
    res = await sidecar.fetch(url)
  } catch (err) {
    return {
      ok: false,
      code: 'NETWORK_ERROR',
      message: err instanceof Error ? err.message : String(err)
    }
  }
  if (!res.ok) {
    return {
      ok: false,
      code: `HTTP_${res.status}`,
      message: `${url} failed with status ${res.status}`
    }
  }
  return { ok: true, body: (await res.json()) as T }
}

function setError(action: OnrampAction, code: string, message: string): void {
  _lastFailedAction = action
  _state.phase.value = 'error'
  _state.errorCode.value = code
  _state.errorMsg.value = message
  disposeSse()
}

function clearError(): void {
  _state.errorMsg.value = null
  _state.errorCode.value = null
}

async function _runScan(path: string): Promise<void> {
  _state.phase.value = 'scanning'
  const res = await postJson<{ task_id: string }>('/scan?stream=true', {
    workspace_root: path,
    workspace_type: 'folder'
  })
  if (!res.ok) {
    setError('scan', res.code, res.message)
    return
  }
  _state.activeTaskId.value = res.body.task_id
  bindSseTask(res.body.task_id, {
    onDone: () => _onScanDone(res.body.task_id),
    onError: (code, message) => setError('scan', code, message)
  })
}

async function _onScanDone(scanTaskId: string): Promise<void> {
  const resultRes = await getJson<unknown>(`/tasks/${scanTaskId}/result`)
  if (!resultRes.ok) {
    setError('scan', resultRes.code, resultRes.message)
    return
  }
  _scanResult = resultRes.body
  await _runKbBuild()
}

async function _runKbBuild(): Promise<void> {
  if (_state.pickedPath.value === null || _scanResult === null) {
    setError('kb-build', 'BAD_STATE', 'kb-build started without scan result')
    return
  }
  _state.phase.value = 'indexing'
  const res = await postJson<{ task_id: string }>('/kb/build', {
    workspace_root: _state.pickedPath.value,
    scan_result: _scanResult
  })
  if (!res.ok) {
    setError('kb-build', res.code, res.message)
    return
  }
  _state.activeTaskId.value = res.body.task_id
  bindSseTask(res.body.task_id, {
    onDone: () => _onKbDone(),
    onError: (code, message) => setError('kb-build', code, message)
  })
}

function _onKbDone(): void {
  _state.phase.value = 'scan-complete'
  _state.activeTaskId.value = null
  disposeSse()
}

async function _runExplore(): Promise<void> {
  if (_state.pickedPath.value === null) {
    setError('explore', 'BAD_STATE', 'explore triggered without picked path')
    return
  }
  _state.phase.value = 'exploring'
  const res = await postJson<{ task_id: string }>('/explore', {
    workspace_root: _state.pickedPath.value,
    task: ONRAMP_DEFAULT_TASK
  })
  if (!res.ok) {
    setError('explore', res.code, res.message)
    return
  }
  _state.activeTaskId.value = res.body.task_id
  bindSseTask(res.body.task_id, {
    onDone: () => _onExploreDone(res.body.task_id),
    onError: (code, message) => setError('explore', code, message)
  })
}

async function _onExploreDone(exploreTaskId: string): Promise<void> {
  const resultRes = await getJson<{ stations?: unknown[] }>(
    `/tasks/${exploreTaskId}/result`
  )
  if (!resultRes.ok) {
    setError('explore', resultRes.code, resultRes.message)
    return
  }
  _explorerState = resultRes.body
  await _runGenerate()
}

async function _runGenerate(): Promise<void> {
  if (_state.pickedPath.value === null || _explorerState === null) {
    setError(
      'generate',
      'BAD_STATE',
      'generate started without explorer state'
    )
    return
  }
  _state.phase.value = 'generating'
  const res = await postJson<{ task_id: string }>('/generate', {
    workspace_root: _state.pickedPath.value,
    task: ONRAMP_DEFAULT_TASK,
    stations: _explorerState.stations ?? []
  })
  if (!res.ok) {
    setError('generate', res.code, res.message)
    return
  }
  _state.activeTaskId.value = res.body.task_id
  bindSseTask(res.body.task_id, {
    onDone: () => _onGenerateDone(),
    onError: (code, message) => setError('generate', code, message)
  })
}

function _onGenerateDone(): void {
  _state.phase.value = 'ready'
  _state.activeTaskId.value = null
  disposeSse()
}

async function start(absolutePath: string): Promise<void> {
  // Re-running start() always resets the chain — picking a different
  // folder is a fresh onramp run.
  disposeSse()
  _scanResult = null
  _explorerState = null
  clearError()
  _state.pickedPath.value = absolutePath
  _state.workspaceId.value = await deriveWorkspaceId(absolutePath)
  _state.activeTaskId.value = null
  await _runScan(absolutePath)
}

async function triggerGenerate(): Promise<void> {
  if (_state.phase.value !== 'scan-complete' && _state.phase.value !== 'error') {
    return
  }
  clearError()
  await _runExplore()
}

async function retry(): Promise<void> {
  if (_state.phase.value !== 'error' || _lastFailedAction === null) {
    return
  }
  const failed = _lastFailedAction
  clearError()
  _lastFailedAction = null
  switch (failed) {
    case 'scan':
      if (_state.pickedPath.value === null) return
      await _runScan(_state.pickedPath.value)
      return
    case 'kb-build':
      await _runKbBuild()
      return
    case 'explore':
      await _runExplore()
      return
    case 'generate':
      await _runGenerate()
      return
  }
}

export function useWorkspaceOnramp(): UseWorkspaceOnrampApi {
  return {
    phase: _state.phase,
    workspaceId: _state.workspaceId,
    pickedPath: _state.pickedPath,
    progressEvents: _state.progressEvents,
    errorMsg: _state.errorMsg,
    errorCode: _state.errorCode,
    activeTaskId: _state.activeTaskId,
    start,
    triggerGenerate,
    retry
  }
}

// Test-only export. Wipes module-level singleton state so vitest tests
// can run in isolation. Production code MUST NOT call this.
export function _resetUseWorkspaceOnrampForTest(): void {
  disposeSse()
  _state.phase.value = 'idle'
  _state.workspaceId.value = null
  _state.pickedPath.value = null
  _state.progressEvents.value = []
  _state.errorMsg.value = null
  _state.errorCode.value = null
  _state.activeTaskId.value = null
  _scanResult = null
  _explorerState = null
  _lastFailedAction = null
}
