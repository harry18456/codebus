import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { QuizWizardGenerating } from "./QuizWizardGenerating"

const EMPTY_EVENTS: never[] = []

describe("QuizWizardGenerating", () => {
  it("renders the three brand banners (topic / codebus generating / thinking)", () => {
    render(
      <QuizWizardGenerating
        topic="專案目的"
        scopePages={[
          "synthesis/project-purpose.md",
          "modules/desktop-workspace.md",
        ]}
        events={EMPTY_EVENTS}
        onCancel={vi.fn()}
      />,
    )
    expect(screen.getByTestId("quiz-wizard-banner-topic")).toBeInTheDocument()
    expect(screen.getByTestId("quiz-wizard-banner-codebus")).toBeInTheDocument()
    expect(
      screen.getByTestId("quiz-wizard-banner-thinking"),
    ).toBeInTheDocument()
  })

  it("topic banner surfaces the topic name and codebus banner surfaces the scope page list", () => {
    render(
      <QuizWizardGenerating
        topic="專案目的"
        scopePages={[
          "synthesis/project-purpose.md",
          "modules/desktop-workspace.md",
        ]}
        events={EMPTY_EVENTS}
        onCancel={vi.fn()}
      />,
    )
    expect(
      screen.getByTestId("quiz-wizard-banner-topic"),
    ).toHaveTextContent("專案目的")
    const codebus = screen.getByTestId("quiz-wizard-banner-codebus")
    expect(codebus).toHaveTextContent("synthesis/project-purpose.md")
    expect(codebus).toHaveTextContent("modules/desktop-workspace.md")
  })

  it("cancel control calls onCancel", () => {
    const onCancel = vi.fn()
    render(
      <QuizWizardGenerating
        topic="專案目的"
        scopePages={[]}
        events={EMPTY_EVENTS}
        onCancel={onCancel}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-generating-cancel"))
    expect(onCancel).toHaveBeenCalledTimes(1)
  })

  it("when events arrive, the stream tail area renders cluster/timeline rows (not just a static placeholder)", () => {
    render(
      <QuizWizardGenerating
        topic="auth"
        scopePages={[]}
        events={[
          {
            kind: "stream",
            data: {
              kind: "tool_use",
              tool_name: "Read",
              input: { file_path: "wiki/synthesis/project-purpose.md" },
              tool_use_id: "tu-1",
            } as never,
          } as never,
        ]}
        onCancel={vi.fn()}
      />,
    )
    const tail = screen.getByTestId("quiz-wizard-generating-stream-tail")
    expect(tail).toBeInTheDocument()
    expect(tail.children.length).toBeGreaterThan(0)
  })
})
