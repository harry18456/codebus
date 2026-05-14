import { useState } from "react"
import type { VerbBanner, VerbEvent } from "@/lib/ipc"

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
  if (event.kind === "stream") {
    if (event.data.kind === "tool_use") {
      const name = event.data.name
      if (name === "Write" || name === "Edit") {
        const path = writeEditPath(event.data.input)
        return (
          <div
            data-testid="stream-tool-use"
            data-tool={name}
            className="font-mono text-[12px] text-fg"
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
          className="font-mono text-[12px] text-fg"
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
        className="text-[12px] italic text-fg-tertiary"
      >
        {bannerLabel(event.data)}
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
  const lines = text.split("\n")
  const firstLine = lines[0] ?? ""
  const restLines = lines.slice(1)
  const moreCount = restLines.length

  return (
    <div
      data-testid="thought-item"
      className="font-mono text-[12px] text-fg-secondary"
    >
      <div>
        <span aria-hidden="true">🤔</span> {firstLine}
        {moreCount > 0 && !open && (
          <button
            type="button"
            data-testid="thought-expand"
            onClick={() => setOpen(true)}
            className="ml-2 text-[11px] text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
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
            className="ml-5 text-[11px] text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
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

/** Pretty-print `input.file_path` for Write/Edit events. */
function writeEditPath(input: unknown): string {
  if (input === null || typeof input !== "object") return ""
  const fp = (input as Record<string, unknown>).file_path
  if (typeof fp !== "string") return ""
  return fp.replace(/\\/g, "/")
}

/** Generic tool-input summarizer for non-Write/Edit tools. */
function summarizeToolInput(input: unknown): string {
  if (input === null || typeof input !== "object") return ""
  const obj = input as Record<string, unknown>
  if (typeof obj.file_path === "string") {
    const parts = obj.file_path.split(/[\\/]/)
    return parts[parts.length - 1] || obj.file_path
  }
  if (typeof obj.pattern === "string") return `"${obj.pattern}"`
  if (typeof obj.command === "string") {
    return obj.command.length > 80
      ? `${obj.command.slice(0, 79)}…`
      : obj.command
  }
  return ""
}

function bannerLabel(banner: VerbBanner): string {
  switch (banner.kind) {
    case "start":
      return `🚌 來囉來囉~ CodeBus 駛入 ${normalizePath(banner.repo_path)}...`
    case "goal":
      return `🎯 任務目標：${banner.goal_text}`
    case "sync_start":
      return "🔄 同步 source → raw/code..."
    case "sync_done":
      return `✓ 同步完成 (${banner.files} 檔, ${banner.mib.toFixed(1)} MiB, ${banner.elapsed_ms} ms)`
    case "pii_summary":
      return `🛡 PII：${banner.scanner}, scanned ${banner.scanned}, hits ${banner.hits}, action ${banner.action}`
    case "lint_start":
      return "🔍 lint 中..."
    case "lint_done":
      return `✓ lint 完成 (${banner.errors} errors, ${banner.warns} warns, ${banner.elapsed_ms} ms)`
    case "commit_done":
      return `🚏 commit ${banner.sha7}`
    case "done":
      return "🎉 完成"
    case "hint":
      return "💡 提示"
  }
}

function normalizePath(p: string): string {
  return p.replace(/\\/g, "/")
}
