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
vi.mock("@/hooks/useLocale", () => ({
  useLocale: vi.fn(() => "en"),
}))

import { useLocale } from "@/hooks/useLocale"
import { useChatStore } from "@/store/chat"

import { ChatNewChatButton } from "./ChatNewChatButton"

const mockedUseLocale = vi.mocked(useLocale)

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

describe("ChatNewChatButton", () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    resetStore()
  })

  it("renders the new-chat button with stable testid", () => {
    render(<ChatNewChatButton />)
    const btn = screen.getByTestId("chat-new-chat-button")
    expect(btn).toBeInTheDocument()
    expect(btn.tagName.toLowerCase()).toBe("button")
  })

  it.each([
    ["en", "+ New chat"],
    ["zh", "+ 新對話"],
  ])(
    "ChatNewChatButton_label_in_%s_locale",
    (locale, expected) => {
      mockedUseLocale.mockReturnValue(locale as "en" | "zh")
      render(<ChatNewChatButton />)
      expect(screen.getByTestId("chat-new-chat-button")).toHaveTextContent(
        expected,
      )
    },
  )

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
