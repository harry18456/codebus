import { describe, expect, it, vi, beforeEach } from "vitest"
import { render } from "@testing-library/react"

// Mock the store with a mode-aware harness. ⌘K must call `openModal()`
// (per spec "Chat Widget Toggle Shortcut") and the action must snapshot
// the current mode into `modalReturnMode`.
const storeState = {
  mode: "bubble" as "bubble" | "floating" | "modal",
  modalReturnMode: null as "bubble" | "floating" | null,
}
const openModal = vi.fn(() => {
  // Mirror the real action: no-op when already in modal mode, otherwise
  // snapshot current mode as `modalReturnMode` and switch to modal.
  if (storeState.mode === "modal") return
  storeState.modalReturnMode = storeState.mode
  storeState.mode = "modal"
})

vi.mock("@/store/chat", () => ({
  useChatStore: {
    getState: () => ({ openModal, ...storeState }),
  },
}))

import { useChatShortcut } from "./useChatShortcut"

function Harness() {
  useChatShortcut()
  return null
}

function fireKey(init: KeyboardEventInit) {
  const event = new KeyboardEvent("keydown", { ...init, cancelable: true })
  window.dispatchEvent(event)
  return event
}

describe("useChatShortcut", () => {
  beforeEach(() => {
    openModal.mockClear()
    storeState.mode = "bubble"
    storeState.modalReturnMode = null
  })

  it("⌘K from bubble: opens modal with modalReturnMode = 'bubble'", () => {
    render(<Harness />)
    fireKey({ key: "k", metaKey: true })
    expect(openModal).toHaveBeenCalledTimes(1)
    expect(storeState.mode).toBe("modal")
    expect(storeState.modalReturnMode).toBe("bubble")
  })

  it("Ctrl+K from floating: opens modal with modalReturnMode = 'floating'", () => {
    storeState.mode = "floating"
    render(<Harness />)
    fireKey({ key: "k", ctrlKey: true })
    expect(openModal).toHaveBeenCalledTimes(1)
    expect(storeState.mode).toBe("modal")
    expect(storeState.modalReturnMode).toBe("floating")
  })

  it("⌘K while already in modal: action runs but is a no-op (does NOT re-snapshot)", () => {
    storeState.mode = "modal"
    storeState.modalReturnMode = "bubble"
    render(<Harness />)
    fireKey({ key: "k", metaKey: true })
    // The hook still invokes openModal; the action body decides to no-op.
    expect(openModal).toHaveBeenCalledTimes(1)
    expect(storeState.mode).toBe("modal")
    expect(storeState.modalReturnMode).toBe("bubble")
  })

  it("calls preventDefault on matching key combo", () => {
    render(<Harness />)
    const event = fireKey({ key: "k", ctrlKey: true })
    expect(event.defaultPrevented).toBe(true)
  })

  it("does not fire when hook is not mounted (Lobby case)", () => {
    // intentionally do NOT render Harness — simulates Lobby route where
    // useChatShortcut is never imported / mounted.
    fireKey({ key: "k", ctrlKey: true })
    fireKey({ key: "k", metaKey: true })
    expect(openModal).not.toHaveBeenCalled()
  })

  it("does not fire on plain k without modifier", () => {
    render(<Harness />)
    fireKey({ key: "k" })
    expect(openModal).not.toHaveBeenCalled()
  })

  it("does not fire on Ctrl+other key", () => {
    render(<Harness />)
    fireKey({ key: "j", ctrlKey: true })
    expect(openModal).not.toHaveBeenCalled()
  })

  it("removes listener on unmount", () => {
    const { unmount } = render(<Harness />)
    unmount()
    fireKey({ key: "k", ctrlKey: true })
    expect(openModal).not.toHaveBeenCalled()
  })
})
