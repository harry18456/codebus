import { useId, useState, type ReactNode } from "react"

import type { VerbEvent } from "@/lib/ipc"
import type { ClusterPhase } from "@/lib/clusterTimeline"
import { useT } from "@/i18n/useT"

/**
 * `ActivityCluster` — wraps consecutive same-phase tool_use rows into a
 * single collapsible row.
 *
 * Spec: app-workspace § "Activity Stream Two-Phase Cluster Rendering".
 *
 * - Heading is a `<button>` (a11y: aria-expanded + aria-controls).
 * - Default open while `terminal` is false (run is still running); default
 *   closed once the surrounding run reaches a terminal state. Toggle is
 *   per-component local state — switching `terminal` re-mounts the
 *   component and resets the toggle (accepted trade-off, see design.md
 *   Risks/Trade-offs).
 * - Collapsed summary line (e.g. `Reading codebase · 12 reads · 195 shell
 *   · 6.2s`) renders ONLY in terminal state.
 * - Heading icon prefix uses the mono ASCII / single-glyph table from
 *   AUDIT W4 design v1.5 lock — `📄` for reading clusters, `✎` for writing
 *   clusters. The cluster heading SHALL NOT contain the generic `🛠️`
 *   marker (that belongs to individual leaf rows).
 */

interface ActivityClusterProps {
  phase: ClusterPhase
  events: VerbEvent[]
  count: number
  terminal: boolean
  /**
   * Elapsed milliseconds for the cluster's duration. Optional; when
   * omitted the summary renders `0.0` seconds. The caller is responsible
   * for measuring the cluster's start / end timestamps if it wants
   * accurate elapsed reporting.
   */
  elapsedMs?: number
  children: ReactNode
}

const READING_PREFIX = "📄"
const WRITING_PREFIX = "✎"

const READING_TOOL_NAMES = new Set(["Read", "Glob", "Grep"])

function isShellLike(name: string): boolean {
  return !READING_TOOL_NAMES.has(name) && name !== "Write" && name !== "Edit"
}

function computeReadingSummary(events: VerbEvent[]): {
  reads: number
  shell: number
} {
  let reads = 0
  let shell = 0
  for (const e of events) {
    if (e.kind !== "stream" || e.data.kind !== "tool_use") continue
    if (READING_TOOL_NAMES.has(e.data.name)) {
      reads += 1
    } else if (isShellLike(e.data.name)) {
      shell += 1
    }
  }
  return { reads, shell }
}

function computeWritingSummary(events: VerbEvent[]): {
  new: number
  updated: number
} {
  let created = 0
  let updated = 0
  for (const e of events) {
    if (e.kind !== "stream" || e.data.kind !== "tool_use") continue
    if (e.data.name === "Write") {
      created += 1
    } else if (e.data.name === "Edit") {
      updated += 1
    }
  }
  return { new: created, updated }
}

function elapsedSecondsString(elapsedMs: number | undefined): string {
  const seconds = (elapsedMs ?? 0) / 1000
  return seconds.toFixed(1)
}

export function ActivityCluster({
  phase,
  events,
  count,
  terminal,
  elapsedMs,
  children,
}: ActivityClusterProps) {
  const t = useT()
  // Cluster heads default open while the surrounding run is still
  // running; once the run terminates the default flips to closed.
  // Switching `terminal` re-mounts the component (parent feeds a new
  // `key`) so the toggle resets — accepted per design.md.
  const [open, setOpen] = useState<boolean>(!terminal)
  const childrenId = useId()

  const isReading = phase === "reading_codebase"
  const prefix = isReading ? READING_PREFIX : WRITING_PREFIX
  const headingKey = isReading
    ? "workspace.activity.cluster.reading.heading"
    : "workspace.activity.cluster.writing.heading"
  const headingLabel = t(headingKey)

  const summary = (() => {
    if (!terminal) return null
    const elapsedSeconds = elapsedSecondsString(elapsedMs)
    if (isReading) {
      const { reads, shell } = computeReadingSummary(events)
      return t("workspace.activity.cluster.summary.reading", {
        reads,
        shell,
        elapsedSeconds,
      })
    }
    const { new: created, updated } = computeWritingSummary(events)
    return t("workspace.activity.cluster.summary.writing", {
      new: created,
      updated,
      elapsedSeconds,
    })
  })()

  const toggleLabel = open
    ? t("workspace.activity.cluster.collapse")
    : t("workspace.activity.cluster.expand")

  return (
    <div data-testid="activity-cluster" data-phase={phase}>
      <button
        type="button"
        data-testid="activity-cluster-heading"
        aria-expanded={open}
        aria-controls={childrenId}
        aria-label={toggleLabel}
        onClick={() => setOpen((prev) => !prev)}
        className="flex w-full items-center gap-2 text-left font-mono text-meta text-fg-secondary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
      >
        <span aria-hidden="true">{open ? "▽" : "▷"}</span>
        <span aria-hidden="true">{prefix}</span>
        <span>{headingLabel}</span>
        <span data-testid="activity-cluster-count" className="text-fg-tertiary">
          ({count})
        </span>
        {summary !== null && (
          <span
            data-testid="activity-cluster-summary"
            className="ml-2 text-fg-tertiary"
          >
            {summary}
          </span>
        )}
      </button>
      {/*
        Children are always mounted (preserves DOM presence for tests +
        screen reader continuity); the `hidden` attribute is the only
        visibility gate. Re-mount on toggle would lose child component
        state and break testing-library traversal.
      */}
      <div
        id={childrenId}
        data-testid="activity-cluster-children"
        hidden={!open}
        className="ml-5"
      >
        {children}
      </div>
    </div>
  )
}
