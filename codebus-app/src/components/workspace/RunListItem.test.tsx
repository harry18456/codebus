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
  // Spec: app-workspace § Goals Overview List and Filter — row icon
  // mapping table. Parametrized over the five outcomes.
  it.each([
    ["running", "🚌"],
    ["succeeded", "✓"],
    ["cancelled", "⏹"],
    ["failed", "⚠"],
    ["interrupted", "⚠"],
  ])("RunListItem_icon_per_outcome (%s → %s)", (outcome, icon) => {
    render(<RunListItem run={makeRun({ outcome })} onClick={() => {}} />)
    expect(screen.getByTestId("run-row-r1")).toHaveTextContent(icon)
  })

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
