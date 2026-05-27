import type { VerbEvent } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"

/**
 * Subscribe to the latest non-thought `VerbEvent` observed on the
 * `goal-stream` channel for `runId`. Returns `null` when the store has
 * not yet recorded a tail event for that run (either because the run
 * is brand new and no non-thought event has arrived, or because the
 * run id is unknown / cleared by `useGoalsStore.reset()`).
 *
 * The selector reads only the slot for the supplied `runId`, so events
 * for other run ids do NOT trigger re-renders of consumers bound to an
 * unrelated `runId`. This is the contract that lets every row in the
 * Goals list run its own `useLatestStreamEvent(run.run_id)` without
 * the list as a whole re-rendering on every stream tick.
 */
export function useLatestStreamEvent(runId: string): VerbEvent | null {
  return useGoalsStore((s) => s.tailByRunId[runId] ?? null)
}
