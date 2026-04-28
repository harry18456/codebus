// Single writer for `<ws>/codebus-tutorials/{task_id}/progress.json`.
// Spec: Requirement "progress.json schema and single-writer path".
//
// Invariants:
// - This module is the only caller of `writeProgressFile` (defensive
//   grep enforces this).
// - No browser-side persistence — `progress.json` on disk is the
//   canonical source of truth (per spec scenario).
// - Writes debounce ~500ms; `beforeunload` flushes synchronously so a
//   tick + immediate close survives.
// - All `state.completed_station_ids` mutations happen inside one
//   module-level watch keyed off `(checkpoints, quizzes, activeRoute)`.
//   Computed derivations stay pure (Vue forbids side effects in
//   `computed`); the watch is the single mutation point.

import { computed, ref, watch, type ComputedRef, type Ref } from 'vue'

import type { RouteJson, RouteStation } from './useStationRoute'
import { useTutorialFiles } from './useTutorialFiles'

const DEBOUNCE_MS = 500

export interface CheckpointProgress {
  done: boolean
  ts: string
}

export interface QuizProgress {
  answer: string
  correct: boolean
  attempts: number
}

export interface TutorialProgress {
  current_station_id: string | null
  completed_station_ids: string[]
  checkpoints: Record<string, CheckpointProgress>
  quizzes: Record<string, QuizProgress>
}

function emptyProgress(): TutorialProgress {
  return {
    current_station_id: null,
    completed_station_ids: [],
    checkpoints: {},
    quizzes: {}
  }
}

const state = ref<TutorialProgress>(emptyProgress())
const workspaceRoot = ref<string | null>(null)
const taskId = ref<string | null>(null)
const activeRoute = ref<RouteJson | null>(null)
const dirty = ref(false)

let debounceTimer: ReturnType<typeof setTimeout> | null = null
let beforeUnloadAttached = false

function attachBeforeUnload(): void {
  if (beforeUnloadAttached) return
  if (typeof window === 'undefined') return
  beforeUnloadAttached = true
  window.addEventListener('beforeunload', () => {
    if (!dirty.value) return
    void flushNow()
  })
}

async function flushNow(): Promise<void> {
  if (!dirty.value) return
  if (!workspaceRoot.value || !taskId.value) return
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  const files = useTutorialFiles()
  const payload = JSON.stringify(state.value)
  const ws = workspaceRoot.value
  const tid = taskId.value
  dirty.value = false
  try {
    await files.writeProgressFile(ws, tid, payload)
  } catch (err) {
    // H1 fix: failed write must self-retry instead of waiting for the
    // next mutation. Restore dirty + reschedule the debounce so a
    // transient IPC failure (e.g. disk lock) recovers without losing
    // user input.
    dirty.value = true
    scheduleFlush()
    throw err
  }
}

function scheduleFlush(): void {
  dirty.value = true
  attachBeforeUnload()
  if (debounceTimer !== null) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    debounceTimer = null
    void flushNow().catch(() => {
      /* H1: scheduleFlush already restored timer in catch path. */
    })
  }, DEBOUNCE_MS)
}

// ----- nested shape guards (H2) --------------------------------------------

function isCheckpointEntry(v: unknown): v is CheckpointProgress {
  if (typeof v !== 'object' || v === null) return false
  const o = v as { done?: unknown; ts?: unknown }
  return typeof o.done === 'boolean' && typeof o.ts === 'string'
}

function isQuizEntry(v: unknown): v is QuizProgress {
  if (typeof v !== 'object' || v === null) return false
  const o = v as { answer?: unknown; correct?: unknown; attempts?: unknown }
  return (
    typeof o.answer === 'string' &&
    typeof o.correct === 'boolean' &&
    typeof o.attempts === 'number'
  )
}

function sanitizeStringRecord<T>(
  raw: unknown,
  guard: (v: unknown) => v is T
): Record<string, T> {
  if (!raw || typeof raw !== 'object') return {}
  const out: Record<string, T> = {}
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof k === 'string' && guard(v)) out[k] = v
  }
  return out
}

function sanitizeStringArray(raw: unknown): string[] {
  if (!Array.isArray(raw)) return []
  return raw.filter((v): v is string => typeof v === 'string')
}

// ----- public API ----------------------------------------------------------

async function loadProgress(ws: string, tid: string): Promise<void> {
  // C2 fix: clean up any pending flush for the previous workspace
  // before swapping `state`. We try to flush first so a debounce-
  // window mutation isn't silently lost; failures here are best-effort
  // because the previous workspace may already be unmounted.
  if (workspaceRoot.value !== null && dirty.value) {
    try {
      await flushNow()
    } catch {
      /* Best-effort: prior workspace may have become unreachable. */
    }
  }
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  dirty.value = false
  workspaceRoot.value = ws
  taskId.value = tid
  state.value = emptyProgress()
  // Drop the previous route so the completed-stations watcher does not
  // recompute against a stale route shape.
  activeRoute.value = null

  const files = useTutorialFiles()
  try {
    const raw = await files.readTutorialFile(
      ws,
      `codebus-tutorials/${tid}/progress.json`
    )
    const parsed = JSON.parse(raw) as Partial<TutorialProgress> | null
    if (parsed && typeof parsed === 'object') {
      state.value = {
        current_station_id:
          typeof parsed.current_station_id === 'string'
            ? parsed.current_station_id
            : null,
        completed_station_ids: sanitizeStringArray(parsed.completed_station_ids),
        checkpoints: sanitizeStringRecord(parsed.checkpoints, isCheckpointEntry),
        quizzes: sanitizeStringRecord(parsed.quizzes, isQuizEntry)
      }
    }
  } catch {
    // Spec: progress.json absent on first visit → in-memory empty
    // schema, file NOT created until the first mutating call.
    state.value = emptyProgress()
  }
  dirty.value = false
}

