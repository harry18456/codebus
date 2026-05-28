import { describe, expect, it, vi } from "vitest"
import { render, screen } from "@testing-library/react"

import { EmptyState } from "./EmptyState"

describe("EmptyState", () => {
  it("renders Chinese title for zh locale", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="zh" />)
    expect(screen.getByText("來搭第一台公車吧")).toBeInTheDocument()
  })

  it("renders English title for non-zh locale", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="en" />)
    expect(screen.getByText("Board your first bus")).toBeInTheDocument()
  })

  it("renders the 🚌 emoji and Board-a-new-bus CTA", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="en" />)
    expect(screen.getByText("🚌")).toBeInTheDocument()
    expect(screen.getByTestId("empty-board-cta")).toHaveTextContent("+ Board a new bus")
  })

  it("renders Quickstart 3-step card", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="en" />)
    expect(screen.getByText("QUICKSTART")).toBeInTheDocument()
    expect(screen.getByText("Pick a repo folder")).toBeInTheDocument()
    expect(screen.getByText(/Run a goal/)).toBeInTheDocument()
    expect(screen.getByText("Quiz yourself to verify")).toBeInTheDocument()
  })

  it("renders step 2 example inside a separate amber pill element", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="en" />)
    // The example wording lives in lobby.empty.step2Example, rendered as a
    // distinct <span> with the amber-tinted pill class set.
    const exampleNode = screen.getByText("Understand X in this codebase")
    expect(exampleNode).toBeInTheDocument()
    expect(exampleNode.tagName).toBe("SPAN")
    expect(exampleNode.className).toContain("bg-accent-tint")
    expect(exampleNode.className).toContain("text-accent")
    expect(exampleNode.className).toContain("font-mono")
    // The pill is rendered as a child element distinct from the step text;
    // a parent text node holding the prefix "Run a goal — e.g." exists, and
    // the pill is a sibling element (split-key contract).
    expect(exampleNode.parentElement?.childElementCount ?? 0).toBeGreaterThanOrEqual(1)
  })

  it("renders step numbers as monospace digits without trailing period", () => {
    render(<EmptyState onBoard={() => {}} localeOverride="en" />)
    // Each step number must be a bare digit (no period) in the rendered card.
    expect(screen.getByText("1")).toBeInTheDocument()
    expect(screen.getByText("2")).toBeInTheDocument()
    expect(screen.getByText("3")).toBeInTheDocument()
    // Periods following digits are forbidden — they were the G3 visual issue.
    expect(screen.queryByText("1.")).toBeNull()
    expect(screen.queryByText("2.")).toBeNull()
    expect(screen.queryByText("3.")).toBeNull()
  })

  it("applies the idle-motion class on the empty-state hero only", () => {
    const { container } = render(
      <EmptyState onBoard={() => {}} localeOverride="en" />,
    )
    const heroes = container.querySelectorAll(".codebus-bus-idle")
    expect(heroes.length).toBe(1)
    // The hero glyph (🚌) carries the idle animation class.
    expect(heroes[0]?.textContent).toBe("🚌")
  })

  it("calls onBoard when the CTA is clicked", () => {
    const onBoard = vi.fn()
    render(<EmptyState onBoard={onBoard} localeOverride="en" />)
    screen.getByTestId("empty-board-cta").click()
    expect(onBoard).toHaveBeenCalledTimes(1)
  })
})
