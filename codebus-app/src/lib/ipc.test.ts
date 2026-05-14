import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import {
  cancelChatTurn,
  cancelGoal,
  getRunDetail,
  listRuns,
  listWikiPages,
  readWikiPage,
  spawnChatTurn,
  spawnGoal,
  type ChatStreamPayload,
  type ChatTurnRunId,
  type ModeFilter,
  type RunDetail,
  type RunLogSummary,
  type VerbEvent,
  type WikiPageMeta,
} from "./ipc"

/**
 * Spec: app-workspace § Tauri IPC Commands. Each typed wrapper SHALL
 * return the Promise<T> shape matching the Rust IPC return type. The
 * assertions are statically enforced — if the wrapper's declared
 * return type drifts, this file fails to typecheck even before the
 * test runner reaches the body.
 */
describe("workspace IPC wrappers", () => {
  it("ipc_wrappers_have_correct_return_types", () => {
    const goal: ReturnType<typeof spawnGoal> = spawnGoal("/v", "g")
    const cancel: ReturnType<typeof cancelGoal> = cancelGoal("r")
    const list: ReturnType<typeof listRuns> = listRuns("/v", {
      kind: "goal",
    } satisfies ModeFilter)
    const detail: ReturnType<typeof getRunDetail> = getRunDetail("/v", "r")
    const pages: ReturnType<typeof listWikiPages> = listWikiPages("/v")
    const body: ReturnType<typeof readWikiPage> = readWikiPage("/v", "s")

    // Assignment-style type narrowing: the explicit annotations on
    // the left-hand side fail TypeScript compilation if the wrapper's
    // return type drifts from Promise<string> / Promise<void> / etc.
    const _goalRet: Promise<string> = goal
    const _cancelRet: Promise<void> = cancel
    const _listRet: Promise<RunLogSummary[]> = list
    const _detailRet: Promise<RunDetail> = detail
    const _pagesRet: Promise<WikiPageMeta[]> = pages
    const _bodyRet: Promise<string> = body

    void _goalRet
    void _cancelRet
    void _listRet
    void _detailRet
    void _pagesRet
    void _bodyRet

    expect(true).toBe(true)
  })

  // Spec: app-workspace § Tauri IPC Commands for Chat Turn Lifecycle.
  it("chat_ipc_wrappers_have_correct_return_types", () => {
    const turn: ReturnType<typeof spawnChatTurn> = spawnChatTurn("/v", "hi", null)
    const turnWithResume: ReturnType<typeof spawnChatTurn> = spawnChatTurn(
      "/v",
      "hi",
      "sess-123",
    )
    const cancel: ReturnType<typeof cancelChatTurn> = cancelChatTurn("chat-2026-05-14T10-20-30Z")

    // Assignment-style: drift in spawnChatTurn return type breaks compile.
    const _turnRet: Promise<ChatTurnRunId> = turn
    const _turnResumeRet: Promise<ChatTurnRunId> = turnWithResume
    const _cancelRet: Promise<void> = cancel

    void _turnRet
    void _turnResumeRet
    void _cancelRet

    expect(true).toBe(true)
  })

  it("chat_stream_payload_shape", () => {
    // Construct a synthetic payload so the structural type check covers
    // the on-wire contract: `{ run_id: string starting "chat-", event: VerbEvent }`.
    const event: VerbEvent = {
      kind: "lifecycle",
      data: { kind: "spawn_start", verb: "chat" },
    }
    const payload: ChatStreamPayload = {
      run_id: "chat-2026-05-14T10-20-30Z",
      event,
    }
    expect(payload.run_id.startsWith("chat-")).toBe(true)
    expect(payload.event.kind).toBe("lifecycle")
  })
})
