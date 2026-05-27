import { useEffect, useMemo, useState } from "react"

import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/ui/StatusPill"
import type { VerbEvent } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"
import { useT } from "@/i18n/useT"

import { ActivityStreamItem, ThoughtItem, foldTimeline } from "./ActivityStreamItem"
import { ActivityCluster } from "./ActivityCluster"
import { projectClusters } from "@/lib/clusterTimeline"

/** Stable empty reference so the useMemo dep stays referentially equal
 *  on the "no activeRun" branch. */
const EMPTY_EVENTS: readonly VerbEvent[] = []

interface RunDetailRunningProps {
  onBack: () => void
}

/**
 * Spec: app-workspace § Run Detail Views — Running.
 *
 * Header (← back + goal + ⏺ Running badge), metadata line (elapsed
 * time live-updated every second + accumulated tokens), Activity
 * stream block (tool_use as one-liners, Thought chunks concatenated
 * into a single trailing block), and the ⏹ Cancel button.
 */
export function RunDetailRunning({ onBack }: RunDetailRunningProps) {
  const t = useT()
  const activeRun = useGoalsStore((s) => s.activeRun)
  const cancelGoal = useGoalsStore((s) => s.cancelGoal)
  const [now, setNow] = useState(() => Date.now())

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(id)
  }, [])

  // Fold consecutive Thought chunks inline, preserving their original
  // position relative to ToolUse / Banner events (spec scenario
  // "Thought chunks fold inline into a single timeline item"). Run
  // memoization BEFORE any conditional early-return so the hook order
  // stays stable across renders.
  const events = activeRun?.events ?? EMPTY_EVENTS
  const timeline = useMemo(() => foldTimeline(events), [events])
  // Project the folded timeline into 2-phase clusters
  // (READING CODEBASE / WRITING WIKI) per app-workspace § "Activity
  // Stream Two-Phase Cluster Rendering". Running view → clusters
  // default-open (`terminal={false}`).
  const clusters = useMemo(() => projectClusters(timeline), [timeline])

  if (!activeRun) return null

  const startedMs = Date.parse(activeRun.startedAt)
  const elapsedSec = Number.isFinite(startedMs)
    ? Math.max(0, Math.floor((now - startedMs) / 1000))
    : 0

  const accumulatedTokens = collectTokens(activeRun.events)

  return (
    <div
      data-testid="run-detail-running"
      className="flex h-full flex-col"
    >
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
        <span
          data-tauri-drag-region
          className="flex-1 truncate text-body"
        >
          {activeRun.goal}
        </span>
        <span data-tauri-drag-region data-testid="running-badge">
          <StatusPill status="running" variant="pill" />
        </span>
        {/*
          Cancel sits in the header right-hand action slot — NOT a
          `<footer>` — so it never visually collides with the
          bottom-right ChatWidget bubble. The wrapper deliberately omits
          `data-tauri-drag-region`; otherwise Tauri's window-drag
          handler swallows pointerdown on this region and the button
          becomes unclickable. The neighbouring `running-badge` keeps
          the drag attribute, so the header overall remains draggable.
        */}
        <span>
          <Button
            data-testid="cancel-button"
            variant="danger"
            size="sm"
            disabled={activeRun.cancelling}
            onClick={() => void cancelGoal(activeRun.runId)}
          >
            {activeRun.cancelling
              ? t("workspace.runDetail.cancellingButton")
              : t("workspace.runDetail.cancelButton")}
          </Button>
        </span>
      </header>
      <div
        data-testid="run-detail-metadata"
        className="border-b border-border px-3 py-1.5 text-meta text-fg-tertiary"
      >
        {elapsedSec}s elapsed · {accumulatedTokens} tokens
      </div>
      <div
        data-testid="activity-stream"
        className="flex flex-1 flex-col gap-0.5 overflow-auto p-3"
      >
        {clusters.map((item, i) => {
          if (item.kind === "thought_block") {
            return <ThoughtItem key={i} text={item.text} />
          }
          if (item.kind === "event") {
            return <ActivityStreamItem key={i} event={item.event} />
          }
          // item.kind === "cluster"
          return (
            <ActivityCluster
              key={i}
              phase={item.phase}
              events={item.events}
              count={item.count}
              terminal={false}
            >
              {item.events.map((evt, j) => (
                <ActivityStreamItem key={j} event={evt} />
              ))}
            </ActivityCluster>
          )
        })}
      </div>
    </div>
  )
}

function collectTokens(events: VerbEvent[]): number {
  return events.reduce((acc, e) => {
    if (e.kind === "stream" && e.data.kind === "usage") {
      return acc + e.data.input_tokens + e.data.output_tokens
    }
    return acc
  }, 0)
}
