import { describe, expect, it } from "vitest"
import { render, screen } from "@testing-library/react"

import { SectionLabel } from "./SectionLabel"

describe("SectionLabel", () => {
  it("renders default variant with section-label class and children text", () => {
    const { container } = render(<SectionLabel>最近</SectionLabel>)
    expect(screen.getByText("最近")).toBeInTheDocument()
    const root = container.firstElementChild
    expect(root).not.toBeNull()
    expect(root?.classList.contains("section-label")).toBe(true)
    expect(root?.classList.contains("section-label--caps")).toBe(false)
  })

  it("renders caps variant with section-label--caps class", () => {
    const { container } = render(<SectionLabel variant="caps">Modules</SectionLabel>)
    const root = container.firstElementChild
    expect(root?.classList.contains("section-label")).toBe(true)
    expect(root?.classList.contains("section-label--caps")).toBe(true)
    expect(screen.getByText("Modules")).toBeInTheDocument()
  })

  it("renders count slot with section-label__count class when count is provided", () => {
    const { container } = render(<SectionLabel count={3}>最近</SectionLabel>)
    const root = container.firstElementChild
    const countNode = root?.querySelector(".section-label__count")
    expect(countNode).not.toBeNull()
    expect(countNode?.textContent).toBe("3")
  })

  it("renders count slot for zero (a meaningful count value)", () => {
    const { container } = render(<SectionLabel count={0}>最近</SectionLabel>)
    const countNode = container.firstElementChild?.querySelector(".section-label__count")
    expect(countNode?.textContent).toBe("0")
  })

  it("omits count slot when count prop is not provided", () => {
    const { container } = render(<SectionLabel>最近</SectionLabel>)
    const countNode = container.firstElementChild?.querySelector(".section-label__count")
    expect(countNode).toBeNull()
  })

  it("accepts string count values", () => {
    const { container } = render(<SectionLabel count="3 / 3">最近</SectionLabel>)
    const countNode = container.firstElementChild?.querySelector(".section-label__count")
    expect(countNode?.textContent).toBe("3 / 3")
  })

  it("merges caller className with internal section-label class", () => {
    const { container } = render(
      <SectionLabel className="my-custom-cls">最近</SectionLabel>,
    )
    const root = container.firstElementChild
    expect(root?.classList.contains("section-label")).toBe(true)
    expect(root?.classList.contains("my-custom-cls")).toBe(true)
  })

  it("does not expose the amber bar as accessible content (decorative ::before)", () => {
    const { container } = render(<SectionLabel>最近</SectionLabel>)
    const root = container.firstElementChild
    // ::before pseudo-element renders the amber bar; it must not appear as a
    // child element in the DOM/aria tree. Only the children + optional count
    // slot are real elements.
    const childElements = root?.querySelectorAll(":scope > *")
    // With no count, the children render as text node only — zero child elements.
    expect(childElements?.length).toBe(0)
  })

  it("renders count as a child element but keeps amber bar pseudo-only", () => {
    const { container } = render(<SectionLabel count={3}>最近</SectionLabel>)
    const root = container.firstElementChild
    const childElements = root?.querySelectorAll(":scope > *")
    // With count, exactly one child element (the count node); amber bar is
    // still pseudo-element and not counted as a child.
    expect(childElements?.length).toBe(1)
    expect(childElements?.[0].classList.contains("section-label__count")).toBe(true)
  })
})