function setRoute(route: RouteJson): void {
  activeRoute.value = route
}

function setCheckpoint(id: string, _itemIndex: number, checked: boolean): void {
  // Spec models `progress.checkpoints[id]` as a single { done, ts }
  // record — it represents the Checkpoint as a whole, not per-item state.
  // The component decides when "all items are checked" and only then
  // flips `done` to true; intermediate item state is held in component
  // memory and never persisted. So `setCheckpoint(id, _, true)` records
  // the Checkpoint as completed; `false` reverses it.
  if (checked) {
    state.value.checkpoints[id] = { done: true, ts: new Date().toISOString() }
  } else {
    delete state.value.checkpoints[id]
  }
  scheduleFlush()
}

function setQuizAnswer(id: string, selected: string, correct: boolean): void {
  const prev = state.value.quizzes[id]
  state.value.quizzes[id] = {
    answer: selected,
    correct,
    attempts: (prev?.attempts ?? 0) + 1
  }
  scheduleFlush()
}

function setCurrentStation(stationId: string): void {
  if (state.value.current_station_id === stationId) return
  state.value.current_station_id = stationId
  scheduleFlush()
}

async function resetProgress(): Promise<void> {
  // Wipe in-memory state and immediately flush an empty progress.json
  // so the unlock-chain computeds re-derive an empty unlocked set on
  // the next tick. The watch keyed on (checkpoints, quizzes,
  // activeRoute) will pick this up and reset completed_station_ids
  // to [] without further help.
  state.value = emptyProgress()
  dirty.value = true
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
  await flushNow()
}

function isStationComplete(station: RouteStation): boolean {
  return station.required_checks.every((checkId) => {
    if (state.value.checkpoints[checkId]?.done === true) return true
    if (state.value.quizzes[checkId]?.correct === true) return true
    return false
  })
}

function computeUnlockedSet(route: RouteJson): Set<string> {
  const set = new Set<string>()
  if (route.stations.length === 0) return set
  set.add(route.stations[0]!.station_id)
  for (let i = 0; i < route.stations.length - 1; i++) {
    const station = route.stations[i]!
    if (!set.has(station.station_id)) break
    if (isStationComplete(station)) {
      set.add(route.stations[i + 1]!.station_id)
    } else {
      break
    }
  }
  return set
}

function computeCompletedList(route: RouteJson): string[] {
  // Stations are "completed" only when every prefix predecessor in
  // route order also passed; this mirrors the unlock chain.
  const out: string[] = []
  for (const station of route.stations) {
    if (!isStationComplete(station)) break
    out.push(station.station_id)
  }
  return out
}

// C1 fix: side effect lives in a watch, not a computed. The watch
// keys off the mutation surface (checkpoints / quizzes) plus the
// active route, recomputes the completed list, and only writes back
// when it actually changes (avoids reactivity loops).
watch(
  [
    () => state.value.checkpoints,
    () => state.value.quizzes,
    activeRoute
  ] as const,
  () => {
    const route = activeRoute.value
    if (!route) return
    const next = computeCompletedList(route)
    const cur = state.value.completed_station_ids
    if (
      next.length === cur.length &&
      next.every((id, i) => id === cur[i])
    ) {
      return
    }
    state.value.completed_station_ids = next
    scheduleFlush()
  },
  { deep: true, flush: 'post' }
)

function unlockedStationIds(route: RouteJson): ComputedRef<Set<string>> {
  return computed(() => computeUnlockedSet(route))
}

function canVisitStation(stationId: string, route: RouteJson): ComputedRef<boolean> {
  return computed(() => {
    const unlocked = computeUnlockedSet(route)
    if (unlocked.has(stationId)) return true
    return state.value.completed_station_ids.includes(stationId)
  })
}

interface TutorialProgressApi {
  state: Ref<TutorialProgress>
  loadProgress: typeof loadProgress
  setRoute: typeof setRoute
  setCheckpoint: typeof setCheckpoint
  setQuizAnswer: typeof setQuizAnswer
  setCurrentStation: typeof setCurrentStation
  isStationComplete: typeof isStationComplete
  unlockedStationIds: typeof unlockedStationIds
  canVisitStation: typeof canVisitStation
  flushNow: typeof flushNow
  resetProgress: typeof resetProgress
}

export function useTutorialProgress(): TutorialProgressApi {
  return {
    state,
    loadProgress,
    setRoute,
    setCheckpoint,
    setQuizAnswer,
    setCurrentStation,
    isStationComplete,
    unlockedStationIds,
    canVisitStation,
    flushNow,
    resetProgress
  }
}
