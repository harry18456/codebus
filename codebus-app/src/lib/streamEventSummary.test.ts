import { describe, expect, it } from "vitest"

import type { TFunction } from "@/i18n/useT"
import type { ToolKind, VerbBanner, VerbEvent } from "@/lib/ipc"

/**
 * Compile-time guard: `ToolKind` must remain a closed union of exactly the
 * five `snake_case` wire strings mirroring the Rust enum. If a contributor
 * widens the union without updating frontend consumers (clusterTimeline /
 * cluster icon helper), this exhaustive switch produces a `never` mismatch
 * at the type level. See agent-stream-rendering § "Stream Event Tool
 * Classification".
 */
function _assertToolKindExhaustive(k: ToolKind): "read" | "write" | "skip" {
  switch (k) {
    case "read":
    case "other_read":
      return "read"
    case "mutation":
    case "other_write":
      return "write"
    case "inspect":
      return "skip"
  }
}
void _assertToolKindExhaustive

import {
  bannerLabel,
  extractInnerCommand,
  summarizeToolInput,
  summarizeVerbEvent,
  toolIconPrefix,
  writeEditPath,
} from "./streamEventSummary"

/**
 * Identity-style stub `t` that returns the key (suitable for assertions
 * that only need to verify "the i18n call happened with key X" rather
 * than the localized string). Real i18n behavior is exercised separately
 * by `ActivityStreamItem.test.tsx` so this module's tests stay pure.
 */
const tStub: TFunction = ((key: string, vars?: Record<string, string | number>) => {
  if (!vars) return key
  const pieces = Object.entries(vars)
    .map(([k, v]) => `${k}=${v}`)
    .join(",")
  return `${key}(${pieces})`
}) as TFunction

function streamToolUse(
  name: string,
  input: unknown,
): Extract<VerbEvent, { kind: "stream" }> {
  return { kind: "stream", data: { kind: "tool_use", name, input } }
}

function streamThought(text: string): Extract<VerbEvent, { kind: "stream" }> {
  return { kind: "stream", data: { kind: "thought", text } }
}

function banner(b: VerbBanner): Extract<VerbEvent, { kind: "banner" }> {
  return { kind: "banner", data: b }
}

describe("bannerLabel", () => {
  it.each<[string, VerbBanner]>([
    ["start", { kind: "start", repo_path: "/v" }],
    ["goal", { kind: "goal", goal_text: "x" }],
    ["sync_start", { kind: "sync_start" }],
    [
      "sync_done",
      { kind: "sync_done", files: 1, mib: 0.5, elapsed_ms: 100 },
    ],
    [
      "pii_summary",
      {
        kind: "pii_summary",
        scanner: "scan",
        scanned: 1,
        hits: 0,
        action: "n",
      },
    ],
    ["lint_start", { kind: "lint_start" }],
    [
      "lint_done",
      { kind: "lint_done", errors: 0, warns: 0, elapsed_ms: 1 },
    ],
    ["commit_done", { kind: "commit_done", sha7: "abcdef1" }],
    ["done", { kind: "done", wiki_path: "/v/wiki" }],
    ["hint", { kind: "hint", wiki_path: "/v/wiki" }],
  ])("returns a non-empty string for banner kind %s", (_label, b) => {
    const out = bannerLabel(b, tStub)
    expect(typeof out).toBe("string")
    expect(out.length).toBeGreaterThan(0)
  })
})

describe("writeEditPath", () => {
  it("returns normalized forward-slash path for a string file_path", () => {
    expect(writeEditPath({ file_path: "wiki\\modules\\auth.md" })).toBe(
      "wiki/modules/auth.md",
    )
  })

  it("returns empty string for non-object input", () => {
    expect(writeEditPath(null)).toBe("")
    expect(writeEditPath(undefined)).toBe("")
    expect(writeEditPath("string")).toBe("")
  })

  it("returns empty string when file_path is missing or non-string", () => {
    expect(writeEditPath({})).toBe("")
    expect(writeEditPath({ file_path: 42 })).toBe("")
  })
})

describe("extractInnerCommand", () => {
  it.each<[string, string, string]>([
    [
      "bash -c 'echo hi'",
      "echo hi",
      "POSIX bash -c with single quotes",
    ],
    [
      'bash -c "echo hi"',
      "echo hi",
      "POSIX bash -c with double quotes",
    ],
    [
      'sh -c "ls -la"',
      "ls -la",
      "POSIX sh -c",
    ],
    [
      '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Content x"',
      "Get-Content x",
      "Windows PowerShell quoted path",
    ],
    [
      '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -NoProfile -Command "ls"',
      "ls",
      "PowerShell with -NoProfile leading flag",
    ],
    [
      "git status",
      "git status",
      "Unwrapped raw command passes through",
    ],
  ])("strips wrapper from %s", (raw, expected) => {
    expect(extractInnerCommand(raw)).toBe(expected)
  })

  it("preserves multi-line PowerShell here-string commands inside the wrapper", () => {
    const heredoc = '@\'\nMulti\nLine\n\'@'
    const wrapped = `powershell.exe -Command "${heredoc}"`
    expect(extractInnerCommand(wrapped)).toBe(heredoc)
  })
})

