import { render, screen, fireEvent, waitFor, within } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}))
const listeners = new Map<string, (e: { payload: unknown }) => void>()
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((channel: string, cb: (e: { payload: unknown }) => void) => {
    listeners.set(channel, cb)
    return Promise.resolve(() => listeners.delete(channel))
  }),
}))

import { invoke } from "@tauri-apps/api/core"
import { QuizReview } from "./QuizReview"
import type { QuizProgress } from "@/lib/ipc"

const invokeMock = vi.mocked(invoke)

const TWO_Q = `## Q1. What is auth?

- A) a cache
- B) identity verification
- C) a database
- D) a queue

## Answer: B

## Explanation: Auth verifies identity.

## Q2. Where does middleware run?

- A) before route handlers
- B) in the database
- C) in the frontend
- D) never

## Answer: A

## Explanation: Middleware runs before handlers.`

const PROGRESS: QuizProgress = {
  schema_version: 1,
  answers: [
    { q: 1, selected: "B", correct: true },
    { q: 2, selected: "C", correct: false },
  ],
  status: "completed",
  started_at: "2026-05-18T10:00:00Z",
  completed_at: "2026-05-18T10:05:00Z",
}

describe("QuizReview", () => {
  beforeEach(() => {
    invokeMock.mockClear()
    invokeMock.mockImplementation((cmd: string) => {
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
      return Promise.resolve(null)
    })
    listeners.clear()
  })
  afterEach(() => listeners.clear())

  // Spec: app-workspace § Quiz History List —
  // "Completed attempt opens Review, not raw markdown".
  it("renders each question with user choice vs correct + explanation, no raw md block", () => {
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-review")).toBeInTheDocument()
    // raw markdown preformatted block must NOT be used.
    expect(screen.queryByTestId("quiz-attempt-md")).not.toBeInTheDocument()

    const qs = screen.getAllByTestId("quiz-review-question")
    expect(qs).toHaveLength(2)
    expect(qs[0]).toHaveTextContent("What is auth?")
    // Q1: user picked B (correct).
    expect(qs[0]).toHaveTextContent("Your answer: B")
    expect(qs[0]).toHaveTextContent("Correct answer: B")
    expect(qs[0]).toHaveTextContent("Auth verifies identity")
    // Q2: user picked C, correct is A.
    expect(qs[1]).toHaveTextContent("Your answer: C")
    expect(qs[1]).toHaveTextContent("Correct answer: A")
    expect(qs[1]).toHaveTextContent("Middleware runs before handlers")
  })

  // Spec: "Redo this resets without spawning" (component-level: fires
  // the callback; QuizTab wires the sidecar reset + no-spawn).
  it("[重做此份] invokes onRedo", () => {
    const onRedo = vi.fn()
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        onRedo={onRedo}
        onBack={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-redo-this"))
    expect(onRedo).toHaveBeenCalledTimes(1)
  })

  // Spec: "View-generation-log opens a modal timeline from Review".
  it("shows the 看過程 modal timeline when events_log is non-null", async () => {
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog="/v/.codebus/log/events-x.jsonl"
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )
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
  })

  // Spec: "No view-generation-log affordance without an events log".
  it("hides the 看過程 affordance when events_log is null", () => {
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    expect(screen.queryByTestId("quiz-view-log")).not.toBeInTheDocument()
  })

  // quiz-attempt-progress task 6.3 (design D6): each question's
  // explanation renders its [[slug]] citations as navigable wikilinks.
  it("renders explanation [[slug]] citations as clickable wikilinks", () => {
    const MD = `## Q1. stem?

- A) a
- B) b
- C) c
- D) d

## Answer: A

## Explanation: See [[auth-middleware-verification]] for the 401 rule.`
    const progress: QuizProgress = {
      schema_version: 1,
      answers: [{ q: 1, selected: "A", correct: true }],
      status: "completed",
      started_at: "2026-05-18T10:00:00Z",
      completed_at: "2026-05-18T10:05:00Z",
    }
    const onOpenWikiPage = vi.fn()
    render(
      <QuizReview
        quizMd={MD}
        progress={progress}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        pages={{
          "auth-middleware-verification": {
            slug: "auth-middleware-verification",
            path: "wiki/processes/auth-middleware-verification.md",
            title: "Auth Middleware Verification",
          },
        }}
        onOpenWikiPage={onOpenWikiPage}
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    const link = screen.getByTestId("wikilink-auth-middleware-verification")
    expect(link).toHaveTextContent("Auth Middleware Verification")
    expect(link.textContent ?? "").not.toContain("[[")
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("auth-middleware-verification")
  })

  it("renders inline markdown in review stem, choices, and explanation", () => {
    const MD = `## Q1. Why use \`codebus-core\` with **Rust**?

- A) *workspace* modeling
- B) plain text
- C) no parser
- D) raw output

## Answer: A

## Explanation: Use \`read_wiki_page\` with [[desktop-app-workspace]] and **typed** data.`
    const progress: QuizProgress = {
      schema_version: 1,
      answers: [{ q: 1, selected: "A", correct: true }],
      status: "completed",
      started_at: "2026-05-18T10:00:00Z",
      completed_at: "2026-05-18T10:05:00Z",
    }
    render(
      <QuizReview
        quizMd={MD}
        progress={progress}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        pages={{
          "desktop-app-workspace": {
            slug: "desktop-app-workspace",
            path: "wiki/modules/desktop-app-workspace.md",
            title: "Desktop App Workspace",
          },
        }}
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )

    const question = screen.getByTestId("quiz-review-question")
    expect(within(question).getByText("codebus-core").tagName).toBe("CODE")
    expect(within(question).getByText("Rust").tagName).toBe("STRONG")
    expect(within(question).getByText("workspace").tagName).toBe("EM")
    expect(within(question).getByText("read_wiki_page").tagName).toBe("CODE")
    expect(within(question).getByText("typed").tagName).toBe("STRONG")
    const link = screen.getByTestId("wikilink-desktop-app-workspace")
    expect(link).toHaveClass("cite-link")
    expect(question).not.toHaveTextContent("`")
  })

  // Phase 5.4 quiz-fullscreen-wizard-view: when hosted inside the wizard
  // (`embedded={true}`), the wizard `TabContentHeader` provides the back
  // affordance, so QuizReview SHALL NOT render its own
  // `[← Back to history]` button (per spec design D6 邊界).
  it("embedded=true hides the standalone back-to-history button", () => {
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        onRedo={vi.fn()}
        onBack={vi.fn()}
        embedded={true}
      />,
    )
    expect(screen.queryByTestId("quiz-attempt-back")).not.toBeInTheDocument()
    // The redo control is preserved — wizard chrome doesn't supply that.
    expect(screen.getByTestId("quiz-redo-this")).toBeInTheDocument()
  })

  it("embedded prop default (omitted) preserves the existing back button", () => {
    render(
      <QuizReview
        quizMd={TWO_Q}
        progress={PROGRESS}
        passThreshold={80}
        vaultPath="/v"
        eventsLog={null}
        onRedo={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-attempt-back")).toBeInTheDocument()
  })
})
