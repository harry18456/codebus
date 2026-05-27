import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { create } from "zustand"

import {
  cancelGoal as cancelGoalIpc,
  listRuns,
  spawnGoal as spawnGoalIpc,
  type GoalStreamPayload,
  type GoalTerminalPayload,
  type RunLogSummary,
  type VerbEvent,
} from "@/lib/ipc"

/**
 * Volatile state for the run currently being streamed. Lives only
 * between `spawn_goal` and the run's terminal event; cleared by
 * `refreshRuns` (which picks up the persisted RunLog row) or by
 * `reset`.
 *
 * The `events` buffer holds every `VerbEvent` received over the
 * `goal-stream` channel, in arrival order. Components project the
 * buffer into view-models (tool_use one-liners, concatenated thought
 * chunks, etc.) per the design "Stream rendering 在 frontend、不在 Rust"
 * decision.
 *
 * `cancelling` is a frontend-only optimistic flag for the
 * `Cancelling…` button state — flipped on click of the Cancel
 * button before the backend `cancel_goal` resolves so the user gets
 * immediate UI feedback.
 */
export interface ActiveRunState {
  runId: string
  goal: string
  startedAt: string
  events: VerbEvent[]
  cancelling: boolean
}

interface GoalsState {
  runs: RunLogSummary[]
  activeRun: ActiveRunState | null
  /**
   * Per-run "latest non-thought stream event" slot, used by the Goals
   * list `RunListItem` running-row stream tail to project a one-line
   * summary without subscribing to the full event timeline.
   *
   * Written by `_onStreamEvent` on every non-thought stream event for
   * any `run_id` (including runs not present in `activeRun` — e.g.
   * goals spawned from a terminal). NOT cleared by `_onTerminal` so
   * the tail can freeze on the last event after a goal ends. Cleared
   * in bulk by `reset()` on Workspace unmount / vault switch.
   */
  tailByRunId: Record<string, VerbEvent>
  /**
   * Last vault path passed to `refreshRuns` / `spawnGoal`. Used by the
   * terminal-event handler to refresh the runs list without forcing
   * callers to thread the vault path through the channel payload.
   */
  _currentVaultPath: string | null
  spawnGoal: (vaultPath: string, text: string) => Promise<string>
  cancelGoal: (runId: string) => Promise<void>
  refreshRuns: (vaultPath: string) => Promise<void>
  /** Clear runs + activeRun. Called on Workspace unmount. */
  reset: () => void
  /**
   * Internal slot exposed for tests so the `goal-stream` listener can
   * drive events without going through the Tauri event bus. Components
   * SHALL NOT call this directly.
   */
  _onStreamEvent: (payload: GoalStreamPayload) => void
  /** Internal slot for tests; same caveat as `_onStreamEvent`. */
  _onTerminal: (payload: GoalTerminalPayload) => void
}

/**
 * Subscribe to the `goal-stream` Tauri event channel exactly once per
 * store instance. Spec scenario
 * `useGoalsStore_subscribes_goal_stream_channel` asserts this happens
 * at store init. The unlisten handle is kept in a closure so the
 * subscription survives for the whole app session; the listener is
 * cheap and v1 has no multi-store scenario that would benefit from
 * teardown.
 */
function startStreamSubscription(
  onEvent: (payload: GoalStreamPayload) => void,
  onTerminal: (payload: GoalTerminalPayload) => void,
): void {
  let unlistenStream: UnlistenFn | null = null
  let unlistenTerminal: UnlistenFn | null = null
  void listen<GoalStreamPayload>("goal-stream", (event) => {
    onEvent(event.payload)
  }).then((handle) => {
    unlistenStream = handle
  })
  void listen<GoalTerminalPayload>("goal-terminal", (event) => {
    onTerminal(event.payload)
  }).then((handle) => {
    unlistenTerminal = handle
  })
  // The handles are captured for parity with future teardown paths;
  // we deliberately do not call them during the app lifetime.
  void unlistenStream
  void unlistenTerminal
}

