import { render, screen, fireEvent } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { BUCKET_IDS, type ScopeBuckets } from "@/store/quiz-wizard"
import { QuizWizardScopeConfirm } from "./QuizWizardScopeConfirm"

const BUCKETS: ScopeBuckets = {
  modules: ["modules/auth.md", "modules/session.md"],
  processes: ["processes/login.md"],
  synthesis: ["synthesis/purpose.md"],
  concepts: [],
  entities: ["entities/user.md"],
}

describe("QuizWizardScopeConfirm", () => {
  it("renders the five bucket sections in the required order (modules → processes → synthesis → concepts → entities)", () => {
    const { container } = render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    const sections = container.querySelectorAll("[data-bucket-id]")
    const ids = Array.from(sections).map((node) =>
      node.getAttribute("data-bucket-id"),
    )
    expect(ids).toEqual([
      "modules",
      "processes",
      "synthesis",
      "concepts",
      "entities",
    ])
  })

  it("renders all five BUCKET_IDS even when a bucket plan is empty (empty state, not hidden)", () => {
    render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    // `concepts` is empty in BUCKETS — section MUST still render.
    expect(
      screen.getByTestId("quiz-wizard-bucket-concepts"),
    ).toBeInTheDocument()
  })

  it("each bucket section carries the english identifier in data-bucket-id even when label is localized", () => {
    render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={vi.fn()}
        onBack={vi.fn()}
      />,
    )
    for (const id of BUCKET_IDS) {
      const section = screen.getByTestId(`quiz-wizard-bucket-${id}`)
      expect(section.getAttribute("data-bucket-id")).toBe(id)
    }
  })

  it("confirming with all buckets selected calls onConfirm with all five identifier strings", () => {
    const onConfirm = vi.fn()
    render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={onConfirm}
        onBack={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-scope-confirm"))
    expect(onConfirm).toHaveBeenCalledTimes(1)
    expect(onConfirm.mock.calls[0]?.[0]).toEqual([
      "modules",
      "processes",
      "synthesis",
      "concepts",
      "entities",
    ])
  })

  it("deselecting the processes bucket excludes it from the onConfirm payload", () => {
    const onConfirm = vi.fn()
    render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={onConfirm}
        onBack={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-bucket-toggle-processes"))
    fireEvent.click(screen.getByTestId("quiz-wizard-scope-confirm"))
    expect(onConfirm).toHaveBeenCalledTimes(1)
    const selected = onConfirm.mock.calls[0]?.[0] as string[]
    expect(selected).not.toContain("processes")
    expect(selected).toEqual([
      "modules",
      "synthesis",
      "concepts",
      "entities",
    ])
  })

  it("back-to-topic button calls onBack and not onConfirm", () => {
    const onConfirm = vi.fn()
    const onBack = vi.fn()
    render(
      <QuizWizardScopeConfirm
        buckets={BUCKETS}
        onConfirm={onConfirm}
        onBack={onBack}
      />,
    )
    fireEvent.click(screen.getByTestId("quiz-wizard-scope-back"))
    expect(onBack).toHaveBeenCalledTimes(1)
    expect(onConfirm).not.toHaveBeenCalled()
  })
})
