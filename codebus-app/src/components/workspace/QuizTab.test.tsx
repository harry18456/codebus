import { render, screen, fireEvent, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

// Mock Tauri core/event BEFORE importing the component (mirrors
// chat.test.ts). `listen` captures the callback per channel so a test
// can drive the terminal payload deterministically.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("quiz-run-1")),
}))
const listeners = new Map<string, (e: { payload: unknown }) => void>()
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((channel: string, cb: (e: { payload: unknown }) => void) => {
    listeners.set(channel, cb)
    return Promise.resolve(() => listeners.delete(channel))
  }),
}))

import { invoke } from "@tauri-apps/api/core"
import { QuizTab } from "./QuizTab"
import { useSettingsStore } from "@/store/settings"

const invokeMock = vi.mocked(invoke)

/** A 5-question quiz body, every answer = A. */
const FIVE_Q_MD = [1, 2, 3, 4, 5]
  .map(
    (n) =>
      `## Q${n}. q${n}?\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e${n}`,
  )
  .join("\n\n")

/**
 * The Quiz tab now mounts on the history view (design D5). Click
 * + New quiz to reach the topic-input view before driving a quiz.
 */
async function openNewQuiz() {
  fireEvent.click(screen.getByTestId("new-quiz"))
  await screen.findByTestId("quiz-topic-input")
}

/**
 * Drive QuizTab from the input view to the summary screen with a
 * 5-question quiz, answering the first 4 correctly (A) and the last
 * one wrong (B) → 4/5.
 */
async function runFiveQuizFourCorrect() {
  await openNewQuiz()
  fireEvent.change(screen.getByTestId("quiz-topic-input"), {
    target: { value: "auth" },
  })
  fireEvent.click(screen.getByTestId("quiz-start"))
  await waitFor(() => expect(listeners.has("quiz-plan-terminal")).toBe(true))
  listeners.get("quiz-plan-terminal")!({
    payload: { run_id: "p1", result: { kind: "scope", pages: ["wiki/a.md"] } },
  })
  await waitFor(() =>
    expect(screen.getByTestId("quiz-generate")).toBeInTheDocument(),
  )
  fireEvent.click(screen.getByTestId("quiz-generate"))
  await waitFor(() =>
    expect(listeners.has("quiz-generate-terminal")).toBe(true),
  )
  listeners.get("quiz-generate-terminal")!({
    payload: {
      run_id: "g1",
      result: {
        kind: "succeeded",
        quiz_md: FIVE_Q_MD,
        planned_pages: ["wiki/a.md"],
        events_log: "/v/.codebus/log/events-x.jsonl",
      },
    },
  })
  await waitFor(() =>
    expect(screen.getByTestId("quiz-stem")).toBeInTheDocument(),
  )
  for (let n = 1; n <= 5; n++) {
    const pick = n <= 4 ? "quiz-choice-A" : "quiz-choice-B"
    fireEvent.click(screen.getByTestId(pick))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    fireEvent.click(screen.getByTestId("quiz-next"))
  }
  await waitFor(() =>
    expect(screen.getByTestId("quiz-summary")).toBeInTheDocument(),
  )
}

function invokedCommands(): string[] {
  return invokeMock.mock.calls.map((c) => c[0] as string)
}

