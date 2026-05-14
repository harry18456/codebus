import { describe, expect, it, vi, beforeEach } from "vitest"
import { render } from "@testing-library/react"

const toggleExpanded = vi.fn()

vi.mock("@/store/chat", () => ({
  useChatStore: {
    getState: () => ({ toggleExpanded }),
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
    toggleExpanded.mockReset()
  })

  it("toggles chat widget on Ctrl+K when mounted (Workspace)", () => {
    render(<Harness />)
    fireKey({ key: "k", ctrlKey: true })
    expect(toggleExpanded).toHaveBeenCalledTimes(1)
  })

  it("toggles chat widget on Cmd+K when mounted (Mac, Workspace)", () => {
    render(<Harness />)
    fireKey({ key: "k", metaKey: true })
    expect(toggleExpanded).toHaveBeenCalledTimes(1)
  })

  it("toggles twice when Ctrl+K pressed twice (expand then collapse)", () => {
    render(<Harness />)
    fireKey({ key: "k", ctrlKey: true })
    fireKey({ key: "k", ctrlKey: true })
    expect(toggleExpanded).toHaveBeenCalledTimes(2)
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
    expect(toggleExpanded).not.toHaveBeenCalled()
  })

  it("does not fire on plain k without modifier", () => {
    render(<Harness />)
    fireKey({ key: "k" })
    expect(toggleExpanded).not.toHaveBeenCalled()
  })

  it("does not fire on Ctrl+other key", () => {
    render(<Harness />)
    fireKey({ key: "j", ctrlKey: true })
    expect(toggleExpanded).not.toHaveBeenCalled()
  })

  it("removes listener on unmount", () => {
    const { unmount } = render(<Harness />)
    unmount()
    fireKey({ key: "k", ctrlKey: true })
    expect(toggleExpanded).not.toHaveBeenCalled()
  })
})
