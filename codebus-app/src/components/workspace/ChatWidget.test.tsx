import { fireEvent, render, screen, act } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

// Mock Tauri event/core so the chat store's import-time `listen(...)` calls
// don't blow up under JSDOM. Mirrors store/chat.test.ts pattern.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { useChatStore } from "@/store/chat"

import { ChatWidget } from "./ChatWidget"

const INITIAL_STATE = useChatStore.getState()

function resetStore(): void {
  useChatStore.setState({
    sessionId: INITIAL_STATE.sessionId,
    turns: INITIAL_STATE.turns,
    activeTurn: INITIAL_STATE.activeTurn,
    tokensTotal: INITIAL_STATE.tokensTotal,
    promoteSuggestion: INITIAL_STATE.promoteSuggestion,
    expanded: INITIAL_STATE.expanded,
    width: INITIAL_STATE.width,
    height: INITIAL_STATE.height,
    onboardedVaults: new Set(),
    lastTranscript: INITIAL_STATE.lastTranscript,
    lastSessionId: INITIAL_STATE.lastSessionId,
  })
}

/**
 * JSDOM defaults to 1024x768. With 1rem = 16px:
 *   - 50% viewport width = 32rem
 *   - 80% viewport height = 38.4rem
 * Tests that need to assert clamp-to-viewport behavior override these
 * via Object.defineProperty so the auto-clamp math is deterministic.
 */
function setViewport(widthPx: number, heightPx: number): void {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    writable: true,
    value: widthPx,
  })
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    writable: true,
    value: heightPx,
  })
}

describe("ChatWidget", () => {
  beforeEach(() => {
    resetStore()
    // 80rem x 60rem viewport — large enough that the default 22x32 and the
    // resize-test target 40x60 both fit without auto-clamp interference.
    setViewport(1280, 960)
  })

  afterEach(() => {
    resetStore()
  })

  it("renders as a bottom-right bubble when collapsed", () => {
    render(<ChatWidget />)
    const widget = screen.getByTestId("chat-widget")
    expect(widget.getAttribute("data-state")).toBe("collapsed")
    // 3rem × 3rem (`h-12 w-12`), pinned bottom-right. Right edge sits 16px
    // from the viewport; bottom edge is offset by the 32px `BottomStrip`
    // plus the 16px gap so the bubble never overlaps the version label.
    expect(widget.tagName.toLowerCase()).toBe("button")
    expect(widget.className).toMatch(/\bh-12\b/)
    expect(widget.className).toMatch(/\bw-12\b/)
    expect(widget.className).toMatch(/\bfixed\b/)
    const style = widget.getAttribute("style") ?? ""
    expect(style).toMatch(/bottom:\s*48px/)
    expect(style).toMatch(/right:\s*16px/)
    const aria = widget.getAttribute("aria-label") ?? ""
    expect(aria.toLowerCase()).toContain("open chat")
  })

  it("expands to default 22rem × 32rem panel when bubble clicked", () => {
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget"))
    const widget = screen.getByTestId("chat-widget")
    expect(widget.getAttribute("data-state")).toBe("expanded")
    const style = widget.getAttribute("style") ?? ""
    expect(style).toMatch(/width:\s*22rem/)
    expect(style).toMatch(/height:\s*32rem/)
    // Bottom offset accounts for the 32px BottomStrip + 16px gap.
    expect(style).toMatch(/bottom:\s*48px/)
    expect(style).toMatch(/right:\s*16px/)
  })

  it("clamps resize handle drag to width ≤ 40rem and height ≤ 60rem", () => {
    // Big viewport so the bounds we're checking are the hard rem caps, not
    // the 50%×80% viewport caps.
    setViewport(2000, 2000)
    useChatStore.setState({ expanded: true, width: 22, height: 32 })
    render(<ChatWidget />)
    const handle = screen.getByTestId("chat-widget-resize-handle")
    // Top-left handle: pointer moves up-left by (delta * 16px) per rem.
    // Start at an arbitrary anchor (500, 500); end at a position that would
    // yield width = startWidth + 28rem = 50rem, height = startHeight + 38rem
    // = 70rem if unclamped.
    fireEvent.pointerDown(handle, { clientX: 500, clientY: 500, pointerId: 1 })
    fireEvent.pointerMove(handle, {
      clientX: 500 - 28 * 16,
      clientY: 500 - 38 * 16,
      pointerId: 1,
    })
    fireEvent.pointerUp(handle, {
      clientX: 500 - 28 * 16,
      clientY: 500 - 38 * 16,
      pointerId: 1,
    })
    expect(useChatStore.getState().width).toBe(40)
    expect(useChatStore.getState().height).toBe(60)
  })

  it("auto-clamps width when viewport shrinks below current widget width", () => {
    // Start expanded at 30rem × 40rem in a generous viewport.
    setViewport(1280, 960)
    useChatStore.setState({ expanded: true, width: 30, height: 40 })
    render(<ChatWidget />)
    // Viewport shrinks so 50% width = 25rem (400px) → width must clamp.
    // Height clamp is independent; leave it under the 80% cap.
    act(() => {
      setViewport(800, 960)
      window.dispatchEvent(new Event("resize"))
    })
    expect(useChatStore.getState().width).toBe(25)
  })

  it("minimize button in the header collapses the widget", () => {
    useChatStore.setState({ expanded: true })
    render(<ChatWidget />)
    const minimize = screen.getByTestId("chat-widget-minimize")
    expect(minimize).toBeInTheDocument()
    fireEvent.click(minimize)
    expect(useChatStore.getState().expanded).toBe(false)
  })

  it("resize handle renders a visible affordance (svg) plus nwse-resize cursor", () => {
    useChatStore.setState({ expanded: true })
    render(<ChatWidget />)
    const handle = screen.getByTestId("chat-widget-resize-handle")
    // Visible affordance — an SVG child the user can see without hovering.
    expect(handle.querySelector("svg")).not.toBeNull()
    // Cursor hint preserved.
    expect(handle.className).toMatch(/cursor-nwse-resize/)
  })

  it("renders a red-dot badge on the collapsed bubble when a promote suggestion is pending", () => {
    useChatStore.setState({
      expanded: false,
      promoteSuggestion: { reason: "auth + JWT looks like a wiki topic", turnIndex: 0 },
    })
    const { rerender } = render(<ChatWidget />)
    expect(screen.getByTestId("chat-widget-promote-badge")).toBeInTheDocument()
    // Expanding clears the badge (per spec: "badge SHALL disappear the next
    // time the widget expands"). We still allow the suggestion itself to
    // persist in the expanded panel — only the bubble badge goes away,
    // which is automatic since the bubble is unmounted on expand.
    useChatStore.setState({ expanded: true })
    rerender(<ChatWidget />)
    expect(screen.queryByTestId("chat-widget-promote-badge")).not.toBeInTheDocument()
  })
})