describe("QuizTab", () => {
  beforeEach(() => {
    invokeMock.mockClear()
    // The idle history effect calls `list_quiz_attempts`; default it to
    // an empty list so tests that don't care about history stay simple.
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts") return Promise.resolve([])
      if (cmd === "read_quiz_attempt") return Promise.resolve("# attempt md")
      return Promise.resolve("quiz-run-1")
    })
    listeners.clear()
  })
  afterEach(() => {
    listeners.clear()
    useSettingsStore.setState({ config: {} })
  })

  // --- fix-app-quiz task 1.3: summary pass/fail driven by the settings
  // store's app.quiz.pass_threshold (NOT a hardcoded component constant).
  // Spec: app-workspace § Quiz Answering and Summary —
  // "Summary applies pass threshold" + "Changing the threshold setting
  // changes the outcome". Design D1 contract: drives the store value.

  it("summary passes a 4/5 quiz when store pass_threshold is 80", async () => {
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 80 } } },
    })
    render(<QuizTab vaultPath="/v" />)
    await runFiveQuizFourCorrect()
    expect(screen.getByTestId("quiz-outcome")).toHaveTextContent("Passed")
  })

  it("summary fails the same 4/5 quiz when store pass_threshold is 90", async () => {
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 90 } } },
    })
    render(<QuizTab vaultPath="/v" />)
    await runFiveQuizFourCorrect()
    expect(screen.getByTestId("quiz-outcome")).toHaveTextContent("Failed")
  })

  // --- task 5.1: tab shell ---

  it("renders quiz history region and new-quiz control, no v1 placeholder", () => {
    render(<QuizTab vaultPath="/v" />)
    expect(screen.getByTestId("quiz-tab")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-history")).toBeInTheDocument()
    expect(screen.getByTestId("new-quiz")).toHaveTextContent("+ New quiz")
    expect(
      screen.queryByText("Coming soon — quiz flow ships in v3-app-quiz"),
    ).not.toBeInTheDocument()
  })

  // --- fix-app-quiz task 12.1 (defect #7): + New quiz only in the
  // browse/compose context (history + idle), hidden once inside a quiz.
  // Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow.
  it("+ New quiz is hidden once inside a quiz flow", async () => {
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("new-quiz")).toBeInTheDocument() // history
    fireEvent.click(screen.getByTestId("new-quiz"))
    await screen.findByTestId("quiz-topic-input")
    expect(screen.getByTestId("new-quiz")).toBeInTheDocument() // idle
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "auth" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-planning")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("new-quiz")).not.toBeInTheDocument()
  })

  // --- fix-app-quiz task 6.1: header must not collide with the fixed
  // WindowControls (min/max/close). Mirror GoalsTab: reserve pr-[160px]
  // and be a drag region. Spec: app-workspace § Workspace Layout.
  it("Quiz header reserves WindowControls inset and is a drag region", () => {
    render(<QuizTab vaultPath="/v" />)
    const header = screen.getByTestId("new-quiz").parentElement!
    expect(header).toHaveAttribute("data-tauri-drag-region")
    expect(header.className).toContain("pr-[160px]")
    // Style parity with GoalsTab's + New goal (variant="primary").
    expect(screen.getByTestId("new-quiz").className).toContain("bg-accent")
  })

  // --- fix-app-quiz task 7.1: defect #2 — + New quiz must open a
  // distinct topic-input view; default tab view is history only.
  // Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow. Design D5.

  it("default view shows history only, not the topic input", async () => {
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-topic-input")).not.toBeInTheDocument()
  })

  it("+ New quiz opens the topic input and hides history", async () => {
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("new-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-history")).not.toBeInTheDocument()
  })

  it("← History returns from the input view to history", async () => {
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("new-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-back-to-history")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("quiz-back-to-history"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-topic-input")).not.toBeInTheDocument()
  })

  it("pendingPage still generates directly without showing history", async () => {
    render(
      <QuizTab
        vaultPath="/v"
        pendingPage="wiki/modules/auth.md"
        onPendingConsumed={vi.fn()}
      />,
    )
    await waitFor(() =>
      expect(invokedCommands()).toContain("spawn_quiz_generate"),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(screen.queryByTestId("quiz-history")).not.toBeInTheDocument()
  })

  // --- task 5.2: plan-confirm-generate flow ---

  it("Start invokes spawn_quiz_plan, not spawn_quiz_generate", async () => {
    render(<QuizTab vaultPath="/v" />)
    await openNewQuiz()
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "how does auth work" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() =>
      expect(invokedCommands()).toContain("spawn_quiz_plan"),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  // fix-app-quiz task 10.1: defect #5 — plan/generate agent activity
  // must render live via the existing stream rendering, not a static
  // label. Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow.
  it("renders live agent stream during planning (not just a static label)", async () => {
    render(<QuizTab vaultPath="/v" />)
    await openNewQuiz()
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "auth" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() => expect(listeners.has("quiz-stream")).toBe(true))
    listeners.get("quiz-stream")!({
      payload: {
        run_id: "quiz-plan-1",
        event: { kind: "stream", data: { kind: "thought", text: "scanning wiki" } },
      },
    })
    await waitFor(() =>
      expect(screen.getByTestId("thought-item")).toBeInTheDocument(),
    )
  })

  it("scope plan shows confirm controls and does NOT auto-generate", async () => {
    render(<QuizTab vaultPath="/v" />)
    await openNewQuiz()
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "auth" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() => expect(listeners.has("quiz-plan-terminal")).toBe(true))

    // Plan resolves with a scope.
    listeners.get("quiz-plan-terminal")!({
      payload: {
        run_id: "quiz-plan-1",
        result: { kind: "scope", pages: ["wiki/modules/auth.md"] },
      },
    })

    await waitFor(() =>
      expect(screen.getByTestId("quiz-confirm")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-scope-page")).toHaveTextContent(
      "wiki/modules/auth.md",
    )
    expect(screen.getByTestId("quiz-generate")).toBeInTheDocument()
    // Confirm gate: generation MUST NOT have started yet.
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")

    // Explicit confirm starts generation.
    fireEvent.click(screen.getByTestId("quiz-generate"))
    await waitFor(() =>
      expect(invokedCommands()).toContain("spawn_quiz_generate"),
    )
  })

  it("no-match plan shows reason and never generates", async () => {
    render(<QuizTab vaultPath="/v" />)
    await openNewQuiz()
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "quantum mechanics" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() => expect(listeners.has("quiz-plan-terminal")).toBe(true))

    listeners.get("quiz-plan-terminal")!({
      payload: {
        run_id: "quiz-plan-1",
        result: { kind: "no_match", reason: "vault only covers web auth" },
      },
    })

    await waitFor(() =>
      expect(screen.getByTestId("quiz-no-match")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-no-match")).toHaveTextContent(
      "vault only covers web auth",
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  it("generate success surfaces the quiz body", async () => {
    render(<QuizTab vaultPath="/v" />)
    await openNewQuiz()
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "auth" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() => expect(listeners.has("quiz-plan-terminal")).toBe(true))
    listeners.get("quiz-plan-terminal")!({
      payload: {
        run_id: "p1",
        result: { kind: "scope", pages: ["wiki/a.md"] },
      },
    })
    await waitFor(() =>
      expect(screen.getByTestId("quiz-generate")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("quiz-generate"))
    await waitFor(() =>
      expect(listeners.has("quiz-generate-terminal")).toBe(true),
    )
    listeners.get("quiz-generate-terminal")!({
      payload: {
        run_id: "g1",
        result: {
          kind: "succeeded",
          quiz_md:
            "## Q1. mock?\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e",
          planned_pages: ["wiki/a.md"],
          events_log: "/v/.codebus/log/events-x.jsonl",
        },
      },
    })
    await waitFor(() =>
      expect(screen.getByTestId("quiz-ready")).toBeInTheDocument(),
    )
    // ready phase renders the answering view (task 5.4), parsed from
    // the quiz body.
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("mock?")
  })

  // --- task 5.3: Page flow skips planning ---
  // Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow —
  // "Quiz-me-on-this skips planning": a pendingPage triggers
  // generation directly (no plan spawn).
  it("pendingPage triggers generate directly, never plan", async () => {
    const onConsumed = vi.fn()
    render(
      <QuizTab
        vaultPath="/v"
        pendingPage="wiki/modules/auth-middleware.md"
        onPendingConsumed={onConsumed}
      />,
    )
    await waitFor(() =>
      expect(invokedCommands()).toContain("spawn_quiz_generate"),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(onConsumed).toHaveBeenCalledTimes(1)
  })

  // --- task 5.5: history list ---
  // Spec: app-workspace § Quiz History List — attempts grouped by slug,
  // retry shows two rows under one slug, view-log affordance present,
  // row opens the attempt markdown.
  const HISTORY_ATTEMPTS = [
    {
      slug: "auth-abcd1234",
      quiz_id: "2026-05-16T11-00-00Z",
      trigger: "ai_planned",
      topic: "auth",
      target_page: null,
      events_log: "/v/.codebus/log/events-b.jsonl",
      path: "/v/.codebus/quiz/auth-abcd1234/2026-05-16T11-00-00Z.md",
    },
    {
      slug: "auth-abcd1234",
      quiz_id: "2026-05-16T10-00-00Z",
      trigger: "ai_planned",
      topic: "auth",
      target_page: null,
      events_log: null,
      path: "/v/.codebus/quiz/auth-abcd1234/2026-05-16T10-00-00Z.md",
    },
  ]

  function mockHistory() {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts")
        return Promise.resolve(HISTORY_ATTEMPTS)
      if (cmd === "read_quiz_attempt")
        return Promise.resolve("---\nquiz_id: x\n---\n\n## Q1. opened")
      if (cmd === "read_quiz_events")
        return Promise.resolve([
          {
            ts: "2026-05-17T00:00:00Z",
            event: {
              kind: "stream",
              data: { kind: "thought", text: "planning the quiz" },
            },
          },
        ])
      return Promise.resolve("quiz-run-1")
    })
  }

  // fix-app-quiz task 11.1 (UX #6 / design D9): view-log moved OFF the
  // history row and INTO the opened attempt detail view, shown as a
  // centered modal. Spec: app-workspace § Quiz History List (re-authored).

  it("history rows have no view-log button and no inline panel", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history-group")).toBeInTheDocument(),
    )
    expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2)
    expect(screen.queryByTestId("quiz-view-log")).not.toBeInTheDocument()
    expect(
      screen.queryByTestId("quiz-view-log-panel"),
    ).not.toBeInTheDocument()
    // opening a row still shows the attempt markdown
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[0])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-attempt-view")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-attempt-view")).toHaveTextContent(
      "## Q1. opened",
    )
  })

  it("opened attempt with events_log shows a modal view-log timeline", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2),
    )
    // row[0] = newest = the one WITH events_log
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[0])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-view-log")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("quiz-view-log"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-view-log-modal")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("thought-item")).toBeInTheDocument()
    expect(screen.queryByTestId("quiz-view-log-path")).not.toBeInTheDocument()
    // dismiss → back to the attempt detail view
    fireEvent.click(screen.getByTestId("quiz-view-log-close"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("quiz-view-log-modal"),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-attempt-view")).toBeInTheDocument()
  })

  it("opened attempt without events_log shows no view-log affordance", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2),
    )
    // row[1] = older = events_log null
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[1])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-attempt-view")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-view-log")).not.toBeInTheDocument()
  })
})
