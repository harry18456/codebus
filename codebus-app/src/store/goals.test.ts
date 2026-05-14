import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { useGoalsStore } from "./goals"

const invokeMock = vi.mocked(invoke)
const listenMock = vi.mocked(listen)

describe("useGoalsStore", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useGoalsStore.setState({ runs: [], activeRun: null })
  })

  afterEach(() => {
    listenMock.mockClear()
  })

  it("useGoalsStore_subscribes_goal_stream_channel", () => {
    // Module import triggered store factory, which calls
    // `listen("goal-stream", ...)` exactly once.
    expect(listenMock).toHaveBeenCalled()
    const [channel] = listenMock.mock.calls[0] as [string, unknown]
    expect(channel).toBe("goal-stream")
  })

  it("useGoalsStore_spawnGoal_optimistic_active_run", async () => {
    invokeMock.mockResolvedValueOnce("2026-05-13T14-56-21Z")

    await useGoalsStore.getState().spawnGoal("/some/vault", "test goal")

    const state = useGoalsStore.getState()
    expect(state.activeRun).not.toBeNull()
    expect(state.activeRun?.runId).toBe("2026-05-13T14-56-21Z")
    expect(state.activeRun?.goal).toBe("test goal")
    expect(state.runs).toHaveLength(1)
    expect(state.runs[0].outcome).toBe("running")
    expect(state.runs[0].run_id).toBe("2026-05-13T14-56-21Z")
  })

  it("cancelGoal flips local cancelling flag before backend resolves", async () => {
    invokeMock.mockResolvedValueOnce("run-x")
    await useGoalsStore.getState().spawnGoal("/some/vault", "g")
    invokeMock.mockResolvedValueOnce(undefined)

    const promise = useGoalsStore.getState().cancelGoal("run-x")
    // Synchronously after the call, cancelling flag should already be set.
    expect(useGoalsStore.getState().activeRun?.cancelling).toBe(true)
    await promise
  })

  it("stream events append to activeRun.events buffer matched by run_id", () => {
    useGoalsStore.setState({
      activeRun: {
        runId: "r1",
        goal: "g",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
    })
    useGoalsStore.getState()._onStreamEvent({
      run_id: "r1",
      event: { kind: "stream", data: { kind: "thought", text: "hi" } },
    })
    useGoalsStore.getState()._onStreamEvent({
      run_id: "r1",
      event: {
        kind: "stream",
        data: {
          kind: "tool_use",
          name: "Read",
          input: { file_path: "a.rs" },
        },
      },
    })
    expect(useGoalsStore.getState().activeRun?.events).toHaveLength(2)

    // Event for a different run id is dropped.
    useGoalsStore.getState()._onStreamEvent({
      run_id: "r-other",
      event: { kind: "stream", data: { kind: "thought", text: "nope" } },
    })
    expect(useGoalsStore.getState().activeRun?.events).toHaveLength(2)
  })

  it("refreshRuns populates the runs list from list_runs IPC", async () => {
    invokeMock.mockResolvedValueOnce([
      {
        run_id: "r1",
        mode: "goal",
        goal: "g1",
        started_at: "2026-05-13T10:00:00Z",
        finished_at: "2026-05-13T10:01:00Z",
        tokens: { input_tokens: 0, output_tokens: 0 },
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: "succeeded",
      },
    ])
    await useGoalsStore.getState().refreshRuns("/v")
    expect(useGoalsStore.getState().runs).toHaveLength(1)
    expect(useGoalsStore.getState().runs[0].mode).toBe("goal")
  })
})
