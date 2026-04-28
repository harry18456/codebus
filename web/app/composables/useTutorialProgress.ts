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

import { computed, ref, type ComputedRef, type Ref } from 'vue'

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
    // Mark dirty again so the next mutation will retry — never silently
    // drop user input.
    dirty.value = true
    throw err
  }
}

function scheduleFlush(): void {
  dirty.value = true
  attachBeforeUnload()
  if (debounceTimer !== null) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    debounceTimer = null
    void flushNow()
  }, DEBOUNCE_MS)
}

async function loadProgress(ws: string, tid: string): Promise<void> {
  workspaceRoot.value = ws
  taskId.value = tid
  state.value = emptyProgress()

  const files = useTutorialFiles()
  try {
    const raw = await files.readTutorialFile(
      ws,
      `codebus-tutorials/${tid}/progress.json`
    )
    const parsed = JSON.parse(raw) as Partial<TutorialProgress>
    state.value = {
      current_station_id: parsed.current_station_id ?? null,
      completed_station_ids: Array.isArray(parsed.completed_station_ids)
        ? parsed.completed_station_ids
        : [],
      checkpoints:
        parsed.checkpoints && typeof parsed.checkpoints === 'object'
          ? parsed.checkpoints
          : {},
      quizzes:
        parsed.quizzes && typeof parsed.quizzes === 'object'
          ? parsed.quizzes
          : {}
    }
  } catch {
    // Spec: progress.json absent on first visit → in-memory empty
    // schema, file NOT created until the first mutating call.
    state.value = emptyProgress()
  }
  dirty.value = false
}

function setCheckpoint(id: string, _itemIndex: number, checked: boolean): void {
  // Spec models `progress.checkpoints[id]` as a single { done, ts }
  // record — it represents the Checkpoint as a whole, not per-item state.
  // The component decides when "all items are checked" and only then
  // flips `done` to true; intermediate item state is held in component
  // memory and never persisted. So `setCheckpoint(id, _, true)` records
  // the Checkpoint as completed; `false` reverses it (rare but allowed
  // when the user unchecks after passing).
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

function isStationComplete(station: RouteStation): boolean {
  return station.required_checks.every((checkId) => {
    if (state.value.checkpoints[checkId]?.done === true) return true
    if (state.value.quizzes[checkId]?.correct === true) return true
    return false
  })
}

function unlockedStationIds(route: RouteJson): ComputedRef<Set<string>> {
  return computed(() => {
    const set = new Set<string>()
    if (route.stations.length === 0) return set
    set.add(route.stations[0]!.station_id)
    for (let i = 0; i < route.stations.length - 1; i++) {
      const station = route.stations[i]!
      if (!set.has(station.station_id)) break
      if (isStationComplete(station)) {
        set.add(route.stations[i + 1]!.station_id)
        // Also mark the station as completed if not already (idempotent).
        if (!state.value.completed_station_ids.includes(station.station_id)) {
          state.value.completed_station_ids = [
            ...state.value.completed_station_ids,
            station.station_id
          ]
          scheduleFlush()
        }
      } else {
        break
      }
    }
    return set
  })
}

function canVisitStation(stationId: string, route: RouteJson): ComputedRef<boolean> {
  const unlocked = unlockedStationIds(route)
  return computed(
    () =>
      unlocked.value.has(stationId) ||
      state.value.completed_station_ids.includes(stationId)
  )
}

interface TutorialProgressApi {
  state: Ref<TutorialProgress>
  loadProgress: typeof loadProgress
  setCheckpoint: typeof setCheckpoint
  setQuizAnswer: typeof setQuizAnswer
  setCurrentStation: typeof setCurrentStation
  isStationComplete: typeof isStationComplete
  unlockedStationIds: typeof unlockedStationIds
  canVisitStation: typeof canVisitStation
  flushNow: typeof flushNow
}

export function useTutorialProgress(): TutorialProgressApi {
  return {
    state,
    loadProgress,
    setCheckpoint,
    setQuizAnswer,
    setCurrentStation,
    isStationComplete,
    unlockedStationIds,
    canVisitStation,
    flushNow
  }
}
