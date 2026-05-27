import { beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"

import type { QuizAttemptMeta } from "@/lib/ipc"
import { useQuizHistoryStore } from "./quiz-history"

const invokeMock = vi.mocked(invoke)
const listenMock = vi.mocked(listen)

function meta(id: string, slug = "session-vs-token"): QuizAttemptMeta {
  return {
    slug,
    quiz_id: id,
    trigger: "topic",
    topic: "session vs token",
    target_page: null,
    events_log: null,
    path: `/v/.codebus/quiz/${slug}/${id}.md`,
  }
}

describe("useQuizHistoryStore", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useQuizHistoryStore.setState({
      vaultPath: null,
      attempts: [],
      loading: false,
    })
  })

  it("loadAttempts populates attempts and remembers vaultPath", async () => {
    invokeMock.mockResolvedValueOnce([meta("q1"), meta("q2")])
    await useQuizHistoryStore.getState().loadAttempts("/v")
    const state = useQuizHistoryStore.getState()
    expect(state.attempts).toHaveLength(2)
    expect(state.vaultPath).toBe("/v")
    expect(state.loading).toBe(false)
  })

  it("loadAttempts swallows IPC errors and leaves attempts empty", async () => {
    invokeMock.mockRejectedValueOnce(new Error("boom"))
    await useQuizHistoryStore.getState().loadAttempts("/v")
    const state = useQuizHistoryStore.getState()
    expect(state.attempts).toEqual([])
    expect(state.loading).toBe(false)
  })

  it("reset clears attempts and vaultPath", async () => {
    invokeMock.mockResolvedValueOnce([meta("q1")])
    await useQuizHistoryStore.getState().loadAttempts("/v")
    useQuizHistoryStore.getState().reset()
    const state = useQuizHistoryStore.getState()
    expect(state.attempts).toEqual([])
    expect(state.vaultPath).toBeNull()
  })

  it("quiz-changed event refreshes attempts when a vaultPath is known", async () => {
    // The factory subscribed via listen() at module import; grab the
    // handler it registered so we can fire it directly.
    const call = listenMock.mock.calls.find(([channel]) => channel === "quiz-changed")
    expect(call).toBeDefined()
    const handler = call![1] as (event: { payload: unknown }) => void

    invokeMock.mockResolvedValueOnce([meta("q1")])
    await useQuizHistoryStore.getState().loadAttempts("/v")
    expect(useQuizHistoryStore.getState().attempts).toHaveLength(1)

    invokeMock.mockResolvedValueOnce([meta("q1"), meta("q2"), meta("q3")])
    handler({ payload: null })
    // Flush the void-prefixed loadAttempts promise chain.
    await vi.waitFor(() =>
      expect(useQuizHistoryStore.getState().attempts).toHaveLength(3),
    )
  })

  it("quiz-changed event with no known vaultPath does not call list_quiz_attempts", async () => {
    const call = listenMock.mock.calls.find(([channel]) => channel === "quiz-changed")
    expect(call).toBeDefined()
    const handler = call![1] as (event: { payload: unknown }) => void

    // beforeEach reset state.vaultPath to null → the handler must
    // see it and skip the IPC call.
    handler({ payload: null })
    await Promise.resolve()
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it("store subscribes to the quiz-changed channel at module init", () => {
    const channels = listenMock.mock.calls.map(([channel]) => channel)
    expect(channels).toContain("quiz-changed")
  })
})
