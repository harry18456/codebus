import { fireEvent, render, screen } from "@testing-library/react"
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
import { useGoalsStore } from "@/store/goals"
import { messages } from "@/i18n/messages"

import { ChatWidget } from "./ChatWidget"

const INITIAL_STATE = useChatStore.getState()

function resetStore(): void {
  useChatStore.setState({
    sessionId: INITIAL_STATE.sessionId,
    turns: INITIAL_STATE.turns,
    activeTurn: INITIAL_STATE.activeTurn,
    tokensTotal: INITIAL_STATE.tokensTotal,
    promoteSuggestion: INITIAL_STATE.promoteSuggestion,
    mode: INITIAL_STATE.mode,
    modalReturnMode: INITIAL_STATE.modalReturnMode,
    onboardedVaults: new Set(),
    lastTranscript: INITIAL_STATE.lastTranscript,
    lastSessionId: INITIAL_STATE.lastSessionId,
  })
  useGoalsStore.setState({ runs: [], activeRun: null })
}

function seedActiveRun(): void {
  useGoalsStore.setState({
    activeRun: {
      runId: "r-pulse",
      goal: "demo goal for pulse dot",
      startedAt: "2026-05-27T10:00:00Z",
      events: [],
      cancelling: false,
    },
  })
}

describe("ChatWidget — bubble mode", () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    resetStore()
  })

  it("renders as a 44px bottom-right bubble (data-state='bubble')", () => {
    render(<ChatWidget />)
    const widget = screen.getByTestId("chat-widget")
    expect(widget.getAttribute("data-state")).toBe("bubble")
    expect(widget.tagName.toLowerCase()).toBe("button")
    // 44×44 (`h-11 w-11`), pinned bottom-right (16px from each anchor,
    // bottom offset = BottomStrip (32px) + 16px gap = 48px).
    expect(widget.className).toMatch(/\bh-11\b/)
    expect(widget.className).toMatch(/\bw-11\b/)
    expect(widget.className).toMatch(/\bfixed\b/)
    const style = widget.getAttribute("style") ?? ""
    expect(style).toMatch(/bottom:\s*48px/)
    expect(style).toMatch(/right:\s*16px/)
    // Default aria-label points to the no-active-goal variant.
    expect(widget.getAttribute("aria-label")).toBe(
      messages.en["chat.widget.aria.openChat"],
    )
  })

  it("click transitions to floating mode (data-state='floating')", () => {
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget"))
    expect(useChatStore.getState().mode).toBe("floating")
    expect(screen.getByTestId("chat-widget").getAttribute("data-state")).toBe(
      "floating",
    )
  })

  it("renders pulse dot inside the bubble while a goal is running", () => {
    seedActiveRun()
    render(<ChatWidget />)
    const widget = screen.getByTestId("chat-widget")
    const dot = screen.getByTestId("chat-widget-active-goal-pulse")
    expect(widget).toContainElement(dot)
  })

  it("pulse dot stays mounted but hidden when no active goal exists", () => {
    render(<ChatWidget />)
    const dot = screen.queryByTestId("chat-widget-active-goal-pulse")
    // Either unmounted OR mounted-but-opacity-0; the contract from spec.
    if (dot !== null) {
      expect(dot.className).toMatch(/\bopacity-0\b/)
    }
  })

  it("renders red promote badge on bubble when a promote suggestion is pending", () => {
    useChatStore.setState({
      promoteSuggestion: {
        reason: "auth + JWT looks like a wiki topic",
        turnIndex: 0,
      },
    })
    render(<ChatWidget />)
    expect(screen.getByTestId("chat-widget-promote-badge")).toBeInTheDocument()
  })

  it("renders pulse dot and promote badge simultaneously on bubble", () => {
    seedActiveRun()
    useChatStore.setState({
      promoteSuggestion: { reason: "wiki topic candidate", turnIndex: 0 },
    })
    render(<ChatWidget />)
    expect(
      screen.getByTestId("chat-widget-active-goal-pulse"),
    ).toBeInTheDocument()
    expect(screen.getByTestId("chat-widget-promote-badge")).toBeInTheDocument()
  })

  it("active-goal aria-label switches when a goal starts running", () => {
    seedActiveRun()
    render(<ChatWidget />)
    const widget = screen.getByTestId("chat-widget")
    expect(widget.getAttribute("aria-label")).toBe(
      messages.en["chat.widget.aria.openChatWithActiveGoalRunning"],
    )
  })
})

