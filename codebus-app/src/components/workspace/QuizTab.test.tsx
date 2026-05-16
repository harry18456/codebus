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

const invokeMock = vi.mocked(invoke)

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

  // --- task 5.2: plan-confirm-generate flow ---

  it("Start invokes spawn_quiz_plan, not spawn_quiz_generate", async () => {
    render(<QuizTab vaultPath="/v" />)
    fireEvent.change(screen.getByTestId("quiz-topic-input"), {
      target: { value: "how does auth work" },
    })
    fireEvent.click(screen.getByTestId("quiz-start"))
    await waitFor(() =>
      expect(invokedCommands()).toContain("spawn_quiz_plan"),
    )
    expect(invokedCommands()).not.toContain("spawn_quiz_generate")
  })

  it("scope plan shows confirm controls and does NOT auto-generate", async () => {
    render(<QuizTab vaultPath="/v" />)
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
  it("renders grouped history with retry rows + view-log, opens an attempt", async () => {
    const attempts = [
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
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts") return Promise.resolve(attempts)
      if (cmd === "read_quiz_attempt")
        return Promise.resolve("---\nquiz_id: x\n---\n\n## Q1. opened")
      return Promise.resolve("quiz-run-1")
    })

    render(<QuizTab vaultPath="/v" />)

    await waitFor(() =>
      expect(screen.getByTestId("quiz-history-group")).toBeInTheDocument(),
    )
    // retry → two attempt rows under the one slug group
    expect(screen.getAllByTestId("quiz-attempt-row")).toHaveLength(2)
    // view-log affordance present for the attempt that has events_log
    expect(screen.getAllByTestId("quiz-view-log")).toHaveLength(1)

    // opening a row loads + shows the attempt markdown
    fireEvent.click(screen.getAllByTestId("quiz-attempt-open")[0])
    await waitFor(() =>
      expect(screen.getByTestId("quiz-attempt-view")).toBeInTheDocument(),
    )
    expect(screen.getByTestId("quiz-attempt-view")).toHaveTextContent(
      "## Q1. opened",
    )
  })
})
