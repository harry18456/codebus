import { useState } from "react"

import { Button } from "@/components/ui/button"
import type { RunDetail } from "@/lib/ipc"

import { NewGoalModal } from "./NewGoalModal"

interface RunDetailCancelledProps {
  detail: RunDetail
  vaultPath: string
  onBack: () => void
  /**
   * Called after the Retry-with-same-goal modal's spawn resolves
   * with the new RunId so the parent can switch directly to the new
   * Running detail view.
   */
  onRetrySpawned?: (runId: string) => void
}

/**
 * Spec: app-workspace § Run Detail Views — Cancelled and Interrupted.
 *
 * Shared layout: header (← back + goal + ⏹ Cancelled badge),
 * warning block, Partial timeline categorized as reading / writing /
 * other, and a `[Retry with same goal]` button that pre-fills the
 * New Goal modal but DOES NOT spawn a new run by itself — the user
 * still has to click `Run` in the modal.
 */
export function RunDetailCancelled({
  detail,
  vaultPath,
  onBack,
  onRetrySpawned,
}: RunDetailCancelledProps) {
  const [retryOpen, setRetryOpen] = useState(false)
  const summary = detail.summary
  const partial = partialTimeline(detail)

  return (
    <div data-testid="run-detail-cancelled" className="flex h-full flex-col">
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
          ← back
        </button>
        <span
          data-tauri-drag-region
          className="flex-1 truncate text-body"
        >
          {summary.goal}
        </span>
        <span
          data-tauri-drag-region
          data-testid="cancelled-badge"
          className="rounded-full bg-warning/20 px-2 py-0.5 text-meta text-warning"
        >
          ⏹ Cancelled
        </span>
      </header>
      <div
        data-testid="cancelled-warning"
        className="m-3 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-meta text-warning"
      >
        Wiki has uncommitted changes — not auto-committed. Review in terminal if needed.
      </div>
      <PartialTimeline timeline={partial} />
      <footer className="flex justify-end border-t border-border px-3 py-2">
        <Button
          data-testid="retry-button"
          onClick={() => setRetryOpen(true)}
        >
          Retry with same goal
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

interface RunDetailInterruptedProps {
  detail: RunDetail
  vaultPath: string
  onBack: () => void
  onRetrySpawned?: (runId: string) => void
}

export function RunDetailInterrupted({
  detail,
  vaultPath,
  onBack,
  onRetrySpawned,
}: RunDetailInterruptedProps) {
  const [retryOpen, setRetryOpen] = useState(false)
  const summary = detail.summary
  const partial = partialTimeline(detail)

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
          ← back
        </button>
        <span
          data-tauri-drag-region
          className="flex-1 truncate text-body"
        >
          {summary.goal}
        </span>
        <span
          data-tauri-drag-region
          data-testid="interrupted-badge"
          className="rounded-full bg-warning/20 px-2 py-0.5 text-meta text-warning"
        >
          ⚠ Interrupted
        </span>
      </header>
      <div
        data-testid="interrupted-warning"
        className="m-3 rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-meta text-warning"
      >
        App was closed before this goal finished. Wiki state may be partial — review in terminal if needed.
      </div>
      <PartialTimeline timeline={partial} />
      <footer className="flex justify-end border-t border-border px-3 py-2">
        <Button
          data-testid="retry-button"
          onClick={() => setRetryOpen(true)}
        >
          Retry with same goal
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
  return (
    <div className="px-3 pb-3 text-meta text-fg-secondary">
      <h3 className="mb-1 text-meta font-semibold uppercase tracking-wide text-fg-tertiary">
        Partial timeline
      </h3>
      <p>
        reading {timeline.reading} · writing {timeline.writing} · other {timeline.other}
      </p>
    </div>
  )
}
