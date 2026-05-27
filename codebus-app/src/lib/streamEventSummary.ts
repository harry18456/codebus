import type { TFunction } from "@/i18n/useT"
import type { ToolKind, VerbBanner, VerbEvent } from "@/lib/ipc"

/**
 * Shared stream-event summary helpers. Extracted from `ActivityStreamItem`
 * so both the Activity stream view and the Goals-list running-row tail
 * project a `VerbEvent` to the same one-line representation.
 *
 * `summarizeVerbEvent` is the one-stop facade: returns the one-line
 * string for events with a summary (banner / tool_use) or `null` for
 * events that intentionally have no one-line projection (thought /
 * lifecycle).
 */

/** Pretty-print `input.file_path` for Write/Edit events with forward slashes. */
export function writeEditPath(input: unknown): string {
  if (input === null || typeof input !== "object") return ""
  const fp = (input as Record<string, unknown>).file_path
  if (typeof fp !== "string") return ""
  return fp.replace(/\\/g, "/")
}

/**
 * Strip a Codex-style shell wrapper (`powershell.exe -Command "..."`,
 * `/bin/sh -c "..."`, `bash -c '...'`) and return the inner
 * user-authored command, with one layer of matching outer quotes
 * removed. Returns the raw input unchanged when no wrapper is
 * recognized. Pure / side-effect-free.
 *
 * The two regexes are anchored at start-of-string and contain no
 * nested quantifiers, so they cannot catastrophically backtrack on
 * adversarial input. The `s` (dotAll) flag lets `.+` span newlines so
 * PowerShell here-strings (`-Command "@'\n...\n'@"`) survive.
 */
export function extractInnerCommand(raw: string): string {
  const trimmed = raw.trimStart()
  const ps = trimmed.match(
    /^(?:"[^"]*powershell\.exe"|[^\s"]*powershell\.exe)(?:\s+-\w+)*\s+-Command\s+(.+)$/is,
  )
  if (ps) return stripOuterQuotes(ps[1].trimEnd())
  const sh = trimmed.match(/^(?:\/[\w./-]+\/)?(?:bash|sh)\s+-c\s+(.+)$/is)
  if (sh) return stripOuterQuotes(sh[1].trimEnd())
  return raw
}

function stripOuterQuotes(s: string): string {
  if (s.length < 2) return s
  const first = s[0]
  const last = s[s.length - 1]
  if ((first === '"' || first === "'") && first === last) {
    return s.slice(1, -1)
  }
  return s
}

/** Generic tool-input summarizer for non-Write/Edit tools. */
export function summarizeToolInput(input: unknown): string {
  if (input === null || typeof input !== "object") return ""
  const obj = input as Record<string, unknown>
  if (typeof obj.file_path === "string") {
    const parts = obj.file_path.split(/[\\/]/)
    return parts[parts.length - 1] || obj.file_path
  }
  if (typeof obj.pattern === "string") return `"${obj.pattern}"`
  if (typeof obj.command === "string") {
    const inner = extractInnerCommand(obj.command)
    return inner.length > 80 ? `${inner.slice(0, 79)}…` : inner
  }
  return ""
}

export function bannerLabel(banner: VerbBanner, t: TFunction): string {
  switch (banner.kind) {
    case "start":
      return t("workspace.activity.banner.start", {
        path: normalizePath(banner.repo_path),
      })
    case "goal":
      return t("workspace.activity.banner.goal", {
        goalText: banner.goal_text,
      })
    case "sync_start":
      return t("workspace.activity.banner.syncStart")
    case "sync_done":
      return t("workspace.activity.banner.syncDone", {
        files: banner.files,
        mib: banner.mib.toFixed(1),
        elapsedMs: banner.elapsed_ms,
      })
    case "pii_summary":
      return t("workspace.activity.banner.piiSummary", {
        scanner: banner.scanner,
        scanned: banner.scanned,
        hits: banner.hits,
        action: banner.action,
      })
    case "lint_start":
      return t("workspace.activity.banner.lintStart")
    case "lint_done":
      return t("workspace.activity.banner.lintDone", {
        errors: banner.errors,
        warns: banner.warns,
        elapsedMs: banner.elapsed_ms,
      })
    case "commit_done":
      return t("workspace.activity.banner.commitDone", { sha7: banner.sha7 })
    case "done":
      return t("workspace.activity.banner.done")
    case "hint":
      return t("workspace.activity.banner.hint")
  }
}

function normalizePath(p: string): string {
  return p.replace(/\\/g, "/")
}

/**
 * Mono ASCII / single-glyph icon prefix for an Activity stream cluster
 * heading or a leaf tool row, mirroring the AUDIT W4 § Tool kind → cluster
 * mapping table (design v1.5 spec lock 2026-05-26):
 *
 *   Read              → 📄
 *   Glob              → 🗂
 *   Grep              → 🔍
 *   Shell tool_kind=read    → $_
 *   Shell tool_kind=inspect → $?
 *   Write / Edit      → ✎
 *   Shell tool_kind=mutation → $!
 *
 * Unknown tool names without `tool_kind` fall back to `$?` (Inspect-safe)
 * — same fallback policy as `classifyToolPhase` in clusterTimeline.ts.
 *
 * This helper is the single source of truth for the cluster-phase glyph
 * vocabulary. The cluster heading (`ActivityCluster`) and any future
 * per-leaf icon rendering SHALL consume it instead of inlining literals.
 */
export function toolIconPrefix(name: string, tool_kind?: ToolKind): string {
  if (name === "Read") return "📄"
  if (name === "Glob") return "🗂"
  if (name === "Grep") return "🔍"
  if (name === "Write" || name === "Edit") return "✎"
  // Shell / Bash / other tool names dispatch on tool_kind.
  if (tool_kind === "mutation" || tool_kind === "other_write") return "$!"
  if (tool_kind === "read" || tool_kind === "other_read") return "$_"
  // Inspect, undefined, or anything not enumerated — inspect-safe fallback.
  return "$?"
}

/**
 * One-stop facade: project a `VerbEvent` to a single-line string for
 * UI surfaces that show "what is this run doing right now" without the
 * full stream timeline. Returns `null` for events that have no useful
 * one-line projection (thought chunks, lifecycle events) — callers
 * SHALL render a placeholder instead.
 */
export function summarizeVerbEvent(
  event: VerbEvent,
  t: TFunction,
): string | null {
  if (event.kind === "banner") {
    return bannerLabel(event.data, t)
  }
  if (event.kind === "stream") {
    if (event.data.kind === "tool_use") {
      const name = event.data.name
      if (name === "Write" || name === "Edit") {
        const path = writeEditPath(event.data.input)
        return `✍️ ${path || name}`
      }
      const summary = summarizeToolInput(event.data.input)
      return summary ? `🛠️ ${name} · ${summary}` : `🛠️ ${name}`
    }
    return null
  }
  return null
}
