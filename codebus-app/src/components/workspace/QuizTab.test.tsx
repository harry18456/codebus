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
      if (cmd === "read_quiz_progress")
        return Promise.resolve({
          schema_version: 1,
          answers: [],
          status: "not_started",
          started_at: null,
          completed_at: null,
        })
      if (cmd === "write_quiz_progress") return Promise.resolve(null)
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

  // quiz-attempt-progress task 8.1 (design D7) — spec: app-workspace
  // § Quiz Tab Plan-Confirm-Generate Flow: confirm view states it will
  // generate from the listed pages; the revise control is `重新規劃`
  // (i18n) and returns to topic input; labels come from i18n.
  it("confirm view: i18n description + 重新規劃 relabel returns to topic input without spawning", async () => {
    const orig = navigator.language
    Object.defineProperty(navigator, "language", {
      value: "zh-TW",
      configurable: true,
    })
    try {
      render(<QuizTab vaultPath="/v" />)
      await openNewQuiz()
      fireEvent.change(screen.getByTestId("quiz-topic-input"), {
        target: { value: "auth" },
      })
      fireEvent.click(screen.getByTestId("quiz-start"))
      await waitFor(() =>
        expect(listeners.has("quiz-plan-terminal")).toBe(true),
      )
      listeners.get("quiz-plan-terminal")!({
        payload: {
          run_id: "quiz-plan-1",
          result: { kind: "scope", pages: ["wiki/modules/auth.md"] },
        },
      })
      await waitFor(() =>
        expect(screen.getByTestId("quiz-confirm")).toBeInTheDocument(),
      )
      // (a) description states it will generate from the listed pages.
      expect(screen.getByTestId("quiz-confirm-desc")).toHaveTextContent(
        "將依下列 wiki 頁面出題",
      )
      // (b) relabeled revise control + (d) confirm label, from i18n.
      expect(screen.getByTestId("quiz-revise")).toHaveTextContent("重新規劃")
      expect(screen.getByTestId("quiz-generate")).toHaveTextContent("確認")
      // (c) revise returns to the topic-input view; the click itself
      // spawns nothing (reaching confirm legitimately required a plan
      // spawn, so assert no NEW spawn after clearing the call record).
      invokeMock.mockClear()
      fireEvent.click(screen.getByTestId("quiz-revise"))
      await waitFor(() =>
        expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument(),
      )
      expect(invokedCommands()).not.toContain("spawn_quiz_plan")
      expect(invokedCommands()).not.toContain("spawn_quiz_generate")
    } finally {
      Object.defineProperty(navigator, "language", {
        value: orig,
        configurable: true,
      })
    }
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

  // --- fix-quiz-ux-wiring task 3.1: generate spawn question count comes
  // from the shared quiz length config (getDefaultLength), NOT a
  // hardcoded component constant. Spec: quiz § Shared Quiz Config
  // Namespace — "App generate uses the configured length". Design D4.
  it("generate spawn uses getDefaultLength() instead of a hardcoded 5", async () => {
    useSettingsStore.setState({ config: { quiz: { default_length: 8 } } })
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
    const genCall = invokeMock.mock.calls.find(
      (c) => c[0] === "spawn_quiz_generate",
    )
    expect(genCall).toBeDefined()
    expect((genCall![1] as { questionCount: number }).questionCount).toBe(8)
  })

  // --- fix-quiz-ux-wiring task 4.1: back-to-history control inside the
  // answering/summary view (design D1). Spec: app-workspace § Quiz Tab
  // Plan-Confirm-Generate Flow — "Back-to-history from the answering
  // view" + "Back-to-history from the summary". Non-destructive: no
  // spawn_quiz_plan / spawn_quiz_generate.

  const IN_PROGRESS_ATTEMPT = {
    slug: "auth-abcd1234",
    quiz_id: "2026-05-16T12-00-00Z",
    trigger: "ai_planned",
    topic: "auth",
    target_page: null,
    events_log: null,
    path: "/v/.codebus/quiz/auth-abcd1234/2026-05-16T12-00-00Z.md",
  }

  it("answering view shows back-to-history; clicking it returns to history without spawning", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts")
        return Promise.resolve([IN_PROGRESS_ATTEMPT])
      if (cmd === "read_quiz_attempt") return Promise.resolve(FIVE_Q_MD)
      if (cmd === "read_quiz_progress")
        return Promise.resolve({
          schema_version: 1,
          answers: [],
          status: "in_progress",
          started_at: "2026-05-16T12:00:00Z",
          completed_at: null,
        })
      if (cmd === "write_quiz_progress") return Promise.resolve(null)
      return Promise.resolve("quiz-run-1")
    })
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-attempt-open")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("quiz-attempt-open"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-stem")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-back-to-history")).toBeInTheDocument()
    fireEvent.click(screen.getByTestId("quiz-back-to-history"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  it("summary shows back-to-history; clicking it returns to history without spawning", async () => {
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 80 } } },
    })
    render(<QuizTab vaultPath="/v" />)
    await runFiveQuizFourCorrect()
    expect(screen.getByTestId("quiz-summary")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-back-to-history")).toBeInTheDocument()
    // Snapshot spawn calls before the back click — the back action
    // itself must be non-destructive (no further spawn).
    invokeMock.mockClear()
    fireEvent.click(screen.getByTestId("quiz-back-to-history"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  // --- fix-quiz-ux-wiring task 5.1: quizHomeSignal prop (design D2).
  // Spec: app-workspace § Quiz Tab Plan-Confirm-Generate Flow —
  // "Re-selecting the Quiz tab returns to quiz history". An incrementing
  // counter from Workspace forces the history phase; the initial 0
  // value must NOT trigger it.

  it("incrementing quizHomeSignal forces the history phase", async () => {
    const { rerender } = render(
      <QuizTab vaultPath="/v" quizHomeSignal={0} />,
    )
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    // Leave history: enter the topic-input view.
    fireEvent.click(screen.getByTestId("new-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-history")).not.toBeInTheDocument()
    // Workspace bumps the signal (re-select active Quiz tab) → history.
    rerender(<QuizTab vaultPath="/v" quizHomeSignal={1} />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
  })

  it("initial quizHomeSignal of 0 does not force the history phase", async () => {
    render(<QuizTab vaultPath="/v" quizHomeSignal={0} />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("new-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument(),
    )
    // The mount-time signal value of 0 must not yank the user back to
    // history — they stay in the topic-input view they navigated to.
    expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument()
    expect(screen.queryByTestId("quiz-history")).not.toBeInTheDocument()
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
        return Promise.resolve(FIVE_Q_MD)
      if (cmd === "read_quiz_progress")
        return Promise.resolve({
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "A", correct: true },
            { q: 3, selected: "A", correct: true },
            { q: 4, selected: "A", correct: true },
            { q: 5, selected: "A", correct: true },
          ],
          status: "completed",
          started_at: "2026-05-16T10:00:00Z",
          completed_at: "2026-05-16T10:05:00Z",
        })
      if (cmd === "write_quiz_progress") return Promise.resolve(null)
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

  // --- quiz-attempt-progress task 4.1 (design D4/D5) ---
  // Spec: app-workspace § Quiz History List — completed attempts open a
  // read-only Review (per-question user choice vs correct + explanation),
  // NOT the raw markdown; rows carry a derived status badge.

  it("history rows show derived badges and no inline view-log panel", async () => {
    const ROWS = [
      {
        slug: "auth-x",
        quiz_id: "c",
        trigger: "ai_planned",
        topic: "auth",
        target_page: null,
        events_log: null,
        path: "/v/.codebus/quiz/auth-x/c.md",
      },
      {
        slug: "auth-x",
        quiz_id: "b",
        trigger: "ai_planned",
        topic: "auth",
        target_page: null,
        events_log: null,
        path: "/v/.codebus/quiz/auth-x/b.md",
      },
      {
        slug: "auth-x",
        quiz_id: "a",
        trigger: "ai_planned",
        topic: "auth",
        target_page: null,
        events_log: null,
        path: "/v/.codebus/quiz/auth-x/a.md",
      },
    ]
    const PROGRESS_BY_PATH: Record<string, unknown> = {
      // c → no sidecar / not-started → 0/5
      "/v/.codebus/quiz/auth-x/c.progress.json": {
        schema_version: 1,
        answers: [],
        status: "not_started",
        started_at: null,
        completed_at: null,
      },
      // b → 2 of 5 answered, in_progress → 2/5
      "/v/.codebus/quiz/auth-x/b.progress.json": {
        schema_version: 1,
        answers: [
          { q: 1, selected: "A", correct: true },
          { q: 2, selected: "B", correct: false },
        ],
        status: "in_progress",
        started_at: "t",
        completed_at: null,
      },
      // a → 5 of 5, 4 correct, completed → 5/5 · 80% · pass (threshold 80)
      "/v/.codebus/quiz/auth-x/a.progress.json": {
        schema_version: 1,
        answers: [
          { q: 1, selected: "A", correct: true },
          { q: 2, selected: "A", correct: true },
          { q: 3, selected: "A", correct: true },
          { q: 4, selected: "A", correct: true },
          { q: 5, selected: "B", correct: false },
        ],
        status: "completed",
        started_at: "t",
        completed_at: "t2",
      },
    }
    useSettingsStore.setState({
      config: { app: { quiz: { pass_threshold: 80 } } },
    })
    invokeMock.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "list_quiz_attempts") return Promise.resolve(ROWS)
      if (cmd === "read_quiz_attempt") return Promise.resolve(FIVE_Q_MD)
      if (cmd === "read_quiz_progress") {
        const p = (args as { path: string }).path
        return Promise.resolve(PROGRESS_BY_PATH[p])
      }
      return Promise.resolve("quiz-run-1")
    })
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(3),
    )
    const badges = screen
      .getAllByTestId("quiz-attempt-badge")
      .map((b) => b.textContent)
    expect(badges).toEqual(["0/5", "2/5", "5/5 · 80% · pass"])
    expect(screen.queryByTestId("quiz-view-log-panel")).not.toBeInTheDocument()
  })

  it("completed attempt opens QuizReview (not raw md); modal view-log works", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2),
    )
    // row[0] = newest = the one WITH events_log; status completed.
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[0])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-review")).toBeInTheDocument(),
    )
    // Review, NOT the raw-markdown attempt view.
    expect(screen.queryByTestId("quiz-attempt-view")).not.toBeInTheDocument()
    expect(screen.queryByTestId("quiz-attempt-md")).not.toBeInTheDocument()
    expect(screen.getAllByTestId("quiz-review-question")).toHaveLength(5)

    fireEvent.click(screen.getByTestId("quiz-view-log"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-view-log-modal")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("thought-item")).toBeInTheDocument()
    fireEvent.click(screen.getByTestId("quiz-view-log-close"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("quiz-view-log-modal"),
      ).not.toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-review")).toBeInTheDocument()
  })

  it("[重做此份] resets the sidecar and restarts answering at Q1 without spawning", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2),
    )
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[0])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-redo-this")).toBeInTheDocument(),
    )
    invokeMock.mockClear()
    fireEvent.click(screen.getByTestId("quiz-redo-this"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-answering")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 1 of 5",
    )
    // sidecar reset persisted; no agent re-spawn.
    expect(invokedCommands()).toContain("write_quiz_progress")
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  // --- quiz-attempt-progress task 7.1 (design D3 revised) ---
  // Spec: app-workspace § Quiz Answering and Summary — "Opening an
  // in-progress attempt restores the last answered question"; Next
  // continues at the first unanswered; submitting there persists.
  it("opening an in-progress attempt restores the last answered question; Next then submit persists via write_quiz_progress without spawning", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts")
        return Promise.resolve([
          {
            slug: "auth-x",
            quiz_id: "2026-05-18T10-00-00Z",
            trigger: "ai_planned",
            topic: "auth",
            target_page: null,
            events_log: null,
            path: "/v/.codebus/quiz/auth-x/2026-05-18T10-00-00Z.md",
          },
        ])
      if (cmd === "read_quiz_attempt")
        return Promise.resolve(FIVE_Q_MD)
      if (cmd === "read_quiz_progress")
        return Promise.resolve({
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "A", correct: true },
          ],
          status: "in_progress",
          started_at: "2026-05-18T10:00:00Z",
          completed_at: null,
        })
      if (cmd === "write_quiz_progress") return Promise.resolve(null)
      return Promise.resolve("quiz-run-1")
    })
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getByTestId("quiz-attempt-open")).toBeInTheDocument(),
    )
    fireEvent.click(screen.getByTestId("quiz-attempt-open"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-answering")).toBeInTheDocument(),
    )
    // Restored at the last answered question (Q2), revealed.
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 2 of 5",
    )
    expect(screen.getByTestId("quiz-verdict")).toBeInTheDocument()
    // Next advances to the first unanswered (Q3); answer there persists.
    fireEvent.click(screen.getByTestId("quiz-next"))
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 3 of 5",
    )
    fireEvent.click(screen.getByTestId("quiz-choice-A"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    await waitFor(() =>
      expect(invokedCommands()).toContain("write_quiz_progress"),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_plan")
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  it("completed attempt without events_log shows Review with no view-log affordance", async () => {
    mockHistory()
    render(<QuizTab vaultPath="/v" />)
    await waitFor(() =>
      expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2),
    )
    // row[1] = older = events_log null
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[1])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-review")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-attempt-view")).not.toBeInTheDocument()
    expect(screen.queryByTestId("quiz-view-log")).not.toBeInTheDocument()
  })
})
