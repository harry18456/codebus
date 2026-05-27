import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { QuizWizardTopic } from "./QuizWizardTopic"

describe("QuizWizardTopic", () => {
  it("renders the input with i18n placeholder and the example pills", () => {
    render(
      <QuizWizardTopic
        examplePills={["Auth flow", "IM adapter"]}
        onSubmit={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-wizard-topic-input")).toBeInTheDocument()
    expect(screen.getByText("Auth flow")).toBeInTheDocument()
    expect(screen.getByText("IM adapter")).toBeInTheDocument()
  })

  it("clicking an example pill fills the input value", () => {
    render(
      <QuizWizardTopic
        examplePills={["Auth flow"]}
        onSubmit={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-topic-pill-0"))
    const input = screen.getByTestId(
      "quiz-wizard-topic-input",
    ) as HTMLInputElement
    expect(input.value).toBe("Auth flow")
  })

  it("submitting a non-empty topic calls onSubmit with the trimmed value", () => {
    const onSubmit = vi.fn()
    render(<QuizWizardTopic examplePills={[]} onSubmit={onSubmit} />)
    const input = screen.getByTestId(
      "quiz-wizard-topic-input",
    ) as HTMLInputElement
    fireEvent.change(input, { target: { value: "  Auth flow  " } })
    fireEvent.click(screen.getByTestId("quiz-wizard-topic-next"))
    expect(onSubmit).toHaveBeenCalledWith("Auth flow")
  })

  it("Enter key on input submits the topic", () => {
    const onSubmit = vi.fn()
    render(<QuizWizardTopic examplePills={[]} onSubmit={onSubmit} />)
    const input = screen.getByTestId("quiz-wizard-topic-input")
    fireEvent.change(input, { target: { value: "Auth flow" } })
    fireEvent.keyDown(input, { key: "Enter" })
    expect(onSubmit).toHaveBeenCalledWith("Auth flow")
  })

  it("empty submit does not call onSubmit; input gains error state and tooltip", () => {
    const onSubmit = vi.fn()
    const { container } = render(
      <QuizWizardTopic examplePills={[]} onSubmit={onSubmit} />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-topic-next"))
    expect(onSubmit).not.toHaveBeenCalled()
    const input = screen.getByTestId("quiz-wizard-topic-input")
    expect(input.getAttribute("data-invalid")).toBe("true")
    const tooltip = container.querySelector(
      "[data-testid='quiz-wizard-topic-empty-tooltip']",
    )
    expect(tooltip).not.toBeNull()
  })

  it("the Next button remains enabled even on empty input (per v1.1 mock — no disabled affordance)", () => {
    render(<QuizWizardTopic examplePills={[]} onSubmit={vi.fn()} />)
    const next = screen.getByTestId(
      "quiz-wizard-topic-next",
    ) as HTMLButtonElement
    expect(next.disabled).toBe(false)
  })
})
