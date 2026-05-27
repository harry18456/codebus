import { describe, expect, it, vi } from "vitest"
import { renderHook } from "@testing-library/react"

import { useNewGoalShortcut } from "./useNewGoalShortcut"

describe("useNewGoalShortcut", () => {
  it("fires onFire when bare N is pressed and nothing is focused", () => {
    const onFire = vi.fn()
    renderHook(() => useNewGoalShortcut(onFire))
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "n" }))
    expect(onFire).toHaveBeenCalledTimes(1)
  })

  it("fires for uppercase N as well (case-insensitive)", () => {
    const onFire = vi.fn()
    renderHook(() => useNewGoalShortcut(onFire))
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "N" }))
    expect(onFire).toHaveBeenCalledTimes(1)
  })

  it("does not fire when Cmd is held (that path belongs to useNewVaultShortcut)", () => {
    const onFire = vi.fn()
    renderHook(() => useNewGoalShortcut(onFire))
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "n", metaKey: true }),
    )
    expect(onFire).not.toHaveBeenCalled()
  })

  it("does not fire when Ctrl is held", () => {
    const onFire = vi.fn()
    renderHook(() => useNewGoalShortcut(onFire))
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "n", ctrlKey: true }),
    )
    expect(onFire).not.toHaveBeenCalled()
  })

  it("does not fire when focus is inside a textarea", () => {
    const onFire = vi.fn()
    const textarea = document.createElement("textarea")
    document.body.appendChild(textarea)
    try {
      renderHook(() => useNewGoalShortcut(onFire))
      textarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "n", bubbles: true }),
      )
      expect(onFire).not.toHaveBeenCalled()
    } finally {
      document.body.removeChild(textarea)
    }
  })

  it("does not fire when focus is inside an input", () => {
    const onFire = vi.fn()
    const input = document.createElement("input")
    document.body.appendChild(input)
    try {
      renderHook(() => useNewGoalShortcut(onFire))
      input.dispatchEvent(
        new KeyboardEvent("keydown", { key: "n", bubbles: true }),
      )
      expect(onFire).not.toHaveBeenCalled()
    } finally {
      document.body.removeChild(input)
    }
  })

  it("does not fire for unrelated keys", () => {
    const onFire = vi.fn()
    renderHook(() => useNewGoalShortcut(onFire))
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "g" }))
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }))
    expect(onFire).not.toHaveBeenCalled()
  })

  it("unbinds the listener on unmount", () => {
    const onFire = vi.fn()
    const { unmount } = renderHook(() => useNewGoalShortcut(onFire))
    unmount()
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "n" }))
    expect(onFire).not.toHaveBeenCalled()
  })
})
