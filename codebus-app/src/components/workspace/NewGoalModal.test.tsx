import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

import { invoke } from "@tauri-apps/api/core"
import { NewGoalModal } from "./NewGoalModal"
import { useGoalsStore } from "@/store/goals"

const invokeMock = vi.mocked(invoke)

describe("NewGoalModal", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  it("NewGoalModal_disables_Run_when_text_empty", () => {
    render(
      <NewGoalModal
        open
        vaultPath="/v"
        onClose={() => {}}
      />,
    )
    expect(screen.getByTestId("new-goal-run")).toBeDisabled()
  })

  it("NewGoalModal_disables_Run_when_active_run_exists_with_hint", () => {
    useGoalsStore.setState({
      activeRun: {
        runId: "r1",
        goal: "x",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
    })
    render(<NewGoalModal open vaultPath="/v" initialText="ready" onClose={() => {}} />)
    expect(screen.getByTestId("new-goal-run")).toBeDisabled()
    expect(screen.getByTestId("new-goal-blocked-hint")).toHaveTextContent(
      "Wait for current run to finish or cancel it before starting a new one.",
    )
  })

  it("NewGoalModal_Run_invokes_spawn_goal_then_closes", async () => {
    invokeMock.mockResolvedValueOnce("2026-05-13T14-56-21Z")
    const onClose = vi.fn()
    render(
      <NewGoalModal
        open
        vaultPath="/v"
        initialText="describe X"
        onClose={onClose}
      />,
    )
    fireEvent.click(screen.getByTestId("new-goal-run"))
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("spawn_goal", {
        vaultPath: "/v",
        goalText: "describe X",
      }),
    )
    await waitFor(() => expect(onClose).toHaveBeenCalled())
  })

  it("NewGoalModal_esc_key_closes", () => {
    const onClose = vi.fn()
    render(
      <NewGoalModal open vaultPath="/v" initialText="x" onClose={onClose} />,
    )
    // Radix Dialog handles Esc internally and triggers onOpenChange(false).
    // Fire keydown at the document root so Radix listener picks it up.
    fireEvent.keyDown(document.body, { key: "Escape", code: "Escape" })
    expect(onClose).toHaveBeenCalled()
  })

  it("Cancel button closes the modal without invoking spawn", () => {
    const onClose = vi.fn()
    render(
      <NewGoalModal open vaultPath="/v" initialText="x" onClose={onClose} />,
    )
    fireEvent.click(screen.getByTestId("new-goal-cancel"))
    expect(onClose).toHaveBeenCalled()
    expect(invokeMock).not.toHaveBeenCalledWith("spawn_goal", expect.anything())
  })

  // vault-switch-goal-regression Decision 7
  it("NewGoalModal_shows_friendly_error_when_backend_rejects_already_active", async () => {
    invokeMock.mockRejectedValueOnce({
      kind: "invalid",
      field: "active_runs",
      message: "another goal run is already active",
    })
    const onClose = vi.fn()
    render(
      <NewGoalModal open vaultPath="/v" initialText="describe X" onClose={onClose} />,
    )
    fireEvent.click(screen.getByTestId("new-goal-run"))
    await waitFor(() => {
      expect(screen.getByTestId("new-goal-spawn-error")).toHaveTextContent(
        "Another goal is still running in the background. Cancel it or wait for it to finish before starting a new one.",
      )
    })
    expect(onClose).not.toHaveBeenCalled()
  })

  it("NewGoalModal_shows_generic_error_for_unknown_spawn_failure", async () => {
    invokeMock.mockRejectedValueOnce({
      kind: "internal",
      message: "tauri channel hung up",
    })
    const onClose = vi.fn()
    render(
      <NewGoalModal open vaultPath="/v" initialText="describe X" onClose={onClose} />,
    )
    fireEvent.click(screen.getByTestId("new-goal-run"))
    await waitFor(() => {
      expect(screen.getByTestId("new-goal-spawn-error")).toHaveTextContent(
        "Failed to start goal: tauri channel hung up",
      )
    })
    expect(onClose).not.toHaveBeenCalled()
  })
})
