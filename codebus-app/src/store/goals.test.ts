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
    useGoalsStore.setState({ runs: [], activeRun: null, tailByRunId: {} })
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

  it("tailByRunId · stream event for active run writes tail and appends to activeRun.events", () => {
    useGoalsStore.setState({
      activeRun: {
        runId: "run-A",
        goal: "g",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
      tailByRunId: {},
    })
    const toolUseEvent = {
      kind: "stream" as const,
      data: {
        kind: "tool_use" as const,
        name: "Read",
        input: { file_path: "x" },
      },
    }
    useGoalsStore.getState()._onStreamEvent({
      run_id: "run-A",
      event: toolUseEvent,
    })
    expect(useGoalsStore.getState().tailByRunId["run-A"]).toEqual(toolUseEvent)
    expect(useGoalsStore.getState().activeRun?.events).toHaveLength(1)
  })

  it("tailByRunId · stream event for terminal-spawned goal (activeRun null) still writes tail", () => {
    useGoalsStore.setState({
      activeRun: null,
      runs: [],
      tailByRunId: {},
    })
    const bannerEvent = {
      kind: "banner" as const,
      data: { kind: "start" as const, repo_path: "/v" },
    }
    useGoalsStore.getState()._onStreamEvent({
      run_id: "run-B",
      event: bannerEvent,
    })
    expect(useGoalsStore.getState().tailByRunId["run-B"]).toEqual(bannerEvent)
    expect(useGoalsStore.getState().activeRun).toBeNull()
  })

  it("tailByRunId · thought event does not write tail (prior value preserved)", () => {
    const prior = {
      kind: "stream" as const,
      data: {
        kind: "tool_use" as const,
        name: "Read",
        input: { file_path: "x" },
      },
    }
    useGoalsStore.setState({
      activeRun: {
        runId: "run-A",
        goal: "g",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
      tailByRunId: { "run-A": prior },
    })
    useGoalsStore.getState()._onStreamEvent({
      run_id: "run-A",
      event: { kind: "stream", data: { kind: "thought", text: "..." } },
    })
    expect(useGoalsStore.getState().tailByRunId["run-A"]).toEqual(prior)
  })

  it("tailByRunId · _onTerminal preserves tail slot after clearing activeRun", () => {
    const tail = {
      kind: "stream" as const,
      data: {
        kind: "tool_use" as const,
        name: "Read",
        input: { file_path: "x" },
      },
    }
    useGoalsStore.setState({
      activeRun: {
        runId: "run-A",
        goal: "g",
        startedAt: "2026-05-13T00:00:00Z",
        events: [],
        cancelling: false,
      },
      runs: [],
      tailByRunId: { "run-A": tail },
      _currentVaultPath: null,
    })
    invokeMock.mockResolvedValue([])
    useGoalsStore.getState()._onTerminal({
      run_id: "run-A",
    })
    expect(useGoalsStore.getState().activeRun).toBeNull()
    expect(useGoalsStore.getState().tailByRunId["run-A"]).toEqual(tail)
  })

  it("tailByRunId · reset() clears tail map alongside runs and activeRun", () => {
    const tail = {
      kind: "banner" as const,
      data: { kind: "sync_start" as const },
    }
    useGoalsStore.setState({
      activeRun: null,
      runs: [
        {
          run_id: "run-A",
          mode: "goal",
          goal: "g",
          started_at: "",
          finished_at: "",
          tokens: { input_tokens: 0, output_tokens: 0 },
          wiki_changed: false,
          lint_error_count: 0,
          lint_warn_count: 0,
          outcome: "succeeded",
        },
      ],
      tailByRunId: { "run-A": tail, "run-B": tail },
    })
    useGoalsStore.getState().reset()
    const s = useGoalsStore.getState()
    expect(s.tailByRunId).toEqual({})
    expect(s.activeRun).toBeNull()
    expect(s.runs).toEqual([])
  })

  it("tailByRunId · vault A→reset→vault B boundary: tail does not bleed across vault switch", () => {
    const vaultATail = {
      kind: "stream" as const,
      data: {
        kind: "tool_use" as const,
        name: "Read",
        input: { file_path: "vaultA/x.rs" },
      },
    }
    useGoalsStore.setState({
      activeRun: null,
      runs: [],
      tailByRunId: { "run-A1": vaultATail, "run-A2": vaultATail },
    })
    // Workspace unmount when switching vaults calls reset.
    useGoalsStore.getState().reset()
    // Now in vault B — fire a fresh stream event.
    const vaultBEvent = {
      kind: "stream" as const,
      data: {
        kind: "tool_use" as const,
        name: "Write",
        input: { file_path: "vaultB/y.md" },
      },
    }
    useGoalsStore.getState()._onStreamEvent({
      run_id: "run-B1",
      event: vaultBEvent,
    })
    const s = useGoalsStore.getState()
    expect(s.tailByRunId["run-A1"]).toBeUndefined()
    expect(s.tailByRunId["run-A2"]).toBeUndefined()
    expect(s.tailByRunId["run-B1"]).toEqual(vaultBEvent)
  })

  it("refreshRuns keeps an in-flight activeRun shown as running over a disk-derived interrupted row", async () => {
    // An in-progress goal: activeRun set, but its terminal RunLog row is
    // not yet on disk, so list_runs synthesizes a virtual `interrupted`
    // row for the same run_id. refreshRuns MUST NOT let that overwrite the
    // optimistic running state.
    useGoalsStore.setState({
      activeRun: {
        runId: "2026-05-13T14-56-21Z",
        goal: "test goal",
        startedAt: "2026-05-13T14:56:21Z",
        events: [],
        cancelling: false,
      },
      runs: [],
    })
    invokeMock.mockResolvedValueOnce([
      {
        run_id: "2026-05-13T14-56-21Z",
        mode: "goal",
        goal: "",
        started_at: "2026-05-13T14:56:21Z",
        finished_at: "",
        tokens: { input_tokens: 0, output_tokens: 0 },
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: "interrupted",
      },
    ])

    await useGoalsStore.getState().refreshRuns("/v")

    const row = useGoalsStore
      .getState()
      .runs.find((r) => r.run_id === "2026-05-13T14-56-21Z")
    expect(row).toBeDefined()
    expect(row?.outcome).toBe("running")
    expect(row?.goal).toBe("test goal")
    expect(row?.started_at).toBe("2026-05-13T14:56:21Z")
  })

  // vault-switch-goal-regression Decision 8
  it("refreshRuns restores activeRun from get_run_detail when backend reports running and frontend forgot", async () => {
    // Simulate the user-reported flow: previously spawned a goal in this
    // vault; navigated back to Lobby (Workspace unmount → reset cleared
    // activeRun); now opened the vault again — refreshRuns sees backend
    // still reports outcome="running" (per Decision 6) AND activeRun is
    // null, so it must re-hydrate activeRun from get_run_detail so the
    // RunDetail view can render past events instead of going blank.
    useGoalsStore.setState({ activeRun: null, runs: [] })
    const runningRunId = "2026-05-28T07-39-26Z"
    invokeMock
      // first call: list_runs returns the in-flight running row
      .mockResolvedValueOnce([
        {
          run_id: runningRunId,
          mode: "goal",
          goal: "in-flight goal text",
          started_at: "2026-05-28T07:39:26Z",
          finished_at: "",
          tokens: { input_tokens: 0, output_tokens: 0 },
          wiki_changed: false,
          lint_error_count: 0,
          lint_warn_count: 0,
          outcome: "running",
        },
      ])
      // second call: get_run_detail returns the past events from disk
      .mockResolvedValueOnce({
        summary: {
          run_id: runningRunId,
          mode: "goal",
          goal: "in-flight goal text",
          started_at: "2026-05-28T07:39:26Z",
          finished_at: "",
          tokens: { input_tokens: 0, output_tokens: 0 },
          wiki_changed: false,
          lint_error_count: 0,
          lint_warn_count: 0,
          outcome: "running",
        },
        events: [
          {
            ts: "2026-05-28T07:39:26.100Z",
            event: {
              kind: "banner",
              data: { kind: "start", repo_path: "/v" },
            },
          },
          {
            ts: "2026-05-28T07:39:26.200Z",
            event: {
              kind: "banner",
              data: { kind: "goal", goal_text: "in-flight goal text" },
            },
          },
        ],
      })

    await useGoalsStore.getState().refreshRuns("/v")

    const state = useGoalsStore.getState()
    expect(state.activeRun).not.toBeNull()
    expect(state.activeRun?.runId).toBe(runningRunId)
    expect(state.activeRun?.goal).toBe("in-flight goal text")
    expect(state.activeRun?.events).toHaveLength(2)
    expect(state.activeRun?.cancelling).toBe(false)
    expect(state.runs).toHaveLength(1)
  })

  it("refreshRuns does NOT restore activeRun when backend has no running row", async () => {
    useGoalsStore.setState({ activeRun: null, runs: [] })
    invokeMock.mockResolvedValueOnce([
      {
        run_id: "old",
        mode: "goal",
        goal: "done",
        started_at: "2026-05-28T07:00:00Z",
        finished_at: "2026-05-28T07:00:30Z",
        tokens: { input_tokens: 0, output_tokens: 0 },
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: "succeeded",
      },
    ])

    await useGoalsStore.getState().refreshRuns("/v")

    expect(useGoalsStore.getState().activeRun).toBeNull()
    expect(useGoalsStore.getState().runs).toHaveLength(1)
    // get_run_detail SHALL NOT have been called when no running row.
    expect(invokeMock).toHaveBeenCalledTimes(1)
  })

  it("refreshRuns falls back gracefully when get_run_detail rejects", async () => {
    useGoalsStore.setState({ activeRun: null, runs: [] })
    const runningRunId = "2026-05-28T07-39-26Z"
    invokeMock
      .mockResolvedValueOnce([
        {
          run_id: runningRunId,
          mode: "goal",
          goal: "in-flight",
          started_at: "2026-05-28T07:39:26Z",
          finished_at: "",
          tokens: { input_tokens: 0, output_tokens: 0 },
          wiki_changed: false,
          lint_error_count: 0,
          lint_warn_count: 0,
          outcome: "running",
        },
      ])
      .mockRejectedValueOnce(new Error("events file gone"))

    await useGoalsStore.getState().refreshRuns("/v")

    // activeRun stays null, runs list still published.
    expect(useGoalsStore.getState().activeRun).toBeNull()
    expect(useGoalsStore.getState().runs).toHaveLength(1)
  })
})
