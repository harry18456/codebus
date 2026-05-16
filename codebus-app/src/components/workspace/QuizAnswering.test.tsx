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
  // Spec: app-workspace § Quiz Answering and Summary —
  // "Correct answer revealed without spawn".
  it("grades a correct answer client-side, shows explanation, no wiki-return", () => {
    render(<QuizAnswering quizMd={TWO_Q} passThreshold={80} />)
    expect(screen.getByTestId("quiz-stem")).toHaveTextContent("What is auth?")
    fireEvent.click(screen.getByTestId("quiz-choice-B"))
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Correct")
    expect(screen.getByTestId("quiz-explanation")).toHaveTextContent(
      "Auth verifies identity",
    )
    expect(screen.queryByTestId("quiz-back-to-wiki")).not.toBeInTheDocument()
  })

  // Spec: "Incorrect answer offers wiki return".
  it("incorrect answer shows [← Back to wiki page] and fires callback", () => {
    const onBack = vi.fn()
    render(
      <QuizAnswering quizMd={TWO_Q} passThreshold={80} onBackToWiki={onBack} />,
    )
    fireEvent.click(screen.getByTestId("quiz-choice-A")) // answer is B
    fireEvent.click(screen.getByTestId("quiz-submit"))
    expect(screen.getByTestId("quiz-verdict")).toHaveTextContent("Incorrect")
    const back = screen.getByTestId("quiz-back-to-wiki")
    expect(back).toBeInTheDocument()
    fireEvent.click(back)
    expect(onBack).toHaveBeenCalledTimes(1)
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
})
