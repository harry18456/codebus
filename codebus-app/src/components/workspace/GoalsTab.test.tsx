import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import type { RunLogSummary } from "@/lib/ipc"
import { GoalsTab } from "./GoalsTab"
import { useGoalsStore } from "@/store/goals"

function makeRun(id: string, startedAt: string): RunLogSummary {
  return {
    run_id: id,
    mode: "goal",
    goal: `goal ${id}`,
    started_at: startedAt,
    finished_at: startedAt,
    tokens: { input_tokens: 0, output_tokens: 0 },
    wiki_changed: false,
    lint_error_count: 0,
    lint_warn_count: 0,
    outcome: "succeeded",
  }
}

describe("GoalsTab", () => {
  beforeEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("GoalsTab_renders_runs_in_descending_started_at_order", () => {
    useGoalsStore.setState({
      runs: [
        makeRun("a", "2026-05-13T10:00:00Z"),
        makeRun("b", "2026-05-13T12:00:00Z"),
        makeRun("c", "2026-05-13T11:00:00Z"),
      ],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    const rows = screen.getAllByTestId(/^run-row-/)
    expect(rows.map((r) => r.dataset.testid)).toEqual([
      "run-row-b", // 12:00 (newest)
      "run-row-c", // 11:00
      "run-row-a", // 10:00
    ])
  })

  it("GoalsTab_empty_state_shows_hint_with_three_prefill_rows", () => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    expect(screen.getByTestId("goals-empty")).toHaveTextContent(
      "Click + New goal to ask codebus to ingest something into the wiki",
    )
    const prefills = [
      screen.getByTestId("goals-empty-prefill-0"),
      screen.getByTestId("goals-empty-prefill-1"),
      screen.getByTestId("goals-empty-prefill-2"),
    ]
    expect(prefills).toHaveLength(3)

    fireEvent.click(prefills[0])
    expect(screen.getByTestId("new-goal-modal")).toBeInTheDocument()
    // Modal opens with the prefill text in its textarea.
    const textarea = screen.getByTestId(
      "new-goal-textarea",
    ) as HTMLTextAreaElement
    expect(textarea.value).toBe(prefills[0].textContent?.replaceAll("“", "").replaceAll("”", "") ?? "")
  })

  it("filters non-goal modes out of the visible list", () => {
    useGoalsStore.setState({
      runs: [
        makeRun("g", "2026-05-13T10:00:00Z"),
        { ...makeRun("c", "2026-05-13T11:00:00Z"), mode: "chat" },
      ],
      activeRun: null,
    })
    render(<GoalsTab vaultPath="/v" onSelectRun={() => {}} />)
    expect(screen.queryByTestId("run-row-c")).toBeNull()
    expect(screen.getByTestId("run-row-g")).toBeInTheDocument()
  })
})
