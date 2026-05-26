import type { RunLogSummary } from "@/lib/ipc"
import { cn } from "@/lib/cn"
import {
  StatusPill,
  type StatusPillStatus,
} from "@/components/ui/StatusPill"

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

function relativeTimestamp(iso: string, now = new Date()): string {
  if (!iso) return ""
  const ts = Date.parse(iso)
  if (Number.isNaN(ts)) return ""
  const diffMs = now.getTime() - ts
  const diffMin = Math.floor(diffMs / 60_000)
  if (diffMin < 1) return "just now"
  if (diffMin < 60) return `${diffMin}m ago`
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return `${diffHr}h ago`
  const diffDay = Math.floor(diffHr / 24)
  return `${diffDay}d ago`
}

interface RunListItemProps {
  run: RunLogSummary
  onClick: (run: RunLogSummary) => void
}

export function RunListItem({ run, onClick }: RunListItemProps) {
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
      <span className="flex-1 truncate text-body">
        {truncate(run.goal || "(no goal text)")}
      </span>
      <span className="text-meta text-fg-tertiary">
        {relativeTimestamp(run.started_at)}
      </span>
    </button>
  )
}
