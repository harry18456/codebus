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
