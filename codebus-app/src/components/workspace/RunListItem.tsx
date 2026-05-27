import type { RunLogSummary } from "@/lib/ipc"
import { cn } from "@/lib/cn"
import {
  StatusPill,
  type StatusPillStatus,
} from "@/components/ui/StatusPill"
import { useT, type TFunction } from "@/i18n/useT"
import { useLatestStreamEvent } from "@/hooks/useLatestStreamEvent"
import { summarizeVerbEvent } from "@/lib/streamEventSummary"

/**
 * Map a RunLog outcome to a canonical three-state status (Phase 3B).
 * Returns `null` for the `running` outcome (rendered with the legacy
 * 🚌 + animate-pulse glyph since the dot variant excludes running by
 * design — running uses motion, not a color dot, in the Goals list).
 */
function outcomeToStatus(outcome: string): StatusPillStatus | null {
  switch (outcome) {
    case "running":
      return null
    case "succeeded":
      return "done"
    case "cancelled":
    case "interrupted":
      return "interrupted"
    case "failed":
      return "failed"
    default:
      return null
  }
}

function truncate(text: string, max = 80): string {
  if (text.length <= max) return text
  return `${text.slice(0, max - 1)}…`
}

function relativeTimestamp(
  iso: string,
  t: TFunction,
  now = new Date(),
): string {
  if (!iso) return ""
  const ts = Date.parse(iso)
  if (Number.isNaN(ts)) return ""
  const diffMs = now.getTime() - ts
  const diffMin = Math.floor(diffMs / 60_000)
  if (diffMin < 1) return "just now"
  if (diffMin < 60) return t("common.minutesAgo", { n: diffMin })
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return t("common.hoursAgo", { n: diffHr })
  const diffDay = Math.floor(diffHr / 24)
  return t("common.daysAgo", { n: diffDay })
}

interface RunListItemProps {
  run: RunLogSummary
  onClick: (run: RunLogSummary) => void
}

export function RunListItem({ run, onClick }: RunListItemProps) {
  const t = useT()
  return (
    <button
      type="button"
      data-testid={`run-row-${run.run_id}`}
      data-outcome={run.outcome}
      onClick={() => onClick(run)}
      className={cn(
        "flex w-full items-center gap-3 rounded-md px-3 py-2 text-left",
        "hover:bg-bg-sunken focus:outline-none focus:ring-2 focus:ring-accent-ring",
      )}
    >
      {run.outcome === "running" ? (
        <span
          aria-hidden="true"
          className={cn("text-body-lg", "animate-pulse")}
        >
          🚌
        </span>
      ) : (
        (() => {
          const status = outcomeToStatus(run.outcome)
          return status ? (
            <StatusPill status={status} variant="dot" />
          ) : (
            <span aria-hidden="true" className="text-body-lg">
              •
            </span>
          )
        })()
      )}
      <span className="min-w-0 flex-1 truncate text-body">
        {truncate(run.goal || "(no goal text)")}
      </span>
      {run.outcome === "running" ? <RunningRowTail runId={run.run_id} /> : null}
      <span className="text-meta text-fg-tertiary">
        {relativeTimestamp(run.started_at, t)}
      </span>
    </button>
  )
}

/**
 * Single-line "stream tail" rendered to the right of the goal text for
 * running rows. Subscribes to `useLatestStreamEvent(runId)` so each row
 * only re-renders when its own run's tail slot updates. Thought chunks
 * are filtered at the store layer, so `summarizeVerbEvent` returning
 * non-null is the common case once the run starts emitting tool_use or
 * banner events; the placeholder covers the brief window before that.
 */
function RunningRowTail({ runId }: { runId: string }) {
  const t = useT()
  const tailEvent = useLatestStreamEvent(runId)
  const summary = tailEvent ? summarizeVerbEvent(tailEvent, t) : null
  const text = summary ?? t("workspace.goals.runningTailPending")
  return (
    <span
      data-testid="run-row-tail"
      className="hidden max-w-[40ch] truncate font-mono text-meta text-fg-secondary tabular-nums lg:block"
    >
      {text}
    </span>
  )
}
