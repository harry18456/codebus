import { fireEvent, render, screen } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import type { RunLogSummary } from "@/lib/ipc"

import { RunListItem } from "./RunListItem"

function makeRun(over: Partial<RunLogSummary> = {}): RunLogSummary {
  return {
    run_id: "r1",
    mode: "goal",
    goal: "describe X",
    started_at: "2026-05-13T10:00:00Z",
    finished_at: "2026-05-13T10:01:00Z",
    tokens: { input_tokens: 0, output_tokens: 0 },
    wiki_changed: false,
    lint_error_count: 0,
    lint_warn_count: 0,
    outcome: "succeeded",
    ...over,
  }
}

describe("RunListItem", () => {
  // Spec: design-system § Status visuals consume StatusPill — non-running
  // outcomes render the three-state dot; running keeps the 🚌 motif since
  // the dot variant excludes running by design (motion > color dot).
  it("RunListItem_running_outcome_renders_bus_glyph_with_pulse", () => {
    render(<RunListItem run={makeRun({ outcome: "running" })} onClick={() => {}} />)
    const row = screen.getByTestId("run-row-r1")
    expect(row).toHaveTextContent("🚌")
  })

  it.each([
    ["succeeded", "bg-status-done"],
    ["cancelled", "bg-status-interrupted"],
    ["interrupted", "bg-status-interrupted"],
    ["failed", "bg-status-failed"],
  ])(
    "RunListItem_status_dot_per_outcome (%s → %s)",
    (outcome, dotClass) => {
      const { container } = render(
        <RunListItem run={makeRun({ outcome })} onClick={() => {}} />,
      )
      const dot = container.querySelector(`.${dotClass}`)
      expect(dot).not.toBeNull()
    },
  )

  it("RunListItem_click_navigates_to_detail", () => {
    const onClick = vi.fn()
    const run = makeRun()
    render(<RunListItem run={run} onClick={onClick} />)
    fireEvent.click(screen.getByTestId("run-row-r1"))
    expect(onClick).toHaveBeenCalledWith(run)
  })

  it("truncates long goal text with ellipsis", () => {
    const longGoal = "a".repeat(200)
    render(<RunListItem run={makeRun({ goal: longGoal })} onClick={() => {}} />)
    expect(screen.getByTestId("run-row-r1")).toHaveTextContent("…")
  })
})
