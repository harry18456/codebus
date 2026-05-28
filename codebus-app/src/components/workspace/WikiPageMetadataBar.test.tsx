import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { WikiPageMetadataBar } from "./WikiPageMetadataBar"

const NOW = new Date("2026-05-28T12:00:00Z")
const TEN_MIN_AGO = new Date("2026-05-28T11:50:00Z").toISOString()

describe("WikiPageMetadataBar", () => {
  it("renders all three segments when goalLast and updatedIso and wikilinkCount are present", () => {
    render(
      <WikiPageMetadataBar
        goalLast="g2"
        updatedIso={TEN_MIN_AGO}
        wikilinkCount={3}
        onGoalClick={() => {}}
        now={NOW}
      />,
    )
    const bar = screen.getByTestId("wiki-page-metadata-bar")
    expect(bar.textContent).toMatch(/Last updated by/)
    expect(bar.textContent).toMatch(/g2/)
    expect(bar.textContent).toMatch(/10m ago|10 分鐘前/)
    expect(bar.textContent).toMatch(/3 sources|3 處引用/)
  })

  it("authoring goal name is clickable and invokes onGoalClick", () => {
    const onGoalClick = vi.fn()
    render(
      <WikiPageMetadataBar
        goalLast="auth-flow"
        updatedIso={TEN_MIN_AGO}
        wikilinkCount={2}
        onGoalClick={onGoalClick}
        now={NOW}
      />,
    )
    const goalBtn = screen.getByTestId("wiki-page-metadata-goal")
    expect(goalBtn.textContent).toBe("auth-flow")
    fireEvent.click(goalBtn)
    expect(onGoalClick).toHaveBeenCalledTimes(1)
    expect(onGoalClick).toHaveBeenCalledWith("auth-flow")
  })

  it("suppresses the Last updated by segment when goalLast is null", () => {
    render(
      <WikiPageMetadataBar
        goalLast={null}
        updatedIso={TEN_MIN_AGO}
        wikilinkCount={2}
        onGoalClick={() => {}}
        now={NOW}
      />,
    )
    const bar = screen.getByTestId("wiki-page-metadata-bar")
    expect(bar.textContent).not.toMatch(/Last updated by|最後更新者/)
    expect(screen.queryByTestId("wiki-page-metadata-goal")).toBeNull()
    expect(bar.textContent).toMatch(/2 sources|2 處引用/)
  })

  it("suppresses the sources segment when wikilinkCount is 0", () => {
    render(
      <WikiPageMetadataBar
        goalLast="g1"
        updatedIso={TEN_MIN_AGO}
        wikilinkCount={0}
        onGoalClick={() => {}}
        now={NOW}
      />,
    )
    const bar = screen.getByTestId("wiki-page-metadata-bar")
    expect(bar.textContent).toMatch(/g1/)
    expect(bar.textContent).not.toMatch(/sources|處引用/)
  })

  it("suppresses the time-ago segment when updatedIso is unparseable", () => {
    render(
      <WikiPageMetadataBar
        goalLast="g1"
        updatedIso="not-a-date"
        wikilinkCount={1}
        onGoalClick={() => {}}
        now={NOW}
      />,
    )
    const bar = screen.getByTestId("wiki-page-metadata-bar")
    expect(bar.textContent).toMatch(/g1/)
    expect(bar.textContent).toMatch(/1 sources|1 處引用/)
    expect(bar.textContent).not.toMatch(/ago|前/)
  })

  it("renders nothing when all three segments are suppressed", () => {
    const { container } = render(
      <WikiPageMetadataBar
        goalLast={null}
        updatedIso="garbage"
        wikilinkCount={0}
        onGoalClick={() => {}}
        now={NOW}
      />,
    )
    expect(container.querySelector('[data-testid="wiki-page-metadata-bar"]')).toBeNull()
  })
})
