import { act, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

// Mock Tauri event/core so the chat store's import-time `listen(...)` calls
// don't blow up under JSDOM. Mirrors ChatTokenDisplay.test.tsx pattern.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { useChatStore } from "@/store/chat"

import { ChatUndoToast } from "./ChatUndoToast"

const INITIAL_STATE = useChatStore.getState()

function resetStore(): void {
  useChatStore.setState({
    sessionId: INITIAL_STATE.sessionId,
    turns: INITIAL_STATE.turns,
    activeTurn: INITIAL_STATE.activeTurn,
    tokensTotal: { input_tokens: 0, output_tokens: 0 },
    promoteSuggestion: INITIAL_STATE.promoteSuggestion,
    mode: INITIAL_STATE.mode,
    modalReturnMode: INITIAL_STATE.modalReturnMode,
    onboardedVaults: new Set(),
    lastTranscript: null,
    lastSessionId: null,
  })
}

describe("ChatUndoToast", () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    resetStore()
    vi.useRealTimers()
  })

  it("does not render when the undo buffer is empty", () => {
    render(<ChatUndoToast />)
    expect(screen.queryByTestId("chat-undo-toast")).not.toBeInTheDocument()
  })

  it("renders 'Started new chat.' + an Undo button when in the undo window", () => {
    // Simulate the post-newSession state: snapshot fields populated.
    useChatStore.setState({
      lastSessionId: "sess-abc",
      lastTranscript: [
        {
          userText: "hi",
          events: [],
          startedAt: "2026-01-01T00:00:00.000Z",
          finishedAt: "2026-01-01T00:00:01.000Z",
        },
      ],
    })
    render(<ChatUndoToast />)
    const toast = screen.getByTestId("chat-undo-toast")
    expect(toast).toBeInTheDocument()
    expect(toast.textContent).toContain("New chat started")
    // Countdown badge surfaces remaining seconds for visual urgency.
    expect(screen.getByTestId("chat-undo-countdown").textContent).toMatch(
      /\(\d+s to undo\)/,
    )
    // The Undo affordance is a real <button> so keyboard users can hit it.
    const undoBtn = screen.getByRole("button", { name: /undo/i })
    expect(undoBtn).toBeInTheDocument()
  })

  it("invokes useChatStore.undoNewSession when the Undo button is clicked and restores the snapshot", () => {
    const restoredTurns = [
      {
        userText: "hello",
        events: [],
        startedAt: "2026-01-01T00:00:00.000Z",
        finishedAt: "2026-01-01T00:00:01.000Z",
      },
    ]
    useChatStore.setState({
      sessionId: null,
      turns: [],
      lastSessionId: "sess-xyz",
      lastTranscript: restoredTurns,
    })
    render(<ChatUndoToast />)
    fireEvent.click(screen.getByRole("button", { name: /undo/i }))
    const state = useChatStore.getState()
    // Snapshot moved back into the live session…
    expect(state.sessionId).toBe("sess-xyz")
    expect(state.turns).toEqual(restoredTurns)
    // …and the undo buffer is drained, so the toast disappears.
    expect(state.lastSessionId).toBeNull()
    expect(state.lastTranscript).toBeNull()
    expect(screen.queryByTestId("chat-undo-toast")).not.toBeInTheDocument()
  })

  it("auto-disappears from the DOM ~5s after newSession() (store gc clears the snapshot)", () => {
    vi.useFakeTimers()
    // Seed a live session, then call newSession() so the store schedules its
    // own 5s gc timer. We don't mock the timer — we let the store's real
    // setTimeout run under fake timers, then advance.
    useChatStore.setState({
      sessionId: "sess-abc",
      turns: [
        {
          userText: "hi",
          events: [],
          startedAt: "2026-01-01T00:00:00.000Z",
          finishedAt: "2026-01-01T00:00:01.000Z",
        },
      ],
      lastSessionId: null,
      lastTranscript: null,
    })
    act(() => {
      useChatStore.getState().newSession()
    })
    render(<ChatUndoToast />)
    // Immediately after newSession, toast is visible (snapshot non-null).
    expect(screen.getByTestId("chat-undo-toast")).toBeInTheDocument()
    // Advance past the 5s undo window — gc fires, clears lastTranscript /
    // lastSessionId, component re-renders to null.
    act(() => {
      vi.advanceTimersByTime(5000)
    })
    expect(screen.queryByTestId("chat-undo-toast")).not.toBeInTheDocument()
  })
})
