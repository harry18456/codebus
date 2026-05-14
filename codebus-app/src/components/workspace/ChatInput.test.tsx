import { fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

// Mock Tauri bus so importing modules that pull in the chat store (which
// subscribes to chat-stream / chat-terminal at module load) is safe in jsdom.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

// Mock useChatStore as a selector hook. Tests drive `mockState` directly to
// flip between the idle (`activeTurn: null`) and active-turn cases without
// touching the real zustand instance.
const spawnTurnMock = vi.fn()
const cancelActiveTurnMock = vi.fn()

interface MockState {
  activeTurn: unknown
  spawnTurn: (vaultPath: string, text: string) => void
  cancelActiveTurn: () => void
}

let mockState: MockState = {
  activeTurn: null,
  spawnTurn: spawnTurnMock,
  cancelActiveTurn: cancelActiveTurnMock,
}

vi.mock("@/store/chat", () => ({
  useChatStore: <T,>(selector: (s: MockState) => T): T => selector(mockState),
}))

import { ChatInput } from "./ChatInput"

describe("ChatInput", () => {
  beforeEach(() => {
    spawnTurnMock.mockReset()
    cancelActiveTurnMock.mockReset()
    mockState = {
      activeTurn: null,
      spawnTurn: spawnTurnMock,
      cancelActiveTurn: cancelActiveTurnMock,
    }
  })

  afterEach(() => {
    mockState = {
      activeTurn: null,
      spawnTurn: spawnTurnMock,
      cancelActiveTurn: cancelActiveTurnMock,
    }
  })

  it("ChatInput_type_then_Enter_calls_spawnTurn", () => {
    render(<ChatInput vaultPath="/v" />)
    const textarea = screen.getByTestId("chat-input-textarea") as HTMLTextAreaElement
    fireEvent.change(textarea, { target: { value: "hello" } })
    fireEvent.keyDown(textarea, { key: "Enter" })
    expect(spawnTurnMock).toHaveBeenCalledTimes(1)
    expect(spawnTurnMock).toHaveBeenCalledWith("/v", "hello")
  })

  it("ChatInput_Send_button_click_equivalent_to_Enter", () => {
    render(<ChatInput vaultPath="/v" />)
    const textarea = screen.getByTestId("chat-input-textarea") as HTMLTextAreaElement
    fireEvent.change(textarea, { target: { value: "hi there" } })
    fireEvent.click(screen.getByTestId("chat-input-send"))
    expect(spawnTurnMock).toHaveBeenCalledTimes(1)
    expect(spawnTurnMock).toHaveBeenCalledWith("/v", "hi there")
  })

  it("ChatInput_renders_Stop_button_when_active_turn", () => {
    mockState = {
      activeTurn: {
        vaultPath: "/v",
        userText: "in flight",
        runId: "r1",
        events: [],
        cancelling: false,
        startedAt: "2026-05-14T00:00:00Z",
      },
      spawnTurn: spawnTurnMock,
      cancelActiveTurn: cancelActiveTurnMock,
    }
    render(<ChatInput vaultPath="/v" />)
    expect(screen.getByTestId("chat-input-stop")).toBeInTheDocument()
    expect(screen.queryByTestId("chat-input-send")).not.toBeInTheDocument()
  })

  it("ChatInput_Stop_click_calls_cancelActiveTurn", () => {
    mockState = {
      activeTurn: {
        vaultPath: "/v",
        userText: "in flight",
        runId: "r1",
        events: [],
        cancelling: false,
        startedAt: "2026-05-14T00:00:00Z",
      },
      spawnTurn: spawnTurnMock,
      cancelActiveTurn: cancelActiveTurnMock,
    }
    render(<ChatInput vaultPath="/v" />)
    fireEvent.click(screen.getByTestId("chat-input-stop"))
    expect(cancelActiveTurnMock).toHaveBeenCalledTimes(1)
  })
})
