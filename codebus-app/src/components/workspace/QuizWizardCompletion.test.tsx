import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { QuizWizardCompletion } from "./QuizWizardCompletion"

describe("QuizWizardCompletion", () => {
  it("renders the fail hero icon and the 'view wrong questions' action on a failed attempt", () => {
    render(
      <QuizWizardCompletion
        topic="專案目的"
        result={{ score: 2, total: 5, wrong: [2, 3, 5] }}
        passed={false}
        threshold={69}
        onRedo={vi.fn()}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-wizard-completion-hero-fail")).toBeInTheDocument()
    expect(
      screen.queryByTestId("quiz-wizard-completion-hero-pass"),
    ).toBeNull()
    expect(screen.getByTestId("quiz-wizard-completion-view-wrong")).toBeInTheDocument()
    expect(
      screen.queryByTestId("quiz-wizard-completion-view-process"),
    ).toBeNull()
  })

  it("renders the pass hero icon and the 'view process' action on a passed attempt", () => {
    render(
      <QuizWizardCompletion
        topic="auth"
        result={{ score: 4, total: 5, wrong: [4] }}
        passed={true}
        threshold={69}
        onRedo={vi.fn()}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-wizard-completion-hero-pass")).toBeInTheDocument()
    expect(
      screen.queryByTestId("quiz-wizard-completion-hero-fail"),
    ).toBeNull()
    expect(
      screen.getByTestId("quiz-wizard-completion-view-process"),
    ).toBeInTheDocument()
    expect(
      screen.queryByTestId("quiz-wizard-completion-view-wrong"),
    ).toBeNull()
  })

  it("renders the wrong-question mono list below the action row when wrong[] is non-empty", () => {
    render(
      <QuizWizardCompletion
        topic="x"
        result={{ score: 2, total: 5, wrong: [2, 3, 5] }}
        passed={false}
        threshold={69}
        onRedo={vi.fn()}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    const list = screen.getByTestId("quiz-wizard-completion-wrong-list")
    expect(list).toHaveTextContent("Q2")
    expect(list).toHaveTextContent("Q3")
    expect(list).toHaveTextContent("Q5")
  })

  it("does not render the wrong list when wrong[] is empty (e.g. perfect score)", () => {
    render(
      <QuizWizardCompletion
        topic="x"
        result={{ score: 5, total: 5, wrong: [] }}
        passed={true}
        threshold={69}
        onRedo={vi.fn()}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    expect(
      screen.queryByTestId("quiz-wizard-completion-wrong-list"),
    ).toBeNull()
  })

  it("the redo button calls onRedo", () => {
    const onRedo = vi.fn()
    render(
      <QuizWizardCompletion
        topic="x"
        result={{ score: 2, total: 5, wrong: [2] }}
        passed={false}
        threshold={69}
        onRedo={onRedo}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-completion-redo"))
    expect(onRedo).toHaveBeenCalledTimes(1)
  })

  it("does not render a back-to-history affordance inside the body (the wizard TabContentHeader provides it)", () => {
    const { container } = render(
      <QuizWizardCompletion
        topic="x"
        result={{ score: 4, total: 5, wrong: [4] }}
        passed={true}
        threshold={69}
        onRedo={vi.fn()}
        onViewWrong={vi.fn()}
        onViewProcess={vi.fn()}
      />,
    )
    expect(
      container.querySelector(
        "[data-testid='quiz-wizard-completion-back-to-history']",
      ),
    ).toBeNull()
  })
})
