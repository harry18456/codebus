import { render, screen } from "@testing-library/react"
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

import { ChatTokenDisplay } from "./ChatTokenDisplay"

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
    lastTranscript: INITIAL_STATE.lastTranscript,
    lastSessionId: INITIAL_STATE.lastSessionId,
  })
}

describe("ChatTokenDisplay", () => {
  beforeEach(() => {
    resetStore()
  })

  afterEach(() => {
    resetStore()
  })

  // Scenario: Token total renders in widget header
  it("renders summed input + output tokens as `<N>k ↑`", () => {
    useChatStore.setState({
      tokensTotal: {
        input_tokens: 1200,
        output_tokens: 2221,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
      },
    })
    render(<ChatTokenDisplay />)
    const el = screen.getByTestId("chat-token-display")
    // 1200 + 2221 = 3421 → 3.421 / 1000 → "3.4k ↑"
    expect(el.textContent).toContain("3.4k ↑")
  })

  // Scenario: Tooltip reveals token breakdown on hover
  it("exposes a tooltip with the four-way breakdown for hover/focus reveal", () => {
    useChatStore.setState({
      tokensTotal: {
        input_tokens: 1200,
        output_tokens: 2221,
        cache_read_tokens: 4500,
        cache_write_tokens: 800,
      },
    })
    render(<ChatTokenDisplay />)
    const el = screen.getByTestId("chat-token-display")
    const title = el.getAttribute("title") ?? ""
    // Spec wants four labeled values present on hover.
    expect(title).toMatch(/input/i)
    expect(title).toMatch(/output/i)
    expect(title).toMatch(/cache read/i)
    expect(title).toMatch(/cache create/i)
    // And the actual counts must be reachable on hover so the user can read
    // the per-bucket numbers, not just labels.
    expect(title).toContain("1200")
    expect(title).toContain("2221")
    expect(title).toContain("4500")
    expect(title).toContain("800")
  })

  // Scenario: Zero state renders 0k
  it("renders `0 ↑` for a fresh session with no turns (not hidden)", () => {
    // resetStore already wipes tokensTotal to zeros; explicit assertion for
    // clarity.
    useChatStore.setState({
      tokensTotal: { input_tokens: 0, output_tokens: 0 },
    })
    render(<ChatTokenDisplay />)
    const el = screen.getByTestId("chat-token-display")
    expect(el).toBeInTheDocument()
    expect(el.textContent).toContain("0 ↑")
  })

  it("rounds totals ≥ 10k to whole-k integers", () => {
    useChatStore.setState({
      tokensTotal: { input_tokens: 21000, output_tokens: 14600 },
    })
    render(<ChatTokenDisplay />)
    // 35600 → 35.6k → rounded → "36k ↑"
    expect(screen.getByTestId("chat-token-display").textContent).toContain(
      "36k ↑",
    )
  })

  it("renders sub-1k totals as the raw integer", () => {
    useChatStore.setState({
      tokensTotal: { input_tokens: 200, output_tokens: 50 },
    })
    render(<ChatTokenDisplay />)
    // 250 < 1000 → "250 ↑"
    expect(screen.getByTestId("chat-token-display").textContent).toContain(
      "250 ↑",
    )
  })
})
