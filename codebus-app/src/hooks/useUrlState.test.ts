import { beforeEach, describe, expect, it, vi } from "vitest"

import { renderHook } from "@testing-library/react"

import { useUrlState } from "./useUrlState"

function setSearch(qs: string) {
  // Replace the current entry to a known search-string without
  // pushing history; tests start each case in a controlled state.
  window.history.replaceState(null, "", `/?${qs}`.replace(/\?$/, "/"))
}

describe("useUrlState", () => {
  beforeEach(() => {
    window.history.replaceState(null, "", "/")
  })

  it("read returns nulls when query params are absent", () => {
    const { result } = renderHook(() => useUrlState())
    expect(result.current.read()).toEqual({
      quiz_step: null,
      staged_id: null,
    })
  })

  it("read returns the two params from window.location.search", () => {
    setSearch("quiz_step=generating&staged_id=abc123")
    const { result } = renderHook(() => useUrlState())
    expect(result.current.read()).toEqual({
      quiz_step: "generating",
      staged_id: "abc123",
    })
  })

  it("write pushes new history entry with both params set", () => {
    const pushSpy = vi.spyOn(window.history, "pushState")
    const { result } = renderHook(() => useUrlState())
    result.current.write({ quiz_step: "scope_confirm", staged_id: "stage-9" })
    expect(pushSpy).toHaveBeenCalledTimes(1)
    // After write, location.search reflects the new state.
    const params = new URLSearchParams(window.location.search)
    expect(params.get("quiz_step")).toBe("scope_confirm")
    expect(params.get("staged_id")).toBe("stage-9")
    pushSpy.mockRestore()
  })

  it("write preserves other query params owned by other systems", () => {
    setSearch("vault=foo&unrelated=bar")
    const { result } = renderHook(() => useUrlState())
    result.current.write({ quiz_step: "topic", staged_id: null })
    const params = new URLSearchParams(window.location.search)
    expect(params.get("vault")).toBe("foo")
    expect(params.get("unrelated")).toBe("bar")
    expect(params.get("quiz_step")).toBe("topic")
    expect(params.has("staged_id")).toBe(false)
  })

  it("write with both nulls removes the two params but keeps others", () => {
    setSearch("vault=foo&quiz_step=generating&staged_id=abc")
    const { result } = renderHook(() => useUrlState())
    result.current.write({ quiz_step: null, staged_id: null })
    const params = new URLSearchParams(window.location.search)
    expect(params.has("quiz_step")).toBe(false)
    expect(params.has("staged_id")).toBe(false)
    expect(params.get("vault")).toBe("foo")
  })

  it("mount and unmount do not push history entries", () => {
    const pushSpy = vi.spyOn(window.history, "pushState")
    const { unmount } = renderHook(() => useUrlState())
    expect(pushSpy).not.toHaveBeenCalled()
    unmount()
    expect(pushSpy).not.toHaveBeenCalled()
    pushSpy.mockRestore()
  })
})
