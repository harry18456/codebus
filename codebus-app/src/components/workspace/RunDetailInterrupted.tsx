import { useState } from "react"

import { Button } from "@/components/ui/button"
import { StatusPill, type StatusPillStatus } from "@/components/ui/StatusPill"
import { useT } from "@/i18n/useT"
import type { InterruptReason, RunDetail, RunOutcome } from "@/lib/ipc"

import { NewGoalModal } from "./NewGoalModal"

interface RunDetailInterruptedProps {
  detail: RunDetail
  vaultPath: string
  onBack: () => void
  /**
   * Called after the Retry-with-same-goal modal's spawn resolves with the
   * new RunId so the parent can switch directly to the new Running detail
   * view.
   */
  onRetrySpawned?: (runId: string) => void
}

/**
 * Spec: app-workspace § Run Detail Views — Cancelled and Interrupted.
 *
 * Single detail view for every non-running terminal outcome that is not
 * `"succeeded"`. The explicit state machine is two stages:
 *
 *   1. outcome → banner tier
 *      "failed"                  → "red"
 *      "cancelled" | "interrupted" → "amber"
 *
 *   2. (amber tier only) interrupt_reason → subtitle i18n key
 *      "app-close"               → banner.reason.appClose
 *      "user-cancel"             → banner.reason.userCancel
 *      "network-drop"            → banner.reason.networkDrop
 *      { other: string }         → banner.reason.other
 *      undefined                 → banner.interruptedSubtitle (generic fallback)
 *
 * The red tier ignores `interrupt_reason` entirely — Failed always uses
 * banner.failedTitle + banner.failedSubtitle. The raw inner string of the
 * Other variant is NEVER rendered into the UI text (it carries
 * schema-internal tokens, not user-facing copy).
 */
export function RunDetailInterrupted({
  detail,
  vaultPath,
  onBack,
  onRetrySpawned,
}: RunDetailInterruptedProps) {
  const t = useT()
  const [retryOpen, setRetryOpen] = useState(false)
  const summary = detail.summary
  const partial = partialTimeline(detail)

  const tier = bannerTier(summary.outcome as RunOutcome)
  const reasonKey = reasonSubtitleKey(tier, summary.interrupt_reason)
  const titleKey: TitleKey =
    tier === "red"
      ? "workspace.runDetail.banner.failedTitle"
      : "workspace.runDetail.banner.interruptedTitle"
  const subtitleKey = subtitleI18nKey(tier, reasonKey)
  const pillStatus: StatusPillStatus = tier === "red" ? "failed" : "interrupted"
  const bannerClass =
    tier === "red"
      ? "m-3 rounded-md border border-error/40 bg-error/10 px-3 py-2 text-meta text-error"
      : "m-3 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-meta text-warning"

  return (
    <div data-testid="run-detail-interrupted" className="flex h-full flex-col">
      <header
        data-tauri-drag-region
        className="flex select-none items-center gap-3 border-b border-border px-3 py-2 pr-[160px]"
      >
        <button
          type="button"
          onClick={onBack}
          data-testid="run-detail-back"
          className="text-meta text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
        >
          {t("workspace.runDetail.backLink")}
        </button>
        <span data-tauri-drag-region className="flex-1 truncate text-body">
          {summary.goal}
        </span>
        <span data-tauri-drag-region data-testid={`interrupted-badge-${tier}`}>
          <StatusPill status={pillStatus} variant="pill" />
        </span>
      </header>
      <div
        data-testid={`interrupted-banner-${reasonKey}`}
        className={bannerClass}
      >
        <div className="font-semibold">{t(titleKey)}</div>
        <div className="mt-0.5">{t(subtitleKey)}</div>
      </div>
      <PartialTimeline timeline={partial} />
      <footer className="flex justify-end border-t border-border px-3 py-2">
        <Button
          data-testid="retry-button"
          onClick={() => setRetryOpen(true)}
        >
          {t("workspace.runDetail.retryButton")}
        </Button>
      </footer>
      <NewGoalModal
        open={retryOpen}
        vaultPath={vaultPath}
        initialText={summary.goal}
        onClose={() => setRetryOpen(false)}
        onSpawned={onRetrySpawned}
      />
    </div>
  )
}

