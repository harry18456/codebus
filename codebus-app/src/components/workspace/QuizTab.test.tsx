import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import { QuizTab } from "./QuizTab"

describe("QuizTab", () => {
  it("QuizTab_v1_placeholder_renders_exact_text", () => {
    const { container } = render(<QuizTab />)
    expect(screen.getByTestId("quiz-tab")).toHaveTextContent(
      "Coming soon — quiz flow ships in v3-app-quiz",
    )
    // No interactive controls.
    expect(container.querySelectorAll("button")).toHaveLength(0)
    expect(container.querySelectorAll("input")).toHaveLength(0)
    expect(container.querySelectorAll("select")).toHaveLength(0)
    expect(container.querySelectorAll("textarea")).toHaveLength(0)
  })
})
