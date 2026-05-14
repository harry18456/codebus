import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import type { RunDetail } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"
import {
  RunDetailCancelled,
  RunDetailInterrupted,
} from "./RunDetailCancelled"

const invokeMock = vi.mocked(invoke)

function makeDetail(over: Partial<RunDetail["summary"]> = {}): RunDetail {
  return {
    summary: {
      run_id: "r1",
      mode: "goal",
      goal: "describe auth flow",
      started_at: "2026-05-13T10:00:00Z",
      finished_at: "2026-05-13T10:01:00Z",
      tokens: { input_tokens: 0, output_tokens: 0 },
      wiki_changed: false,
      lint_error_count: 0,
      lint_warn_count: 0,
      outcome: "cancelled",
      ...over,
    },
    events: [],
  }
}

describe("RunDetailCancelled", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("RunDetailCancelled_warning_contains_uncommitted_changes_substring", () => {
    render(
      <RunDetailCancelled
        detail={makeDetail()}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(
      screen.getByTestId("cancelled-warning").textContent,
    ).toContain("Wiki has uncommitted changes — not auto-committed")
  })

  it("RunDetailCancelled_retry_button_prefills_modal_without_spawning", () => {
    render(
      <RunDetailCancelled
        detail={makeDetail({ goal: "describe auth flow" })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    fireEvent.click(screen.getByTestId("retry-button"))
    // Modal opens, textarea pre-filled with the original goal text.
    const textarea = screen.getByTestId(
      "new-goal-textarea",
    ) as HTMLTextAreaElement
    expect(textarea.value).toBe("describe auth flow")
    // spawn_goal IPC SHALL NOT be invoked by the retry click alone.
    expect(invokeMock).not.toHaveBeenCalledWith("spawn_goal", expect.anything())
  })
})

describe("RunDetailInterrupted", () => {
  it("RunDetailInterrupted_warning_contains_app_was_closed_substring", () => {
    render(
      <RunDetailInterrupted
        detail={makeDetail({ outcome: "interrupted" })}
        vaultPath="/v"
        onBack={() => {}}
      />,
    )
    expect(
      screen.getByTestId("interrupted-warning").textContent,
    ).toContain("App was closed before this goal finished")
  })
})