export const useGoalsStore = create<GoalsState>((set, get) => {
  startStreamSubscription(
    (payload) => get()._onStreamEvent(payload),
    (payload) => get()._onTerminal(payload),
  )

  return {
    runs: [],
    activeRun: null,
    tailByRunId: {},
    _currentVaultPath: null,

    async spawnGoal(vaultPath, text) {
      const runId = await spawnGoalIpc(vaultPath, text)
      const startedAt = new Date().toISOString()
      // Optimistic insert: synthesize a running-state summary so the
      // Goals overview list immediately shows the new row + the
      // Running detail view can switch in without waiting for the
      // first `goal-stream` event.
      const optimisticSummary: RunLogSummary = {
        run_id: runId,
        mode: "goal",
        goal: text,
        started_at: startedAt,
        finished_at: "",
        tokens: { input_tokens: 0, output_tokens: 0 },
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: "running",
      }
      set((state) => ({
        activeRun: {
          runId,
          goal: text,
          startedAt,
          events: [],
          cancelling: false,
        },
        runs: [optimisticSummary, ...state.runs],
        _currentVaultPath: vaultPath,
      }))
      return runId
    },

    async cancelGoal(runId) {
      // Flip the local cancelling flag synchronously so the button
      // transitions to its `Cancelling…` disabled state immediately.
      set((state) =>
        state.activeRun && state.activeRun.runId === runId
          ? {
              activeRun: { ...state.activeRun, cancelling: true },
            }
          : {},
      )
      await cancelGoalIpc(runId)
    },

    async refreshRuns(vaultPath) {
      set({ _currentVaultPath: vaultPath })
      const runs = await listRuns(vaultPath, { kind: "goal" })
      // An in-flight goal has no terminal RunLog row yet, so `list_runs`
      // synthesizes a virtual `interrupted` row for it (events present,
      // RunLog absent). Let the optimistic `running` state from `activeRun`
      // win over that disk-derived row so the in-progress goal keeps its
      // 🚌 running indicator instead of flipping to ⚠ interrupted.
      set((state) => {
        const ar = state.activeRun
        if (!ar) return { runs }
        const runningSummary: RunLogSummary = {
          run_id: ar.runId,
          mode: "goal",
          goal: ar.goal,
          started_at: ar.startedAt,
          finished_at: "",
          tokens: { input_tokens: 0, output_tokens: 0 },
          wiki_changed: false,
          lint_error_count: 0,
          lint_warn_count: 0,
          outcome: "running",
        }
        const withoutActive = runs.filter((r) => r.run_id !== ar.runId)
        return { runs: [runningSummary, ...withoutActive] }
      })
    },

    reset() {
      set({
        runs: [],
        activeRun: null,
        tailByRunId: {},
        _currentVaultPath: null,
      })
    },

    _onStreamEvent(payload) {
      set((state) => {
        const isThought =
          payload.event.kind === "stream" &&
          payload.event.data.kind === "thought"
        // tail slot tracks the latest non-thought event for ANY run id —
        // including runs not present in `activeRun` (e.g. goals spawned
        // from a terminal). Thought chunks are filtered at write time so
        // hook consumers never need to walk history looking for the last
        // non-thought event.
        const nextTail = isThought
          ? state.tailByRunId
          : { ...state.tailByRunId, [payload.run_id]: payload.event }

        if (!state.activeRun || state.activeRun.runId !== payload.run_id) {
          return nextTail === state.tailByRunId ? {} : { tailByRunId: nextTail }
        }
        return {
          activeRun: {
            ...state.activeRun,
            events: [...state.activeRun.events, payload.event],
          },
          ...(nextTail === state.tailByRunId ? {} : { tailByRunId: nextTail }),
        }
      })
    },

    _onTerminal(payload) {
      const state = get()
      if (!state.activeRun || state.activeRun.runId !== payload.run_id) {
        return
      }
      set({ activeRun: null })
      const vaultPath = state._currentVaultPath
      if (vaultPath) {
        void get().refreshRuns(vaultPath)
      }
    },
  }
})
