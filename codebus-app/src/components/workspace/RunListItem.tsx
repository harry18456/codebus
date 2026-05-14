import type { RunLogSummary } from "@/lib/ipc"
import { cn } from "@/lib/cn"

/**
 * One row in the Goals overview list.
 *
 * Spec: app-workspace § Goals Overview List and Filter — row icon
 * mapping table:
 *
 * | RunLog outcome                                  | Row icon |
 * | ----------------------------------------------- | -------- |
 * | (active run in progress, no RunLog row yet)     | ⚪       |
 * | succeeded                                       | ✓        |
 * | cancelled                                       | ⏹        |
 * | failed                                          | ⚠        |
 * | virtual interrupted (events have no RunLog row) | ⚠        |
 */
function outcomeIcon(outcome: string): string {
  switch (outcome) {
    case "running":
      // Mirrors the CLI's bus motif (init Start banner "🚌 來囉來囉"),
      // pulsing via Tailwind's `animate-pulse` in render.
      return "🚌"
    case "succeeded":
      return "✓"
    case "cancelled":
      return "⏹"
    case "failed":
    case "interrupted":
      return "⚠"
    default:
      return "•"
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
      <span
        aria-hidden="true"
        className={cn(
          "text-[14px]",
          run.outcome === "running" && "animate-pulse",
        )}
      >
        {outcomeIcon(run.outcome)}
      </span>
      <span className="flex-1 truncate text-[13px]">
        {truncate(run.goal || "(no goal text)")}
      </span>
      <span className="text-[11px] text-fg-tertiary">
        {relativeTimestamp(run.started_at)}
      </span>
    </button>
  )
}