/** Stage 1 — outcome → banner tier. */
type BannerTier = "red" | "amber"

function bannerTier(outcome: RunOutcome): BannerTier {
  switch (outcome) {
    case "failed":
      return "red"
    case "cancelled":
    case "interrupted":
      return "amber"
    default:
      // `succeeded` / `running` never reach this component (Workspace.tsx
      // routes those to RunDetailDone / RunDetailRunning). Defensive
      // fallback keeps render safe even if a future virtual entry ships
      // an outcome value we don't recognise.
      return "amber"
  }
}

/** Stage 2 — interrupt_reason → reason key used for testid + subtitle lookup. */
type ReasonKey = "appClose" | "userCancel" | "networkDrop" | "other" | "fallback"

function reasonSubtitleKey(
  tier: BannerTier,
  reason: InterruptReason | undefined,
): ReasonKey {
  if (tier === "red") {
    // Red tier ignores interrupt_reason by contract.
    return "fallback"
  }
  if (reason === undefined) {
    return "fallback"
  }
  if (typeof reason === "string") {
    switch (reason) {
      case "app-close":
        return "appClose"
      case "user-cancel":
        return "userCancel"
      case "network-drop":
        return "networkDrop"
    }
  }
  // `{ other: string }` newtype variant — raw inner string is intentionally
  // not surfaced; the generic reason.other copy carries the user-facing
  // message.
  return "other"
}

type TitleKey =
  | "workspace.runDetail.banner.failedTitle"
  | "workspace.runDetail.banner.interruptedTitle"

type SubtitleKey =
  | "workspace.runDetail.banner.failedSubtitle"
  | "workspace.runDetail.banner.interruptedSubtitle"
  | "workspace.runDetail.banner.reason.appClose"
  | "workspace.runDetail.banner.reason.userCancel"
  | "workspace.runDetail.banner.reason.networkDrop"
  | "workspace.runDetail.banner.reason.other"

function subtitleI18nKey(tier: BannerTier, reasonKey: ReasonKey): SubtitleKey {
  if (tier === "red") {
    return "workspace.runDetail.banner.failedSubtitle"
  }
  switch (reasonKey) {
    case "appClose":
      return "workspace.runDetail.banner.reason.appClose"
    case "userCancel":
      return "workspace.runDetail.banner.reason.userCancel"
    case "networkDrop":
      return "workspace.runDetail.banner.reason.networkDrop"
    case "other":
      return "workspace.runDetail.banner.reason.other"
    case "fallback":
      return "workspace.runDetail.banner.interruptedSubtitle"
  }
}

interface Timeline {
  reading: number
  writing: number
  other: number
}

function partialTimeline(detail: RunDetail): Timeline {
  const READING = new Set(["Read", "Glob", "Grep"])
  const WRITING = new Set(["Write", "Edit"])
  const out: Timeline = { reading: 0, writing: 0, other: 0 }
  for (const env of detail.events) {
    if (env.event.kind !== "stream") continue
    if (env.event.data.kind !== "tool_use") continue
    const name = env.event.data.name
    if (READING.has(name)) out.reading += 1
    else if (WRITING.has(name)) out.writing += 1
    else out.other += 1
  }
  return out
}

function PartialTimeline({ timeline }: { timeline: Timeline }) {
  const t = useT()
  return (
    <div className="px-3 pb-3 text-meta text-fg-secondary">
      <h3 className="mb-1 text-meta font-semibold uppercase tracking-wide text-fg-tertiary">
        {t("workspace.runDetail.partialTimelineLabel")}
      </h3>
      <p>
        reading {timeline.reading} · writing {timeline.writing} · other {timeline.other}
      </p>
    </div>
  )
}
