import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@/hooks/useLocale", () => ({
  useLocale: vi.fn(() => "en"),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import type { RunLogSummary, VerbEvent } from "@/lib/ipc"
import { useLocale } from "@/hooks/useLocale"
import { useGoalsStore } from "@/store/goals"

import { RunListItem } from "./RunListItem"

const mockedUseLocale = vi.mocked(useLocale)

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

  describe("running row stream tail", () => {
    beforeEach(() => {
      useGoalsStore.setState({ tailByRunId: {} })
    })

    afterEach(() => {
      useGoalsStore.setState({ tailByRunId: {} })
    })

    const toolUseRead: VerbEvent = {
      kind: "stream",
      data: {
        kind: "tool_use",
        name: "Read",
        input: { file_path: "raw/code/auth.rs" },
      },
    }

    it("renders run-row-tail when outcome is running and tail slot has an event", () => {
      useGoalsStore.setState({ tailByRunId: { r1: toolUseRead } })
      render(
        <RunListItem run={makeRun({ outcome: "running" })} onClick={() => {}} />,
      )
      const tail = screen.getByTestId("run-row-tail")
      expect(tail).toBeDefined()
      expect(tail.textContent).toContain("Read")
      expect(tail.textContent).toContain("auth.rs")
    })

    it("renders placeholder when outcome is running but tail slot is empty", () => {
      render(
        <RunListItem run={makeRun({ outcome: "running" })} onClick={() => {}} />,
      )
      const tail = screen.getByTestId("run-row-tail")
      expect(tail.textContent).toBe("…")
    })

    it("does not render run-row-tail when outcome is succeeded even if tail slot has an event", () => {
      useGoalsStore.setState({ tailByRunId: { r1: toolUseRead } })
      render(
        <RunListItem
          run={makeRun({ outcome: "succeeded" })}
          onClick={() => {}}
        />,
      )
      expect(screen.queryByTestId("run-row-tail")).toBeNull()
    })

    it.each(["cancelled", "interrupted", "failed"])(
      "does not render run-row-tail when outcome is %s",
      (outcome) => {
        useGoalsStore.setState({ tailByRunId: { r1: toolUseRead } })
        render(
          <RunListItem run={makeRun({ outcome })} onClick={() => {}} />,
        )
        expect(screen.queryByTestId("run-row-tail")).toBeNull()
      },
    )

    it("tail element has the required typography and truncate classes", () => {
      useGoalsStore.setState({ tailByRunId: { r1: toolUseRead } })
      render(
        <RunListItem run={makeRun({ outcome: "running" })} onClick={() => {}} />,
      )
      const tail = screen.getByTestId("run-row-tail")
      const className = tail.className
      for (const cls of [
        "font-mono",
        "text-meta",
        "text-fg-secondary",
        "tabular-nums",
        "truncate",
      ]) {
        expect(className).toContain(cls)
      }
    })
  })

  // --- i18n time-ago rendering (Pattern 5 template-literal sweep follow-up) ---
  describe("relative timestamp i18n", () => {
    const fixedNow = new Date("2026-05-26T12:00:00Z")

    beforeEach(() => {
      vi.useFakeTimers()
      vi.setSystemTime(fixedNow)
    })

    afterEach(() => {
      vi.useRealTimers()
      mockedUseLocale.mockReturnValue("en")
    })

    const cases: Array<{
      locale: "en" | "zh"
      ago: string
      startedAt: string
      expected: string
    }> = [
      { locale: "en", ago: "5m", startedAt: "2026-05-26T11:55:00Z", expected: "5m ago" },
      { locale: "en", ago: "3h", startedAt: "2026-05-26T09:00:00Z", expected: "3h ago" },
      { locale: "en", ago: "2d", startedAt: "2026-05-24T12:00:00Z", expected: "2d ago" },
      { locale: "zh", ago: "5m", startedAt: "2026-05-26T11:55:00Z", expected: "5 分鐘前" },
      { locale: "zh", ago: "3h", startedAt: "2026-05-26T09:00:00Z", expected: "3 小時前" },
      { locale: "zh", ago: "2d", startedAt: "2026-05-24T12:00:00Z", expected: "2 天前" },
    ]

    for (const { locale, ago, startedAt, expected } of cases) {
      it(`RunListItem_time_ago_${ago}_in_${locale}_locale`, () => {
        mockedUseLocale.mockReturnValue(locale)
        render(
          <RunListItem
            run={makeRun({ started_at: startedAt })}
            onClick={() => {}}
          />,
        )
        expect(screen.getByTestId("run-row-r1")).toHaveTextContent(expected)
      })
    }
  })
})
