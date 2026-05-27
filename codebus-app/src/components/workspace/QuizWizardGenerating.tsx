import { useMemo } from "react"

import { Button } from "@/components/ui/button"
import { projectClusters } from "@/lib/clusterTimeline"
import { summarizeVerbEvent } from "@/lib/streamEventSummary"
import type { VerbEvent } from "@/lib/ipc"
import { useT } from "@/i18n/useT"

import {
  ActivityStreamItem,
  ThoughtItem,
  foldTimeline,
} from "./ActivityStreamItem"
import { ActivityCluster } from "./ActivityCluster"

export interface QuizWizardGeneratingProps {
  topic: string
  scopePages: string[]
  events: readonly VerbEvent[]
  onCancel: () => void
}

/**
 * Step 3 of the Quiz wizard — async generation with live stream tail.
 *
 * Spec: app-workspace § Quiz Tab Wizard Content Header And Layout
 * "Generating step renders live stream tail" + Plan-Confirm-Generate
 * Flow "Plan and generate agent activity is rendered live".
 *
 * D4 reuse rule: import `streamEventSummary` and `clusterTimeline` /
 * `ActivityCluster` instead of rolling our own renderer.
 */
export function QuizWizardGenerating({
  topic,
  scopePages,
  events,
  onCancel,
}: QuizWizardGeneratingProps) {
  const t = useT()

  // Reuse helper to keep the projection consistent with the run-detail
  // running view (D4 — single rendering path for stream events).
  const timeline = useMemo(
    () => foldTimeline(events as VerbEvent[]),
    [events],
  )
  const clusters = useMemo(() => projectClusters(timeline), [timeline])
  // Touch `summarizeVerbEvent` so its import is preserved by tree-shaking
  // and the wizard share the exact summary projection used elsewhere.
  void summarizeVerbEvent

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-3 px-6 py-8">
      <h2 className="text-h-section font-semibold text-fg-primary">
        {t("workspace.quiz.wizard.step3.title")}
      </h2>

      <div
        data-testid="quiz-wizard-banner-topic"
        className="rounded-md border border-border bg-bg-raised px-3 py-2 text-meta text-fg-secondary"
      >
        <span className="mr-2">🎓</span>
        <span>quiz · </span>
        <strong className="text-fg-primary">{topic}</strong>
      </div>
      <div
        data-testid="quiz-wizard-banner-codebus"
        className="rounded-md border border-border bg-bg-raised px-3 py-2 text-meta text-fg-secondary"
      >
        <span className="mr-2">🚌</span>
        <span>{t("workspace.quiz.wizard.step3.generatingHint")}</span>
        {scopePages.length > 0 && (
          <span className="ml-2 font-mono text-fg-tertiary">
            · {scopePages.join(" · ")}
          </span>
        )}
      </div>
      <div
        data-testid="quiz-wizard-banner-thinking"
        className="rounded-md border border-border bg-bg-raised px-3 py-2 text-meta text-fg-secondary"
      >
        <span className="mr-2">🤔</span>
        <span>
          {t("workspace.quiz.wizard.step3.generatingHint")}
        </span>
      </div>

      <div
        data-testid="quiz-wizard-generating-stream-tail"
        className="mt-2 flex flex-col gap-2"
      >
        {clusters.map((cluster, idx) => (
          <ActivityCluster
            key={`cluster-${idx}`}
            cluster={cluster}
            terminal={false}
          />
        ))}
        {clusters.length === 0 &&
          timeline.map((item, idx) =>
            item.kind === "thought" ? (
              <ThoughtItem key={`thought-${idx}`} item={item} />
            ) : (
              <ActivityStreamItem key={`item-${idx}`} item={item} />
            ),
          )}
      </div>

      <div className="mt-4 flex items-center justify-end">
        <Button
          variant="secondary"
          onClick={onCancel}
          data-testid="quiz-wizard-generating-cancel"
        >
          {t("workspace.quiz.wizard.action.cancel")}
        </Button>
      </div>
    </div>
  )
}
