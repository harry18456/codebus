import { useEffect, useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import type { RunLogSummary } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { useT } from "@/i18n/useT"

import { NewGoalModal } from "./NewGoalModal"
import { RunListItem } from "./RunListItem"
import { WatcherStatusBanner } from "./WatcherStatusBanner"

/**
 * Pre-fill examples for the empty Goals overview hint. Spec
 * (app-workspace § Goals Overview List and Filter) requires exactly
 * three clickable rows; the strings themselves are illustrative.
 */
const GOAL_EXAMPLES: readonly string[] = [
  "describe the authentication flow",
  "summarize the data ingestion pipeline",
  "map the public API surface",
] as const

interface GoalsTabProps {
  vaultPath: string
  onSelectRun: (run: RunLogSummary) => void
  /**
   * Called after `spawn_goal` resolves with the new RunId so the
   * parent (Workspace) can switch directly to the Running detail
   * view instead of dropping the user back into the overview list.
   */
  onSpawnedRun?: (runId: string) => void
}

export function GoalsTab({
  vaultPath,
  onSelectRun,
  onSpawnedRun,
}: GoalsTabProps) {
  const t = useT()
  const runs = useGoalsStore((s) => s.runs)
  const refreshRuns = useGoalsStore((s) => s.refreshRuns)
  const [modalOpen, setModalOpen] = useState(false)
  const [prefill, setPrefill] = useState("")

  // Terminal-spawned goals appearing in `<vault>/.codebus/log/` SHALL
  // surface in the Goals list without requiring the user to switch
  // tabs. Spec: `Goals Tab Subscribes To Watcher Events`.
  useEffect(
    () => useWatcherEvent("goals-changed", () => {
      void refreshRuns(vaultPath)
    }),
    [refreshRuns, vaultPath],
  )

  // Design + spec: Goals tab filters to mode === "goal" client-side
  // even though IPC already filters. The extra guard ensures the UI
  // remains correct if a future store update bypasses the filter.
  // Then sort descending by started_at as the visible invariant.
  const goalRuns = useMemo(() => {
    return runs
      .filter((r) => r.mode === "goal")
      .slice()
      .sort((a, b) => b.started_at.localeCompare(a.started_at))
  }, [runs])

  function openModalWith(text = "") {
    setPrefill(text)
    setModalOpen(true)
  }

  return (
    <div
      data-testid="goals-tab"
      className="flex h-full w-full flex-col"
    >
      <WatcherStatusBanner vaultPath={vaultPath} />
      {/* pr-[160px] leaves room for the fixed WindowControls (3 × 46px). */}
      <div
        data-tauri-drag-region
        className="flex justify-end border-b border-border p-3 pr-[160px]"
      >
        <Button
          variant="primary"
          data-testid="new-goal-button"
          onClick={() => openModalWith()}
        >
          + New goal
        </Button>
      </div>
      {goalRuns.length === 0 ? (
        <div
          data-testid="goals-empty"
          className="flex flex-1 flex-col items-center justify-center gap-4 px-8 text-center"
        >
          <p className="text-body text-fg-secondary">
            {t("workspace.goals.emptyHint")}
          </p>
          <div className="flex flex-col gap-1">
            {GOAL_EXAMPLES.map((ex, i) => (
              <button
                key={ex}
                type="button"
                data-testid={`goals-empty-prefill-${i}`}
                onClick={() => openModalWith(ex)}
                className="rounded-sm text-left text-meta text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
              >
                “{ex}”
              </button>
            ))}
          </div>
        </div>
      ) : (
        <ul className="flex flex-1 flex-col gap-1 overflow-auto p-2">
          {goalRuns.map((run) => (
            <li key={run.run_id}>
              <RunListItem run={run} onClick={onSelectRun} />
            </li>
          ))}
        </ul>
      )}
      <NewGoalModal
        open={modalOpen}
        vaultPath={vaultPath}
        initialText={prefill}
        onClose={() => setModalOpen(false)}
        onSpawned={onSpawnedRun}
      />
    </div>
  )
}
