import { fireEvent, render, screen } from "@testing-library/react"
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

import { ChatNewChatButton } from "./ChatNewChatButton"

const INITIAL_STATE = useChatStore.getState()

function resetStore(): void {
  useChatStore.setState({
    sessionId: INITIAL_STATE.sessionId,
    turns: INITIAL_STATE.turns,
    activeTurn: INITIAL_STATE.activeTurn,
    tokensTotal: { input_tokens: 0, output_tokens: 0 },
    promoteSuggestion: INITIAL_STATE.promoteSuggestion,
    expanded: INITIAL_STATE.expanded,
    width: INITIAL_STATE.width,
    height: INITIAL_STATE.height,
    onboardedVaults: new Set(),
    lastTranscript: null,
    lastSessionId: null,
  })
}

describe("ChatNewChatButton", () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    resetStore()
  })

  it("renders a '+ New chat' button with stable testid", () => {
    render(<ChatNewChatButton />)
    const btn = screen.getByTestId("chat-new-chat-button")
    expect(btn).toBeInTheDocument()
    expect(btn.tagName.toLowerCase()).toBe("button")
    // Hard-coded copy per task spec (i18n lands in task 7.2).
    expect(btn.textContent).toContain("+ New chat")
  })

  it("triggers useChatStore.newSession when clicked, stashing current session into undo buffer", () => {
    // Seed an in-progress session so newSession has something to snapshot.
    useChatStore.setState({
      sessionId: "sess-abc",
      turns: [
        {
          userText: "hello",
          events: [],
          startedAt: "2026-01-01T00:00:00.000Z",
          finishedAt: "2026-01-01T00:00:01.000Z",
        },
        {
          userText: "follow-up",
          events: [],
          startedAt: "2026-01-01T00:00:02.000Z",
          finishedAt: "2026-01-01T00:00:03.000Z",
        },
      ],
    })
    render(<ChatNewChatButton />)
    fireEvent.click(screen.getByTestId("chat-new-chat-button"))
    const state = useChatStore.getState()
    // Live session cleared.
    expect(state.sessionId).toBeNull()
    expect(state.turns).toEqual([])
    // Undo buffer populated from the pre-click session.
    expect(state.lastSessionId).toBe("sess-abc")
    expect(state.lastTranscript).toHaveLength(2)
  })
})