describe("summarizeToolInput", () => {
  it("returns basename for file_path input", () => {
    expect(summarizeToolInput({ file_path: "raw/code/auth.rs" })).toBe(
      "auth.rs",
    )
    expect(summarizeToolInput({ file_path: "auth.rs" })).toBe("auth.rs")
  })

  it("returns quoted pattern for pattern input", () => {
    expect(summarizeToolInput({ pattern: "**/*.md" })).toBe('"**/*.md"')
  })

  it("returns shell command stripped of wrapper", () => {
    expect(
      summarizeToolInput({
        command: 'bash -c "git status"',
      }),
    ).toBe("git status")
  })

  it("truncates long shell command to 80 chars ending with ellipsis", () => {
    const cmd = "a".repeat(800)
    const out = summarizeToolInput({ command: cmd })
    expect(out.length).toBe(80)
    expect(out.endsWith("…")).toBe(true)
  })

  it("returns empty string for non-object input", () => {
    expect(summarizeToolInput(null)).toBe("")
    expect(summarizeToolInput("x")).toBe("")
    expect(summarizeToolInput(undefined)).toBe("")
  })

  it("returns empty string for object without recognized keys", () => {
    expect(summarizeToolInput({ unknown: "x" })).toBe("")
  })
})

describe("summarizeVerbEvent", () => {
  it("renders Write tool_use as '✍️ <path>' with forward slashes", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Write", { file_path: "wiki\\modules\\auth.md" }),
      tStub,
    )
    expect(out).not.toBeNull()
    expect(out).toContain("✍️")
    expect(out).toContain("wiki/modules/auth.md")
  })

  it("renders Edit tool_use as '✍️ <path>'", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Edit", { file_path: "wiki/x.md" }),
      tStub,
    )
    expect(out).toContain("✍️")
    expect(out).toContain("wiki/x.md")
  })

  it("renders generic Read tool_use with name and basename", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Read", { file_path: "raw/code/auth.rs" }),
      tStub,
    )
    expect(out).toContain("🛠️")
    expect(out).toContain("Read")
    expect(out).toContain("auth.rs")
  })

  it("renders Glob tool_use with quoted pattern", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Glob", { pattern: "wiki/**/*.md" }),
      tStub,
    )
    expect(out).toContain("Glob")
    expect(out).toContain('"wiki/**/*.md"')
  })

  it("renders Bash tool_use with inner command after wrapper strip", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Bash", { command: 'bash -c "git status"' }),
      tStub,
    )
    expect(out).toContain("Bash")
    expect(out).toContain("git status")
    expect(out).not.toContain("bash -c")
  })

  it("renders banner event via bannerLabel", () => {
    const out = summarizeVerbEvent(banner({ kind: "sync_start" }), tStub)
    expect(out).toBe("workspace.activity.banner.syncStart")
  })

  it("returns null for thought event", () => {
    expect(summarizeVerbEvent(streamThought("thinking..."), tStub)).toBeNull()
  })

  it("renders tool_use without recognized input keys as just name", () => {
    const out = summarizeVerbEvent(
      streamToolUse("Read", { unknown: "x" }),
      tStub,
    )
    expect(out).toContain("Read")
  })

  it("returns null for lifecycle events", () => {
    const evt: VerbEvent = {
      kind: "lifecycle",
      data: { kind: "spawn_start", verb: "v" },
    }
    expect(summarizeVerbEvent(evt, tStub)).toBeNull()
  })
})

describe("toolIconPrefix · AUDIT W4 design v1.5 mono-icon table", () => {
  it("toolIconPrefix_returns_design_v1_5_glyph (Read)", () => {
    expect(toolIconPrefix("Read")).toBe("📄")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Glob)", () => {
    expect(toolIconPrefix("Glob")).toBe("🗂")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Grep)", () => {
    expect(toolIconPrefix("Grep")).toBe("🔍")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Write)", () => {
    expect(toolIconPrefix("Write")).toBe("✎")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Edit)", () => {
    expect(toolIconPrefix("Edit")).toBe("✎")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Shell read)", () => {
    expect(toolIconPrefix("Shell", "read")).toBe("$_")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Shell inspect)", () => {
    expect(toolIconPrefix("Shell", "inspect")).toBe("$?")
  })
  it("toolIconPrefix_returns_design_v1_5_glyph (Shell mutation)", () => {
    expect(toolIconPrefix("Shell", "mutation")).toBe("$!")
  })
  it("Bash without tool_kind falls back to inspect-safe $?", () => {
    expect(toolIconPrefix("Bash")).toBe("$?")
  })
  it("other_read maps like read for unknown tool names", () => {
    expect(toolIconPrefix("FutureTool", "other_read")).toBe("$_")
  })
  it("other_write maps like mutation for unknown tool names", () => {
    expect(toolIconPrefix("FutureTool", "other_write")).toBe("$!")
  })
})
