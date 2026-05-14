import { describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import {
  cancelGoal,
  getRunDetail,
  listRuns,
  listWikiPages,
  readWikiPage,
  spawnGoal,
  type ModeFilter,
  type RunDetail,
  type RunLogSummary,
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
})