describe("ChatWidget — floating mode", () => {
  beforeEach(() => {
    resetStore()
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
  })

  afterEach(() => {
    resetStore()
  })

  it("renders panel with data-state='floating' at fixed 360×460 size", () => {
    render(<ChatWidget />)
    const widget = screen.getByTestId("chat-widget")
    expect(widget.getAttribute("data-state")).toBe("floating")
    const style = widget.getAttribute("style") ?? ""
    expect(style).toMatch(/width:\s*360px/)
    expect(style).toMatch(/height:\s*460px/)
    expect(style).toMatch(/bottom:\s*48px/)
    expect(style).toMatch(/right:\s*16px/)
  })

  it("renders NO resize handle (Resize Affordance was removed)", () => {
    render(<ChatWidget />)
    expect(
      screen.queryByTestId("chat-widget-resize-handle"),
    ).not.toBeInTheDocument()
  })

  it("does not render pulse dot in floating mode", () => {
    seedActiveRun()
    render(<ChatWidget />)
    expect(
      screen.queryByTestId("chat-widget-active-goal-pulse"),
    ).not.toBeInTheDocument()
  })

  it("minimize button returns the widget to bubble mode", () => {
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget-minimize"))
    expect(useChatStore.getState().mode).toBe("bubble")
    expect(useChatStore.getState().modalReturnMode).toBeNull()
  })

  it("expand-to-modal button opens modal with modalReturnMode='floating'", () => {
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget-expand-to-modal"))
    expect(useChatStore.getState().mode).toBe("modal")
    expect(useChatStore.getState().modalReturnMode).toBe("floating")
  })
})

describe("ChatWidget — modal mode", () => {
  beforeEach(() => {
    resetStore()
    // Seed modal with bubble as the recorded return mode so the close-
    // to-return-mode behavior is exercised by Esc / backdrop click tests.
    useChatStore.setState({ mode: "modal", modalReturnMode: "bubble" })
  })

  afterEach(() => {
    resetStore()
  })

  it("renders centered modal via radix Dialog (data-state='modal' + role='dialog')", () => {
    render(<ChatWidget />)
    // The Dialog content IS the widget root — same element carries
    // role="dialog", aria-modal="true", data-testid="chat-widget", and
    // data-state="modal".
    const dialog = screen.getByRole("dialog")
    expect(dialog.getAttribute("aria-modal")).toBe("true")
    expect(dialog.getAttribute("data-testid")).toBe("chat-widget")
    expect(dialog.getAttribute("data-state")).toBe("modal")
  })

  it("does not render pulse dot in modal mode", () => {
    seedActiveRun()
    render(<ChatWidget />)
    expect(
      screen.queryByTestId("chat-widget-active-goal-pulse"),
    ).not.toBeInTheDocument()
  })

  it("dock button transitions modal → floating regardless of return mode", () => {
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget-dock-to-floating"))
    expect(useChatStore.getState().mode).toBe("floating")
    expect(useChatStore.getState().modalReturnMode).toBeNull()
  })

  it("close button transitions modal → bubble regardless of return mode", () => {
    // Seed with floating as the return mode to prove ✕ ignores it.
    useChatStore.setState({ mode: "modal", modalReturnMode: "floating" })
    render(<ChatWidget />)
    fireEvent.click(screen.getByTestId("chat-widget-modal-close"))
    expect(useChatStore.getState().mode).toBe("bubble")
    expect(useChatStore.getState().modalReturnMode).toBeNull()
  })
})
