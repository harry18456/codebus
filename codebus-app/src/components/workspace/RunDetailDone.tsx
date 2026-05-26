import { useMemo, useState } from "react"

import type { EventEnvelope, RunDetail, VerbEvent } from "@/lib/ipc"
import { useT, type TFunction } from "@/i18n/useT"
import { useWikiStore } from "@/store/wiki"

import {
  ActivityStreamItem,
  ThoughtItem,
  foldTimeline,
} from "./ActivityStreamItem"

interface RunDetailDoneProps {
  detail: RunDetail
  onBack: () => void
  /** Switch to the Wiki tab and load the given page slug. */
  onSelectPage: (slug: string) => void
}

/**
 * Spec: app-workspace § Run Detail Views — Done.
 *
 * Header (← back + goal + ✓ Done badge), metadata (duration, tokens),
 * Covered pages block (grouped by verb phase, derived from ToolUse
 * Write/Edit events under `wiki/`), Lint stats line, Activity summary
 * (tool-use counts grouped by verb phase), AND a collapsible Run
 * details block (full events.jsonl replay using ActivityStreamItem +
 * ThoughtItem fold).
 */
export function RunDetailDone({ detail, onBack, onSelectPage }: RunDetailDoneProps) {
  const t = useT()
  const summary = detail.summary
  const pages = useWikiStore((s) => s.pages)
  const [detailsOpen, setDetailsOpen] = useState(false)

  const phases = useMemo(() => phasesFromEvents(detail.events), [
    detail.events,
  ])
  const timeline = useMemo<readonly VerbEvent[]>(
    () => detail.events.map((env) => env.event),
    [detail.events],
  )
  const foldedTimeline = useMemo(() => foldTimeline(timeline), [timeline])

  const totalCoveredPages = phases.flatMap((p) => p.coveredPages).length
  const totalToolUses = phases.reduce(
    (acc, p) => acc + p.toolCounts.reduce((s, c) => s + c.count, 0),
    0,
  )

  const durationSec = computeDurationSec(summary.started_at, summary.finished_at)
  const totalTokens =
    summary.tokens.input_tokens + summary.tokens.output_tokens

  return (
    <div data-testid="run-detail-done" className="flex h-full flex-col">
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
          {summary.goal}
        </span>
        <span
          data-tauri-drag-region
          data-testid="done-badge"
          className="rounded-full bg-success/20 px-2 py-0.5 text-meta text-success"
        >
          {t("workspace.runDetail.doneBadge")}
        </span>
      </header>
      <div className="border-b border-border px-3 py-1.5 text-meta text-fg-tertiary">
        {durationSec}s · {totalTokens} tokens
      </div>
      <div className="flex-1 overflow-auto p-3">
        <section>
          <h3 className="mb-2 text-meta font-semibold uppercase tracking-wide text-fg-tertiary">
            {t("workspace.runDetail.coveredPagesLabel")}
          </h3>
          {totalCoveredPages === 0 ? (
            <p className="text-meta text-fg-tertiary">
              {t("workspace.runDetail.coveredPagesEmpty")}
            </p>
          ) : (
            <div className="flex flex-col gap-3">
              {phases.map((phase) => (
                <div key={`covered-${phase.verb}`}>
                  <h4
                    data-testid={`covered-phase-${phase.verb}`}
                    className="mb-1 text-meta text-fg-tertiary"
                  >
                    {phaseLabel(t, phase.verb)}
                  </h4>
                  {phase.coveredPages.length === 0 ? (
                    <p className="ml-3 text-meta text-fg-tertiary">
                      {t("workspace.runDetail.coveredPagesPhaseEmpty")}
                    </p>
                  ) : (
                    <ul className="ml-3 flex flex-col gap-1">
                      {phase.coveredPages.map((slug) => (
                        <li key={`${phase.verb}-${slug}`}>
                          <button
                            type="button"
                            data-testid={`covered-page-${slug}`}
                            data-slug={slug}
                            onClick={() => onSelectPage(slug)}
                            style={{ color: "#7c8cff" }}
                            className="text-left text-meta hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
                          >
                            {pages[slug]?.title ?? slug}
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>
        <section className="mt-4">
          <h3 className="mb-2 text-meta font-semibold uppercase tracking-wide text-fg-tertiary">
            {t("workspace.runDetail.lintLabel")}
          </h3>
          <p className="text-meta text-fg-secondary">
            {summary.lint_error_count} errors · {summary.lint_warn_count} warnings
          </p>
        </section>
        <section className="mt-4">
          <h3 className="mb-2 text-meta font-semibold uppercase tracking-wide text-fg-tertiary">
            {t("workspace.runDetail.activitySummaryLabel")}
          </h3>
          {totalToolUses === 0 && phases.length === 0 ? (
            <p className="text-meta text-fg-tertiary">—</p>
          ) : (
            <div
              data-testid="activity-summary"
              className="flex flex-col gap-3"
            >
              {phases.map((phase) => (
                <div key={`activity-${phase.verb}`}>
                  <h4
                    data-testid={`activity-phase-${phase.verb}`}
                    className="mb-1 text-meta text-fg-tertiary"
                  >
                    {phaseLabel(t, phase.verb)}
                  </h4>
                  {phase.toolCounts.length === 0 ? (
                    <p className="ml-3 text-meta text-fg-tertiary">
                      {t("workspace.runDetail.phaseEmptyHint")}
                    </p>
                  ) : (
                    <ul className="ml-3 flex flex-col gap-0.5 text-meta text-fg-secondary">
                      {phase.toolCounts.map(({ tool, count }) => (
                        <li
                          key={`${phase.verb}-${tool}`}
                          data-testid={`activity-summary-${phase.verb}-${tool}`}
                          className="font-mono"
                        >
                          {toolLabel(t, tool, count)}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>
        <section className="mt-4">
          <button
            type="button"
            data-testid="run-details-toggle"
            aria-expanded={detailsOpen}
            onClick={() => setDetailsOpen((v) => !v)}
            className="mb-2 text-left text-meta font-semibold uppercase tracking-wide text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            <span className="text-accent">
              {detailsOpen
                ? t("workspace.runDetail.hideDetails")
                : t("workspace.runDetail.showDetails")}
            </span>
          </button>
          {detailsOpen && (
            <div
              data-testid="run-details-block"
              className="flex flex-col gap-0.5 rounded-md border border-border bg-bg-sunken p-3"
            >
              {foldedTimeline.map((item, i) =>
                item.kind === "thought_block" ? (
                  <ThoughtItem key={i} text={item.text} />
                ) : (
                  <ActivityStreamItem key={i} event={item.event} />
                ),
              )}
            </div>
          )}
        </section>
      </div>
    </div>
  )
}

interface ToolCount {
  tool: string
  count: number
}

interface PhaseSummary {
  verb: string
  toolCounts: ToolCount[]
  coveredPages: string[]
}

/**
 * Slice the events file into verb phases using
 * `VerbLifecycleEvent::SpawnStart` / `SpawnEnd` markers, then per
 * phase tally ToolUse counts AND extract Covered pages.
 *
 * Multiple spawns of the same verb (e.g., fix loop iterates twice)
 * merge into one phase bucket so the user sees a single `fix` heading.
 *
 * Tool use observed outside any spawn pair (legacy events, init-time
 * banners) is bucketed under a synthetic phase named `""` (empty) and
 * suppressed from rendering — those cases should not appear in
 * well-formed events files.
 */
function phasesFromEvents(events: EventEnvelope[]): PhaseSummary[] {
  const order: string[] = []
  const buckets: Record<string, { toolCounts: Record<string, number>; coveredPages: Set<string> }> = {}
  let currentVerb: string | null = null

  const ensure = (verb: string) => {
    if (!(verb in buckets)) {
      buckets[verb] = { toolCounts: {}, coveredPages: new Set<string>() }
      order.push(verb)
    }
    return buckets[verb]
  }

  for (const env of events) {
    const event = env.event
    if (event.kind === "lifecycle") {
      const data = event.data
      if (data.kind === "spawn_start") {
        currentVerb = data.verb
        ensure(currentVerb)
        continue
      }
      if (data.kind === "spawn_end") {
        currentVerb = null
        continue
      }
      continue
    }
    if (event.kind !== "stream") continue
    if (event.data.kind !== "tool_use") continue
    if (currentVerb === null) continue
    const bucket = ensure(currentVerb)
    const toolName = event.data.name
    bucket.toolCounts[toolName] = (bucket.toolCounts[toolName] ?? 0) + 1

    if (toolName === "Write" || toolName === "Edit") {
      const slug = extractWikiSlug(event.data.input)
      if (slug) bucket.coveredPages.add(slug)
    }
  }

  return order
    .filter((verb) => verb.length > 0)
    .map((verb) => ({
      verb,
      toolCounts: Object.entries(buckets[verb].toolCounts)
        .map(([tool, count]) => ({ tool, count }))
        .sort((a, b) => b.count - a.count),
      coveredPages: [...buckets[verb].coveredPages],
    }))
}

function extractWikiSlug(input: unknown): string | null {
  if (input === null || typeof input !== "object") return null
  const fp = (input as Record<string, unknown>).file_path
  if (typeof fp !== "string") return null
  const parts = fp.replace(/\\/g, "/").split("/")
  if (!parts.includes("wiki")) return null
  const last = parts[parts.length - 1]
  if (!last.endsWith(".md")) return null
  return last.slice(0, -3)
}

function phaseLabel(t: TFunction, verb: string): string {
  switch (verb) {
    case "goal":
      return t("workspace.runDetail.phaseGoal")
    case "fix":
      return t("workspace.runDetail.phaseFix")
    case "query":
      return t("workspace.runDetail.phaseQuery")
    case "chat":
      return t("workspace.runDetail.phaseChat")
    default:
      return t("workspace.runDetail.phaseOther", { verb })
  }
}

function toolLabel(t: TFunction, tool: string, n: number): string {
  switch (tool) {
    case "Read":
      return t("workspace.runDetail.toolReadLine", { n })
    case "Glob":
      return t("workspace.runDetail.toolGlobLine", { n })
    case "Grep":
      return t("workspace.runDetail.toolGrepLine", { n })
    case "Write":
      return t("workspace.runDetail.toolWriteLine", { n })
    case "Edit":
      return t("workspace.runDetail.toolEditLine", { n })
    default:
      return t("workspace.runDetail.toolOtherLine", { n, tool })
  }
}

function computeDurationSec(start: string, end: string): number {
  const s = Date.parse(start)
  const e = Date.parse(end || start)
  if (!Number.isFinite(s) || !Number.isFinite(e)) return 0
  return Math.max(0, Math.floor((e - s) / 1000))
}
