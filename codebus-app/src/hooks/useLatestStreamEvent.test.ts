import { act, renderHook } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import type { VerbEvent } from "@/lib/ipc"
import { useGoalsStore } from "@/store/goals"

import { useLatestStreamEvent } from "./useLatestStreamEvent"

function toolUse(file_path: string): VerbEvent {
  return {
    kind: "stream",
    data: { kind: "tool_use", name: "Read", input: { file_path } },
  }
}

describe("useLatestStreamEvent", () => {
  beforeEach(() => {
    useGoalsStore.setState({ tailByRunId: {} })
  })

  afterEach(() => {
    useGoalsStore.setState({ tailByRunId: {} })
  })

  it("returns the tail value for a known run id", () => {
    const evt = toolUse("a.rs")
    useGoalsStore.setState({ tailByRunId: { "run-A": evt } })
    const { result } = renderHook(() => useLatestStreamEvent("run-A"))
    expect(result.current).toEqual(evt)
  })

  it("returns null when the run id is not present in the tail map", () => {
    const { result } = renderHook(() => useLatestStreamEvent("run-Z"))
    expect(result.current).toBeNull()
  })

  it("does not re-render when an unrelated run id's tail changes", () => {
    const evtA = toolUse("a.rs")
    useGoalsStore.setState({ tailByRunId: { "run-A": evtA } })

    let renderCount = 0
    const { result } = renderHook(() => {
      renderCount += 1
      return useLatestStreamEvent("run-A")
    })
    const initialRenders = renderCount
    expect(result.current).toEqual(evtA)

    // Write a tail for a DIFFERENT run id — run-A's slot is untouched.
    act(() => {
      useGoalsStore.setState((s) => ({
        tailByRunId: { ...s.tailByRunId, "run-B": toolUse("b.rs") },
      }))
    })

    expect(renderCount).toBe(initialRenders)
    expect(result.current).toEqual(evtA)
  })

  it("re-renders and returns the new value when the same run id's tail changes", () => {
    const evt1 = toolUse("a.rs")
    useGoalsStore.setState({ tailByRunId: { "run-A": evt1 } })
    const { result } = renderHook(() => useLatestStreamEvent("run-A"))
    expect(result.current).toEqual(evt1)

    const evt2 = toolUse("b.rs")
    act(() => {
      useGoalsStore.setState((s) => ({
        tailByRunId: { ...s.tailByRunId, "run-A": evt2 },
      }))
    })
    expect(result.current).toEqual(evt2)
  })
})
