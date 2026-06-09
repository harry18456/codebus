import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { QuizAnswering } from "./QuizAnswering"

const TWO_Q = `## Q1. What is auth?

- A) a cache
- B) identity verification
- C) a database
- D) a queue

## Answer: B

## Explanation: Auth verifies identity, see [[auth-middleware]].

## Q2. Where does middleware run?

- A) before route handlers
- B) in the database
- C) in the frontend
- D) never

## Answer: A

## Explanation: Middleware runs before handlers.`

describe("QuizAnswering", () => {
  // quiz-attempt-progress task 6.3 (design D6) — spec: app-workspace
  // § Quiz Answering and Summary "Explanation citations render as
  // navigable wikilinks on both outcomes". The standalone
  // `[← Back to wiki page]` button is removed.
  it("correct answer: explanation [[slug]] renders as a clickable wikilink, no back-to-wiki button", () => {
    const onOpenWikiPage = vi.fn()
    render(
      <QuizAnswering
        quizMd={TWO_Q}
        passThreshold={80}
        pages={{
          "auth-middleware": {
            slug: "auth-middleware",
            path: "wiki/modules/auth-middleware.md",
            title: "Auth Middleware Guide",
          },
        }}
        onOpenWikiPage={onOpenWikiPage}
      />,
    )
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("What is auth?")
    fireEvent.click(screen.getByTestId("quiz-choice-B")) // correct
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Correct")
    expect(screen.getByTestId("quiz-explanation")).toHaveTextContent(
      "Auth verifies identity",
    )
    const link = screen.getByTestId("wikilink-auth-middleware")
    // design D6 (corrected): show the page title, never the bracketed slug.
    expect(link).toHaveTextContent("Auth Middleware Guide")
    expect(link.textContent ?? "").not.toContain("[[")
    expect(link.textContent ?? "").not.toContain("]]")
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("auth-middleware")
    expect(screen.queryByTestId("quiz-back-to-wiki")).not.toBeInTheDocument()
  })

  it("incorrect answer: explanation wikilink is still rendered and clickable", () => {
    const onOpenWikiPage = vi.fn()
    render(
      <QuizAnswering
        quizMd={TWO_Q}
        passThreshold={80}
        pages={{
          "auth-middleware": {
            slug: "auth-middleware",
            path: "wiki/modules/auth-middleware.md",
            title: "Auth Middleware Guide",
          },
        }}
        onOpenWikiPage={onOpenWikiPage}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-choice-A")) // answer is B → wrong
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Incorrect")
    const link = screen.getByTestId("wikilink-auth-middleware")
    expect(link).toHaveTextContent("Auth Middleware Guide")
    expect(link.textContent ?? "").not.toContain("[[")
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("auth-middleware")
    expect(screen.queryByTestId("quiz-back-to-wiki")).not.toBeInTheDocument()
  })

  it("renders inline markdown in stem, choices, and explanation", () => {
    const onOpenWikiPage = vi.fn()
    const MD = `## Q1. Why use \`codebus-core\` with **Rust**?

- A) *workspace* modeling
- B) plain text
- C) no parser
- D) raw output

## Answer: A

## Explanation: Use \`read_wiki_page\` with [[desktop-app-workspace]] and **typed** data.`
    render(
      <QuizAnswering
        quizMd={MD}
        passThreshold={80}
        pages={{
          "desktop-app-workspace": {
            slug: "desktop-app-workspace",
            path: "wiki/modules/desktop-app-workspace.md",
            title: "Desktop App Workspace",
          },
        }}
        onOpenWikiPage={onOpenWikiPage}
      />,
    )

    expect(screen.getByText("codebus-core").tagName).toBe("CODE")
    expect(screen.getByText("Rust").tagName).toBe("STRONG")
    expect(screen.getByText("workspace").tagName).toBe("EM")
    expect(screen.getByTestId("quiz-stem")).not.toHaveTextContent("`")
    fireEvent.click(screen.getByTestId("quiz-choice-A"))
    fireEvent.click(screen.getByTestId("quiz-submit"))

    expect(screen.getByText("read_wiki_page").tagName).toBe("CODE")
    expect(screen.getByText("typed").tagName).toBe("STRONG")
    const link = screen.getByTestId("wikilink-desktop-app-workspace")
    expect(link).toHaveClass("cite-link")
    fireEvent.click(link)
    expect(onOpenWikiPage).toHaveBeenCalledWith("desktop-app-workspace")
    expect(screen.getByTestId("quiz-explanation")).not.toHaveTextContent("`")
  })

  // Spec: "Summary applies pass threshold" — 2/2 (100%) ≥ 80 → pass.
  it("summary passes when score meets the threshold", () => {
    render(<QuizAnswering quizMd={TWO_Q} passThreshold={80} />)
    // Q1 correct
    fireEvent.click(screen.getByTestId("quiz-choice-B"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    fireEvent.click(screen.getByTestId("quiz-next"))
    // Q2 correct
    fireEvent.click(screen.getByTestId("quiz-choice-A"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    fireEvent.click(screen.getByTestId("quiz-next")) // Finish
    expect(screen.getByTestId("quiz-summary")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-score")).toHaveTextContent("2 / 2")
    expect(screen.getByTestId("quiz-outcome")).toHaveTextContent("Passed")
  })

  // Below threshold → fail.
  it("summary fails when score is below the threshold", () => {
    render(<QuizAnswering quizMd={TWO_Q} passThreshold={80} />)
    // Q1 wrong
    fireEvent.click(screen.getByTestId("quiz-choice-A"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    fireEvent.click(screen.getByTestId("quiz-next"))
    // Q2 wrong
    fireEvent.click(screen.getByTestId("quiz-choice-B"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    fireEvent.click(screen.getByTestId("quiz-next")) // Finish
    expect(screen.getByTestId("quiz-score")).toHaveTextContent("0 / 2")
    expect(screen.getByTestId("quiz-outcome")).toHaveTextContent("Failed")
  })

  it("renders an empty-state for an unparseable body", () => {
    render(<QuizAnswering quizMd={"not a quiz"} passThreshold={80} />)
    expect(screen.getByTestId("quiz-answering-empty")).toBeInTheDocument()
  })

  // --- quiz-attempt-progress task 3.1 (design D3) ---

  // Spec: app-workspace § Quiz Answering and Summary —
  // "Each submission persists progress".
  it("persists every submission: in_progress per answer, completed on final", () => {
    const onPersist = vi.fn()
    render(
      <QuizAnswering
        quizMd={TWO_Q}
        passThreshold={80}
        onPersist={onPersist}
      />,
    )
    // Q1 submit → in_progress, cursor {q:1,revealed:true} (design D3 final).
    fireEvent.click(screen.getByTestId("quiz-choice-B")) // correct
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(onPersist).toHaveBeenCalledTimes(1)
    const p1 = onPersist.mock.calls[0][0]
    expect(p1.status).toBe("in_progress")
    expect(p1.answers).toEqual([{ q: 1, selected: "B", correct: true }])
    expect(p1.completed_at).toBeNull()
    expect(p1.cursor).toEqual({ q: 1, revealed: true })

    // Next also persists (cursor advanced, answers unchanged).
    fireEvent.click(screen.getByTestId("quiz-next"))
    expect(onPersist).toHaveBeenCalledTimes(2)
    const pNext = onPersist.mock.calls[1][0]
    expect(pNext.status).toBe("in_progress")
    expect(pNext.cursor).toEqual({ q: 2, revealed: false })
    expect(pNext.answers).toHaveLength(1)

    // Q2 submit is the final question → completed + completed_at.
    fireEvent.click(screen.getByTestId("quiz-choice-C")) // answer is A → wrong
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(onPersist).toHaveBeenCalledTimes(3)
    const p2 = onPersist.mock.calls[2][0]
    expect(p2.status).toBe("completed")
    expect(p2.completed_at).toBeTruthy()
    expect(p2.cursor).toEqual({ q: 2, revealed: true })
    expect(p2.answers).toEqual([
      { q: 1, selected: "B", correct: true },
      { q: 2, selected: "C", correct: false },
    ])
  })

  // quiz-attempt-progress task 7.1 (design D3 revised) — spec:
  // app-workspace § Quiz Answering and Summary "Opening an in-progress
  // attempt restores the last answered question" + "Advancing from the
  // restored question continues at the first unanswered".
  it("resume restores the last answered question in its submitted state; Next continues at first unanswered", () => {
    const FIVE_Q = [1, 2, 3, 4, 5]
      .map(
        (n) =>
          `## Q${n}. stem ${n}?\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e${n}`,
      )
      .join("\n\n")
    render(
      <QuizAnswering
        quizMd={FIVE_Q}
        passThreshold={80}
        initialProgress={{
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "B", correct: false },
          ],
          status: "in_progress",
          started_at: "2026-05-18T10:00:00Z",
          completed_at: null,
        }}
      />,
    )
    // Restored at the LAST answered question (Q2), in submitted state.
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 2 of 5",
    )
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("stem 2?")
    // Q2 answer is A; stored selected was B → revealed as Incorrect.
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Incorrect")
    expect(screen.getByTestId("quiz-explanation")).toHaveTextContent("e2")
    // NOT jumped to Q3, NOT restarted at Q1.
    expect(screen.getByTestId("quiz-answering")).not.toHaveTextContent(
      "Question 3 of 5",
    )

    // Next advances to the first unanswered question (Q3), blank.
    fireEvent.click(screen.getByTestId("quiz-next"))
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 3 of 5",
    )
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("stem 3?")
    expect(screen.queryByTestId("quiz-verdict")).not.toBeInTheDocument()
  })

  // quiz-attempt-progress task 11.3 (design D3 final — precise cursor).
  const FIVE_Q = [1, 2, 3, 4, 5]
    .map(
      (n) =>
        `## Q${n}. stem ${n}?\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e${n}`,
    )
    .join("\n\n")

  it("cursor {q:4,revealed:false} over answers Q1-3 → restores Q4 blank (not Q3)", () => {
    render(
      <QuizAnswering
        quizMd={FIVE_Q}
        passThreshold={80}
        initialProgress={{
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "A", correct: true },
            { q: 3, selected: "B", correct: false },
          ],
          status: "in_progress",
          started_at: "2026-05-18T10:00:00Z",
          completed_at: null,
          cursor: { q: 4, revealed: false },
        }}
      />,
    )
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 4 of 5",
    )
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("stem 4?")
    expect(screen.queryByTestId("quiz-verdict")).not.toBeInTheDocument()
  })

  it("cursor {q:2,revealed:true} → restores Q2 submitted (not legacy last-answered Q3)", () => {
    render(
      <QuizAnswering
        quizMd={FIVE_Q}
        passThreshold={80}
        initialProgress={{
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "C", correct: false },
            { q: 3, selected: "A", correct: true },
          ],
          status: "in_progress",
          started_at: "2026-05-18T10:00:00Z",
          completed_at: null,
          cursor: { q: 2, revealed: true },
        }}
      />,
    )
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 2 of 5",
    )
    // Q2 answer is A, stored selected C → revealed Incorrect.
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Incorrect")
    expect(screen.getByTestId("quiz-explanation")).toHaveTextContent("e2")
  })

  it("legacy sidecar without cursor → falls back to last answered (Q2 revealed)", () => {
    render(
      <QuizAnswering
        quizMd={FIVE_Q}
        passThreshold={80}
        initialProgress={{
          schema_version: 1,
          answers: [
            { q: 1, selected: "A", correct: true },
            { q: 2, selected: "B", correct: false },
          ],
          status: "in_progress",
          started_at: "2026-05-18T10:00:00Z",
          completed_at: null,
        }}
      />,
    )
    expect(screen.getByTestId("quiz-answering")).toHaveTextContent(
      "Question 2 of 5",
    )
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Incorrect")
  })

  it("persists cursor on submit (revealed:true) and on Next (next q, revealed:false)", () => {
    const onPersist = vi.fn()
    render(
      <QuizAnswering
        quizMd={FIVE_Q}
        passThreshold={80}
        onPersist={onPersist}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-choice-A")) // Q1 correct
    fireEvent.click(screen.getByTestId("quiz-submit"))
    const afterSubmit = onPersist.mock.calls.at(-1)![0]
    expect(afterSubmit.cursor).toEqual({ q: 1, revealed: true })
    expect(afterSubmit.status).toBe("in_progress")

    fireEvent.click(screen.getByTestId("quiz-next"))
    const afterNext = onPersist.mock.calls.at(-1)![0]
    expect(afterNext.cursor).toEqual({ q: 2, revealed: false })
    expect(afterNext.answers).toHaveLength(1) // unchanged by Next
    expect(afterNext.status).toBe("in_progress")
  })

  // Phase 5.4 quiz-fullscreen-wizard-view: the `embedded` prop is a marker
  // that says "rendered inside the wizard". QuizAnswering already does
  // not carry a back-to-history button (the `[← Back to wiki page]`
  // affordance was removed by quiz-attempt-progress D6); the embedded
  // prop must therefore (a) accept without crashing, and (b) preserve
  // the stem / choices / progress persistence semantics.
  it("embedded=true preserves the existing answering UI (no crash, no extra back button)", () => {
    const { container } = render(
      <QuizAnswering
        quizMd={TWO_Q}
        passThreshold={80}
        embedded={true}
      />,
    )
    expect(screen.getByTestId("quiz-answering")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("What is auth?")
    expect(
      container.querySelector("[data-testid='quiz-attempt-back']"),
    ).toBeNull()
  })
})
