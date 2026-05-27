import type { ToolKind, VerbEvent } from "@/lib/ipc"
import type { TimelineItem } from "@/components/workspace/ActivityStreamItem"

/**
 * Activity stream 2-phase cluster projection.
 *
 * Spec: app-workspace § "Activity Stream Two-Phase Cluster Rendering"
 * (locked by design v1.5 + AUDIT W4 / X1 series, 2026-05-26).
 *
 * Two phases:
 *   - `reading_codebase` — intake (Read / Glob / Grep / Shell read / inspect)
 *   - `writing_wiki` — mutation (Write / Edit / Shell mutation)
 *
 * Tool name → phase classification (from Pre-apply Task 1.3 校準):
 *
 *   | provider | name                                | phase            |
 *   | -------- | ----------------------------------- | ---------------- |
 *   | Claude   | "Read" / "Glob" / "Grep"            | reading_codebase |
 *   | Claude   | "Write" / "Edit"                    | writing_wiki     |
 *   | Claude   | "Bash" (look at tool_kind)          | per tool_kind    |
 *   | Codex    | "Shell" (always; look at tool_kind) | per tool_kind    |
 *
 *   tool_kind on Shell/Bash:
 *     "read" / "inspect" / "other_read" → reading_codebase
 *     "mutation" / "other_write"        → writing_wiki
 *     undefined                         → reading_codebase  (Inspect fallback)
 *
 * Codex provider never emits `tool_kind` today (codex_parser.rs constructs
 * StreamEvent::ToolUse from `command_execution` without a kind field) — all
 * Codex shells fall back to reading_codebase. Acceptable trade-off pending
 * a follow-up change. See design.md Open Questions.
 *
 * Cluster boundary rules (locked AUDIT W4 § 5 條細節):
 *   1. Banner is NOT clustered (flat between clusters)
 *   2. Thought block is NOT clustered AND breaks an open cluster
 *   3. Phase change breaks the open cluster AND opens a new one
 *   4. Clusters MAY repeat (read → thought → read → write produces three
 *      clusters with the thought between the first two)
 *   5. Cluster count includes ONLY tool_use rows (not thought blocks)
 */

export type ClusterPhase = "reading_codebase" | "writing_wiki"

export type ClusterItem =
  | {
      kind: "cluster"
      phase: ClusterPhase
      events: VerbEvent[]
      count: number
    }
  | { kind: "event"; event: VerbEvent }
  | { kind: "thought_block"; text: string }

const READING_TOOL_NAMES = new Set(["Read", "Glob", "Grep"])
const WRITING_TOOL_NAMES = new Set(["Write", "Edit"])

const READING_TOOL_KINDS = new Set<ToolKind>(["read", "inspect", "other_read"])
const WRITING_TOOL_KINDS = new Set<ToolKind>(["mutation", "other_write"])

/**
 * Classify a single VerbEvent into its cluster phase. Returns `null` for
 * events that cannot belong to a cluster (banners, thoughts, tool_results,
 * usage, lifecycle).
 *
 * Legacy events with missing `tool_kind` on Shell/Bash classify as
 * `reading_codebase` per the Inspect fallback decision (design.md
 * "Fallback default 走 Inspect 不走 Mutation").
 */
export function classifyToolPhase(event: VerbEvent): ClusterPhase | null {
  if (event.kind !== "stream") return null
  if (event.data.kind !== "tool_use") return null
  const { name, tool_kind } = event.data
  if (READING_TOOL_NAMES.has(name)) return "reading_codebase"
  if (WRITING_TOOL_NAMES.has(name)) return "writing_wiki"
  // Shell / Bash / any other tool: consult tool_kind, with Inspect fallback.
  if (tool_kind === undefined) return "reading_codebase"
  if (READING_TOOL_KINDS.has(tool_kind)) return "reading_codebase"
  if (WRITING_TOOL_KINDS.has(tool_kind)) return "writing_wiki"
  return "reading_codebase"
}

/**
 * Project a (foldTimeline-produced) TimelineItem[] into a ClusterItem[].
 *
 * - Consecutive tool_use events of the same phase fold into a single
 *   `cluster` item (count = number of tool_use rows).
 * - Banner events and thought blocks pass through flat AND break any open
 *   cluster.
 * - Phase changes break the open cluster and open a new one.
 *
 * Pure function: input arrays are never mutated.
 */
export function projectClusters(items: readonly TimelineItem[]): ClusterItem[] {
  const out: ClusterItem[] = []
  let openPhase: ClusterPhase | null = null
  let openEvents: VerbEvent[] = []

  const flush = () => {
    if (openPhase !== null && openEvents.length > 0) {
      out.push({
        kind: "cluster",
        phase: openPhase,
        events: openEvents,
        count: openEvents.length,
      })
    }
    openPhase = null
    openEvents = []
  }

  for (const item of items) {
    if (item.kind === "thought_block") {
      flush()
      out.push(item)
      continue
    }
    // item.kind === "event"
    const phase = classifyToolPhase(item.event)
    if (phase === null) {
      // Non-clustering event (banner / non-tool stream / lifecycle).
      flush()
      out.push(item)
      continue
    }
    if (openPhase !== null && openPhase !== phase) {
      flush()
    }
    openPhase = phase
    openEvents = [...openEvents, item.event]
  }
  flush()
  return out
}
