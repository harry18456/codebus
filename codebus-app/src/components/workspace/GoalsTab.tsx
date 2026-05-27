import { useEffect, useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import { SectionLabel } from "@/components/ui/SectionLabel"
import { TabContentHeader } from "@/components/ui/TabContentHeader"
import type { RunLogSummary } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"
import { useNewGoalShortcut } from "@/hooks/useNewGoalShortcut"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { useT } from "@/i18n/useT"

import { NewGoalModal } from "./NewGoalModal"
import { RunListItem } from "./RunListItem"
import { WatcherStatusBanner } from "./WatcherStatusBanner"

/**
 * i18n keys for the three empty-state pre-fill examples. Localized values
 * live in `messages.ts`; the order here is the on-screen order (1..3).
 */
const GOAL_EXAMPLE_KEYS = [
  "workspace.goals.examplePlaceholder1",
  "workspace.goals.examplePlaceholder2",
  "workspace.goals.examplePlaceholder3",
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

  // Phase 4C: the content header row renders a `<kbd>N</kbd>` chip
  // labeling this shortcut. The hook auto-scopes to "Goals tab active"
  // because GoalsTab is unmounted on tab switch (Workspace re-mount
  // contract). The handler ignores presses while the New Goal modal is
  // open by checking the open state.
  useNewGoalShortcut(() => {
    if (modalOpen) return
    openModalWith()
  })

  return (
    <div
      data-testid="goals-tab"
      className="flex h-full w-full flex-col"
    >
      <WatcherStatusBanner vaultPath={vaultPath} />
      <TabContentHeader
        testId="tab-content-header-goals"
        title={t("workspace.goals.headerTitle")}
        subtitle={t("workspace.goals.headerSubtitle")}
        cta={
          <Button
            variant="primary"
            data-testid="new-goal-button"
            onClick={() => openModalWith()}
          >
            {t("workspace.goals.newGoalButton")}
          </Button>
        }
        shortcutChipText="N"
      />
      {goalRuns.length === 0 ? (
        <div
          data-testid="goals-empty"
          className="flex flex-1 flex-col items-center justify-center gap-6 px-8 text-center"
        >
          <div
            data-testid="goals-empty-hero"
            className="flex flex-col items-center gap-2"
          >
            <span
              aria-hidden="true"
              className="text-[40px] leading-none"
            >
              🎯
            </span>
            <h2 className="text-h-empty font-medium text-fg-primary">
              {t("workspace.goals.emptyHeroTitle")}
            </h2>
            <p className="text-body text-fg-secondary">
              {t("workspace.goals.emptyHeroSubtitle")}
            </p>
          </div>
          <div className="flex flex-col gap-1">
            {GOAL_EXAMPLE_KEYS.map((key, i) => {
              const example = t(key)
              return (
                <button
                  key={key}
                  type="button"
                  data-testid={`goals-empty-prefill-${i}`}
                  onClick={() => openModalWith(example)}
                  className="rounded-sm border border-border bg-bg-raised px-3 py-1 text-left font-mono text-meta text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
                >
                  {example}
                </button>
              )
            })}
          </div>
        </div>
      ) : (
        <div className="flex flex-1 flex-col overflow-auto p-2">
          <SectionLabel variant="caps" className="px-1 pb-1 pt-2">
            RECENT
          </SectionLabel>
          <ul className="flex flex-col gap-1">
            {goalRuns.map((run) => (
              <li key={run.run_id}>
                <RunListItem run={run} onClick={onSelectRun} />
              </li>
            ))}
          </ul>
        </div>
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
