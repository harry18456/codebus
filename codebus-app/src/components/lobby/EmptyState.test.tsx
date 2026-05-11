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

  it("calls onBoard when the CTA is clicked", () => {
    const onBoard = vi.fn()
    render(<EmptyState onBoard={onBoard} localeOverride="en" />)
    screen.getByTestId("empty-board-cta").click()
    expect(onBoard).toHaveBeenCalledTimes(1)
  })
})
