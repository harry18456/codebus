import { describe, expect, it } from "vitest"
import { render } from "@testing-library/react"

import { PhaseDots } from "./PhaseDots"

describe("PhaseDots", () => {
  it("renders `total` dots with the active dot at `current`", () => {
    const { container } = render(<PhaseDots total={4} current={2} />)
    const dots = container.querySelectorAll("[data-phase-index]")
    expect(dots).toHaveLength(4)

    expect(dots[0]?.getAttribute("data-phase-state")).toBe("done")
    expect(dots[1]?.getAttribute("data-phase-state")).toBe("current")
    expect(dots[2]?.getAttribute("data-phase-state")).toBe("pending")
    expect(dots[3]?.getAttribute("data-phase-state")).toBe("pending")
  })

  it("supports 6 dots (LoadingOverlay use case) with current=5 active", () => {
    const { container } = render(<PhaseDots total={6} current={5} />)
    const dots = container.querySelectorAll("[data-phase-index]")
    expect(dots).toHaveLength(6)
    expect(dots[4]?.getAttribute("data-phase-state")).toBe("current")
    expect(dots[4]?.className).toContain("bg-accent")
    expect(dots[4]?.className).toContain("ring-accent-tint")
  })

  it("renders the active dot with bg-warn when state=error (amber-warm failure)", () => {
    const { container } = render(
      <PhaseDots total={6} current={3} state="error" />,
    )
    const dots = container.querySelectorAll("[data-phase-index]")
    expect(dots[2]?.getAttribute("data-phase-state")).toBe("error")
    expect(dots[2]?.className).toContain("bg-warn")
    expect(dots[2]?.className).not.toContain("bg-accent")
  })

  it("renders the active dot as done (no ring) when state=done", () => {
    const { container } = render(
      <PhaseDots total={6} current={6} state="done" />,
    )
    const dots = container.querySelectorAll("[data-phase-index]")
    expect(dots[5]?.getAttribute("data-phase-state")).toBe("done")
    expect(dots[5]?.className).toContain("bg-fg-tertiary")
  })

  it("attaches testId + custom currentAttrName (quiz wizard compatibility)", () => {
    const { container } = render(
      <PhaseDots
        total={4}
        current={3}
        testId="quiz-wizard-step-dots"
        currentAttrName="current-step"
      />,
    )
    const root = container.querySelector(
      "[data-testid='quiz-wizard-step-dots']",
    )
    expect(root).not.toBeNull()
    expect(root?.getAttribute("data-current-step")).toBe("3")
  })

  it("uses data-phase as the default attribute name when none is given", () => {
    const { container } = render(<PhaseDots total={6} current={2} />)
    const root = container.firstElementChild
    expect(root?.getAttribute("data-phase")).toBe("2")
  })
})
