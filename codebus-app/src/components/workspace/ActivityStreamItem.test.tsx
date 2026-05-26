import { render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it } from "vitest"

import { ActivityStreamItem, ThoughtItem } from "./ActivityStreamItem"
import type { VerbEvent } from "@/lib/ipc"

/**
 * Temporarily set `navigator.language` for a single test. Mirrors the
 * pattern used by QuizTab.test.tsx so `useT` resolves to the same
 * locale the component would see in production.
 */
function withLocale(lang: string, run: () => void) {
  const orig = navigator.language
  Object.defineProperty(navigator, "language", {
    value: lang,
    configurable: true,
  })
  try {
    run()
  } finally {
    Object.defineProperty(navigator, "language", {
      value: orig,
      configurable: true,
    })
  }
}

/**
 * Build a `tool_use` VerbEvent for a Shell-like command tool. Defaults
 * to name "Bash" (the Claude Code tool name); callers may override for
 * provider-specific names (e.g. Codex "Shell").
 */
function shellToolUse(command: string, name: string = "Bash"): VerbEvent {
  return {
    kind: "stream",
    data: { kind: "tool_use", name, input: { command } },
  }
}

describe("ActivityStreamItem · X1 Shell Command Wrapper Extraction", () => {
  // critical-bugs-ql1-x1-qgen1 task 2.1
  // Spec: app-workspace § Activity Stream Shell Command Wrapper
  // Extraction — the displayed command MUST be the inner user-authored
  // command, not the OS-specific wrapper the agent runtime wraps it in.

  it("strips a PowerShell wrapper with a quoted absolute path", () => {
    render(
      <ActivityStreamItem
        event={shellToolUse(
          '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "Get-Content package.json | Select-Object -First 50"',
        )}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("Get-Content package.json")
    expect(row.textContent).not.toContain("powershell.exe")
    expect(row.textContent).not.toContain("-Command")
  })

  it("strips a PowerShell wrapper with a leading -NoProfile switch (real Codex form)", () => {
    // Discovered live during apply: 79/195 Shell rows in a real Codex
    // run still exposed the wrapper because the agent used
    // `-NoProfile -Command` rather than the bare `-Command`. Spec
    // scenario "PowerShell wrapper with leading switch flags is stripped".
    render(
      <ActivityStreamItem
        event={shellToolUse(
          '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -NoProfile -Command "Get-ChildItem -Recurse -File wiki"',
        )}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("Get-ChildItem -Recurse -File wiki")
    expect(row.textContent).not.toContain("powershell.exe")
    expect(row.textContent).not.toContain("-NoProfile")
    expect(row.textContent).not.toContain("-Command")
  })

  it("strips a PowerShell wrapper with multiple leading switch flags", () => {
    render(
      <ActivityStreamItem
        event={shellToolUse(
          'powershell.exe -NoLogo -NonInteractive -NoProfile -Command "Get-Date"',
        )}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("Get-Date")
    expect(row.textContent).not.toContain("powershell.exe")
    expect(row.textContent).not.toContain("-NoLogo")
    expect(row.textContent).not.toContain("-NonInteractive")
    expect(row.textContent).not.toContain("-NoProfile")
    expect(row.textContent).not.toContain("-Command")
  })

  it("strips a bare powershell.exe -Command wrapper (no leading path)", () => {
    render(
      <ActivityStreamItem
        event={shellToolUse('powershell.exe -Command "ls D:\\"')}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("ls D:\\")
    expect(row.textContent).not.toContain("powershell.exe")
  })

  it("strips a PowerShell wrapper around a multi-line here-string inner command", () => {
    // Discovered live during apply: 5 Shell rows in the real Codex run
    // used a PowerShell here-string (`@'\n...'@`) as the inner command,
    // so the inner payload contained `\n`. The regex MUST be dotAll
    // (`s` flag) for `.+` to span newlines and still anchor at `$`.
    render(
      <ActivityStreamItem
        event={shellToolUse(
          '"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe" -Command "@\'\ninit code line 1\ninit code line 2\n\'@"',
        )}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("init code line 1")
    expect(row.textContent).not.toContain("powershell.exe")
    expect(row.textContent).not.toContain("-Command")
  })

  it("strips a /bin/sh -c wrapper with a double-quoted inner command", () => {
    render(
      <ActivityStreamItem
        event={shellToolUse('/bin/sh -c "git log --oneline -n 20"')}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("git log --oneline -n 20")
    expect(row.textContent).not.toContain("/bin/sh")
    expect(row.textContent).not.toContain("-c ")
  })

  it("strips a bash -c wrapper with a single-quoted inner command", () => {
    render(
      <ActivityStreamItem
        event={shellToolUse("bash -c 'grep -r \"AppShell\" src/'")}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("grep -r")
    expect(row.textContent).not.toContain("bash -c")
  })

  it("strips a bare sh -c wrapper with a double-quoted inner command", () => {
    render(<ActivityStreamItem event={shellToolUse('sh -c "ls -la"')} />)
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("ls -la")
    // The leading `sh -c ` must be gone; a coincidental "sh" inside the
    // inner command (e.g. `sh-friendly`) would still pass — we only
    // assert the wrapper prefix is not present at the start of the body.
    expect(row.textContent).not.toMatch(/sh -c/)
  })

  it("passes an unwrapped command through unchanged", () => {
    render(<ActivityStreamItem event={shellToolUse("git status --short")} />)
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).toContain("git status --short")
  })

  // (QGEN1 tests follow this describe block — see below.)

  it("truncates a 200-char PowerShell-wrapped inner command after extraction, not before", () => {
    // 200-char inner command, all 'a'. After extraction the body MUST
    // be 80 chars + ellipsis and MUST NOT contain any wrapper text.
    const inner = "a".repeat(200)
    render(
      <ActivityStreamItem
        event={shellToolUse(`powershell.exe -Command "${inner}"`)}
      />,
    )
    const row = screen.getByTestId("stream-tool-use")
    expect(row.textContent).not.toContain("powershell.exe")
    expect(row.textContent).not.toContain("-Command")
    // The summary body lives in the inner span (` · <summary>`); it MUST
    // start with at least 70 leading 'a's (we are tolerant: an extractor
    // that leaves a stray leading quote would still satisfy the
    // wrapper-stripped invariant, but the truncation cap MUST be hit).
    expect(row.textContent).toMatch(/a{70,}…/)
    // And the 'a' run MUST NOT spill past the 80-char cap.
    expect(row.textContent).not.toMatch(/a{81,}/)
  })
})

describe("ThoughtItem · QGEN1 Internal Sentinel Marker Filter", () => {
  // critical-bugs-ql1-x1-qgen1 task 3.1
  // Spec: app-workspace § Activity Stream Internal Sentinel Marker
  // Filter — `[CODEBUS_*]` markers are an agent ↔ codebus-core wire
  // protocol; the renderer MUST translate known markers and suppress
  // unknown ones rather than render them raw.

  const KNOWN_MARKER_TEXT =
    "[CODEBUS_QUIZ_NO_VALIDATE] codex sandbox cannot run quiz structure validation"

  beforeEach(() => {
    // Ensure each test starts from a clean DOM (RTL auto-cleanup also
    // runs, but be explicit).
  })
  afterEach(() => {
    // No shared state to reset.
  })

  it("zh-tw: known marker is replaced by the translated text", () => {
    withLocale("zh-TW", () => {
      render(<ThoughtItem text={KNOWN_MARKER_TEXT} />)
      const item = screen.getByTestId("thought-item")
      expect(item.textContent).toContain(
        "codex 沙箱無法跑 quiz 結構驗證，跳過此步",
      )
      expect(item.textContent).not.toContain("[CODEBUS_QUIZ_NO_VALIDATE]")
    })
  })

  it("unknown marker is suppressed entirely (no DOM)", () => {
    withLocale("zh-TW", () => {
      render(
        <ThoughtItem text="[CODEBUS_FUTURE_MARKER] some payload codebus-app has never seen" />,
      )
      expect(screen.queryByTestId("thought-item")).toBeNull()
    })
  })

  it("plain thought text without a leading marker renders verbatim", () => {
    withLocale("zh-TW", () => {
      render(
        <ThoughtItem text="I will start by reading README.md to understand the project structure." />,
      )
      const item = screen.getByTestId("thought-item")
      expect(item.textContent).toContain(
        "I will start by reading README.md to understand the project structure.",
      )
    })
  })

  it("mid-sentence marker does NOT trigger the filter (verbatim)", () => {
    withLocale("zh-TW", () => {
      render(
        <ThoughtItem text="The agent emitted [CODEBUS_QUIZ_SCOPE] wiki/a.md as its first line." />,
      )
      const item = screen.getByTestId("thought-item")
      expect(item.textContent).toContain("[CODEBUS_QUIZ_SCOPE]")
    })
  })
})

describe("ActivityStreamItem · bannerLabel i18n (10 cases × en + zh)", () => {
  // Each case asserts both en + zh wording per the i18n Bundle Coverage
  // Policy emoji-prefixed scenario: the emoji + text live in one bundle
  // value so the rendered string contains both the emoji and the locale's
  // text in one pass.
  const bannerEvents: Array<{
    label: string
    event: VerbEvent
    en: string
    zh: string
  }> = [
    {
      label: "start",
      event: { kind: "banner", data: { kind: "start", repo_path: "C:\\repos\\x" } },
      en: "🚌 Here comes the CodeBus, rolling into C:/repos/x...",
      zh: "🚌 來囉來囉~ CodeBus 駛入 C:/repos/x...",
    },
    {
      label: "goal",
      event: { kind: "banner", data: { kind: "goal", goal_text: "describe X" } },
      en: "🎯 Goal target: describe X",
      zh: "🎯 任務目標：describe X",
    },
    {
      label: "sync_start",
      event: { kind: "banner", data: { kind: "sync_start" } },
      en: "🔄 Syncing source → raw/code...",
      zh: "🔄 同步 source → raw/code...",
    },
    {
      label: "sync_done",
      event: {
        kind: "banner",
        data: { kind: "sync_done", files: 12, mib: 0.5, elapsed_ms: 480 },
      },
      en: "✓ Sync done (12 files, 0.5 MiB, 480 ms)",
      zh: "✓ 同步完成 (12 檔, 0.5 MiB, 480 ms)",
    },
    {
      label: "pii_summary",
      event: {
        kind: "banner",
        data: {
          kind: "pii_summary",
          scanner: "regex",
          scanned: 9,
          hits: 0,
          action: "warn",
        },
      },
      en: "🛡 PII: regex, scanned 9, hits 0, action warn",
      zh: "🛡 PII：regex, scanned 9, hits 0, action warn",
    },
    {
      label: "lint_start",
      event: { kind: "banner", data: { kind: "lint_start" } },
      en: "🔍 Linting...",
      zh: "🔍 lint 中...",
    },
    {
      label: "lint_done",
      event: {
        kind: "banner",
        data: { kind: "lint_done", errors: 0, warns: 2, elapsed_ms: 350 },
      },
      en: "✓ Lint done (0 errors, 2 warnings, 350 ms)",
      zh: "✓ lint 完成 (0 errors, 2 warns, 350 ms)",
    },
    {
      label: "commit_done",
      event: { kind: "banner", data: { kind: "commit_done", sha7: "abc1234" } },
      en: "🚏 Commit abc1234",
      zh: "🚏 commit abc1234",
    },
    {
      label: "done",
      event: { kind: "banner", data: { kind: "done", wiki_path: "/v/.codebus/wiki" } },
      en: "🎉 Complete",
      zh: "🎉 完成",
    },
    {
      label: "hint",
      event: { kind: "banner", data: { kind: "hint", wiki_path: "/v/.codebus/wiki" } },
      en: "💡 Hint",
      zh: "💡 提示",
    },
  ]

  for (const { label, event, en, zh } of bannerEvents) {
    it(`ActivityStreamItem_banner_${label}_en`, () => {
      withLocale("en-US", () => {
        render(<ActivityStreamItem event={event} />)
        const row = screen.getByTestId("stream-banner")
        expect(row.textContent).toBe(en)
      })
    })
    it(`ActivityStreamItem_banner_${label}_zh`, () => {
      withLocale("zh-TW", () => {
        render(<ActivityStreamItem event={event} />)
        const row = screen.getByTestId("stream-banner")
        expect(row.textContent).toBe(zh)
      })
    })
  }
})
