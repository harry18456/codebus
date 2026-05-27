import { useState } from "react"
import type { VerbEvent } from "@/lib/ipc"
import { useT, type TFunction } from "@/i18n/useT"
import type { MessageKey } from "@/i18n/messages"
import {
  bannerLabel,
  summarizeToolInput,
  writeEditPath,
} from "@/lib/streamEventSummary"

/**
 * Render one element of the Activity stream.
 *
 * Spec: app-workspace § Run Detail Views — Running.
 *
 * Emoji leaders mirror CLI `render::stream_event` (with GUI-side
 * shortening — no `[Agent 思考]` / `[呼叫工具]` Chinese labels since
 * GUI already has surrounding visual context):
 *
 * - `ToolUse Write|Edit` → `✍️ <file_path>` (path only, dict shape suppressed)
 * - `ToolUse <other>`    → `🛠️ <name> · <input-summary>` (file_path → basename,
 *                          pattern → quoted, command → first 80 chars)
 * - `Thought`            → handled by `ThoughtItem` via `foldTimeline`
 *                          (consecutive Thought chunks fold to one item)
 * - `ToolResult`         → NOT rendered (internal flow signal — full
 *                          trace lives in the Done detail Run details
 *                          collapsible)
 * - `Usage`              → NOT rendered
 * - `VerbBanner::*`      → italic muted line for lifecycle context
 * - `VerbLifecycleEvent::*` → NOT rendered
 */
interface ActivityStreamItemProps {
  event: VerbEvent
}

export function ActivityStreamItem({ event }: ActivityStreamItemProps) {
  const t = useT()
  if (event.kind === "stream") {
    if (event.data.kind === "tool_use") {
      const name = event.data.name
      if (name === "Write" || name === "Edit") {
        const path = writeEditPath(event.data.input)
        return (
          <div
            data-testid="stream-tool-use"
            data-tool={name}
            className="font-mono text-meta text-fg"
          >
            ✍️ {path || name}
          </div>
        )
      }
      const summary = summarizeToolInput(event.data.input)
      return (
        <div
          data-testid="stream-tool-use"
          data-tool={name}
          className="font-mono text-meta text-fg"
        >
          🛠️ {name}
          {summary && (
            <span className="text-fg-secondary"> · {summary}</span>
          )}
        </div>
      )
    }
    return null
  }
  if (event.kind === "banner") {
    return (
      <div
        data-testid="stream-banner"
        className="text-meta italic text-fg-tertiary"
      >
        {bannerLabel(event.data, t)}
      </div>
    )
  }
  return null
}

/**
 * A folded Thought item — represents one or more consecutive
 * `StreamEvent::Thought` chunks concatenated into a single block.
 * Single-line content renders inline; multi-line content renders the
 * first line plus a `(<N> more lines ▼)` toggle.
 */
interface ThoughtItemProps {
  text: string
}

export function ThoughtItem({ text }: ThoughtItemProps) {
  const [open, setOpen] = useState(false)
  const t = useT()

  // QGEN1: `[CODEBUS_*]` sentinel markers at the start of a thought
  // block are an agent ↔ codebus-core wire protocol and MUST NOT be
  // rendered raw. Known markers map to a user-facing translation;
  // unknown markers are suppressed (return null) — failing closed is
  // safer than leaking a stray `[CODEBUS_…]` substring. Markers
  // appearing mid-block (e.g. an agent quoting its own protocol) are
  // intentionally NOT filtered. Spec: app-workspace § Activity Stream
  // Internal Sentinel Marker Filter.
  const markerResult = classifyLeadingMarker(text, t)
  if (markerResult.kind === "suppress") return null
  const renderedText =
    markerResult.kind === "translated" ? markerResult.text : text
  const lines = renderedText.split("\n")
  const firstLine = lines[0] ?? ""
  const restLines = lines.slice(1)
  const moreCount = restLines.length

  return (
    <div
      data-testid="thought-item"
      className="font-mono text-meta text-fg-secondary"
    >
      <div>
        <span aria-hidden="true">🤔</span> {firstLine}
        {moreCount > 0 && !open && (
          <button
            type="button"
            data-testid="thought-expand"
            onClick={() => setOpen(true)}
            className="ml-2 text-meta text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            ({moreCount} more line{moreCount > 1 ? "s" : ""} ▼)
          </button>
        )}
      </div>
      {open && moreCount > 0 && (
        <>
          <div
            data-testid="thought-rest"
            className="ml-5 whitespace-pre-wrap"
          >
            {restLines.join("\n")}
          </div>
          <button
            type="button"
            data-testid="thought-collapse"
            onClick={() => setOpen(false)}
            className="ml-5 text-meta text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            ▲ collapse
          </button>
        </>
      )}
    </div>
  )
}

/**
 * One element in the rendered timeline. Either a single VerbEvent
 * (banner / tool_use) or a folded thought block (one+ consecutive
 * StreamEvent::Thought chunks concatenated, in their original
 * timeline position).
 */
export type TimelineItem =
  | { kind: "event"; event: VerbEvent }
  | { kind: "thought_block"; text: string }

/**
 * Fold consecutive `Thought` events into a single `thought_block`
 * item. The fold breaks every time a non-Thought event is observed,
 * preserving the inline causality "this thought, then that tool".
 *
 * Spec: app-workspace § Run Detail Views — Running, "Thought chunks
 * fold inline into a single timeline item".
 */
export function foldTimeline(events: readonly VerbEvent[]): TimelineItem[] {
  const items: TimelineItem[] = []
  let buf: string | null = null
  const flush = () => {
    if (buf !== null) {
      items.push({ kind: "thought_block", text: buf })
      buf = null
    }
  }
  for (const event of events) {
    if (event.kind === "stream" && event.data.kind === "thought") {
      buf = (buf ?? "") + event.data.text
      continue
    }
    flush()
    items.push({ kind: "event", event })
  }
  flush()
  return items
}

/**
 * Registry of `[CODEBUS_*]` sentinel markers that have a user-facing
 * translation. The key is the marker name stripped of `CODEBUS_`
 * (e.g. `[CODEBUS_QUIZ_NO_VALIDATE]` → registry key `QUIZ_NO_VALIDATE`);
 * the value is the i18n message key. Markers not listed here are
 * suppressed by `classifyLeadingMarker`.
 *
 * Adding a new marker: register the translation in
 * `src/i18n/messages.ts` (both `en` and `zh` bundles), then add the
 * mapping here.
 */
const MARKER_I18N_KEYS: Record<string, MessageKey> = {
  QUIZ_NO_VALIDATE: "activity.marker.codebusQuizNoValidate",
}

type MarkerResult =
  | { kind: "translated"; text: string }
  | { kind: "suppress" }
  | { kind: "passthrough" }

/**
 * Classify whether `text` begins with a `[CODEBUS_*]` sentinel marker
 * and, if so, whether it has a registered translation. Pure /
 * side-effect-free. The leading-whitespace tolerance matches the spec
 * (`^\s*\[CODEBUS_...]`); a mid-block marker returns `passthrough`.
 */
export function classifyLeadingMarker(
  text: string,
  t: TFunction,
): MarkerResult {
  const m = text.match(/^\s*\[CODEBUS_([A-Z0-9_]+)\]\s*(.*)$/s)
  if (!m) return { kind: "passthrough" }
  const markerName = m[1]
  const key = MARKER_I18N_KEYS[markerName]
  if (key) return { kind: "translated", text: t(key) }
  return { kind: "suppress" }
}

