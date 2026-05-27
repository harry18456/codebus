import { describe, expect, it } from "vitest"
import { render, screen } from "@testing-library/react"

import { TabContentHeader } from "./TabContentHeader"

describe("TabContentHeader", () => {
  it("renders title-only with no subtitle, cta, or shortcut chip", () => {
    const { container } = render(<TabContentHeader title="Goals" />)
    expect(screen.getByRole("heading", { level: 1, name: "Goals" })).toBeInTheDocument()
    expect(container.querySelector("p")).toBeNull()
    expect(container.querySelector("[data-tch-cta]")).toBeNull()
    expect(container.querySelector("[data-tch-chip]")).toBeNull()
  })

  it("renders subtitle below the h1 when provided", () => {
    render(
      <TabContentHeader
        title="Goals"
        subtitle="List what you want to understand"
      />,
    )
    expect(screen.getByRole("heading", { level: 1, name: "Goals" })).toBeInTheDocument()
    expect(screen.getByText("List what you want to understand")).toBeInTheDocument()
  })

  it("renders CTA on the right side without a shortcut chip", () => {
    const { container } = render(
      <TabContentHeader
        title="Quiz"
        cta={<button data-testid="cta-btn">+ New quiz</button>}
      />,
    )
    expect(screen.getByTestId("cta-btn")).toBeInTheDocument()
    expect(container.querySelector("[data-tch-chip]")).toBeNull()
  })

  it("renders CTA and a shortcut chip with aria-hidden when both are provided", () => {
    const { container } = render(
      <TabContentHeader
        title="Goals"
        cta={<button data-testid="cta-btn">+ New goal</button>}
        shortcutChipText="N"
      />,
    )
    expect(screen.getByTestId("cta-btn")).toBeInTheDocument()
    const chip = container.querySelector("[data-tch-chip]")
    expect(chip).not.toBeNull()
    expect(chip?.textContent).toBe("N")
    expect(chip?.getAttribute("aria-hidden")).toBe("true")
  })

  it("suppresses the shortcut chip when no CTA is provided (chip is meaningless alone)", () => {
    const { container } = render(
      <TabContentHeader title="Goals" shortcutChipText="N" />,
    )
    expect(container.querySelector("[data-tch-chip]")).toBeNull()
  })

  it("applies the given testId as data-testid on the root row element", () => {
    const { container } = render(
      <TabContentHeader title="Goals" testId="tab-content-header-goals" />,
    )
    const root = container.firstElementChild
    expect(root).not.toBeNull()
    expect(root?.getAttribute("data-testid")).toBe("tab-content-header-goals")
    // Root row keeps the Tauri drag region so the header still drags the window.
    expect(root?.getAttribute("data-tauri-drag-region")).not.toBeNull()
  })
})
