import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

// Mock Tauri event/core BEFORE importing the store, mirroring goals.test.ts
// pattern. The store module subscribes to "chat-stream" / "chat-terminal" at
// import time, so the mocks must be in place by then.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { listen } from "@tauri-apps/api/event"
import { invoke } from "@tauri-apps/api/core"
import { useChatStore } from "./chat"
import { useSettingsStore } from "./settings"

const listenMock = vi.mocked(listen)
const invokeMock = vi.mocked(invoke)

function setActiveProviderProfile(provider: string, profile: string): void {
  useSettingsStore.setState({
    config: { agent: { active_provider: provider, providers: { [provider]: { active: profile } } } },
  } as never)
}

/**
 * Snapshot the initial state once at module load so each test can reset
 * the store back to it without having to know the field-by-field defaults.
 * Captured AFTER the mock is in place so no live listeners leak.
 */
const INITIAL_STATE = useChatStore.getState()

function resetStore(): void {
  useChatStore.setState({
    sessionId: INITIAL_STATE.sessionId,
    sessionProviderKey: INITIAL_STATE.sessionProviderKey,
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
}

describe("useChatStore", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetStore()
  })

  afterEach(() => {
    vi.useRealTimers()
    listenMock.mockClear()
  })

  it("Vault switch resets the chat session and returns widget to bubble mode", () => {
    // Seed an active session in modal mode for V1 (covers the failure-mode
    // mitigation: modal open during vault switch must reset mode too).
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
      mode: "modal",
      modalReturnMode: "floating",
    })

    // Workspace unmount on vault switch calls resetForVault for V2.
    useChatStore.getState().resetForVault("/v2")

    const s = useChatStore.getState()
    expect(s.sessionId).toBeNull()
    expect(s.turns).toHaveLength(0)
    expect(s.activeTurn).toBeNull()
    expect(s.promoteSuggestion).toBeNull()
    expect(s.lastSessionId).toBeNull()
    expect(s.lastTranscript).toBeNull()
    // Per spec "Chat Session Lifecycle and Reset Triggers" scenario
    // "Vault switch resets the chat session and returns widget to bubble mode":
    // mode reverts to bubble + modalReturnMode clears.
    expect(s.mode).toBe("bubble")
    expect(s.modalReturnMode).toBeNull()
  })

  it("+ New chat triggers undo toast (saves last buffer + clears session)", () => {
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
    })

    useChatStore.getState().newSession()

    const s = useChatStore.getState()
    expect(s.sessionId).toBeNull()
    expect(s.turns).toHaveLength(0)
    // Undo buffer holds the previous session so the toast's [Undo] can restore.
    expect(s.lastSessionId).toBe("abc-123")
    expect(s.lastTranscript).toHaveLength(3)
  })

  it("Undo within 5 seconds restores session", () => {
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
    })

    useChatStore.getState().newSession()
    // Within the 5s window (advance 2 seconds), user clicks Undo.
    vi.advanceTimersByTime(2000)
    useChatStore.getState().undoNewSession()

    const s = useChatStore.getState()
    expect(s.sessionId).toBe("abc-123")
    expect(s.turns).toHaveLength(3)
    expect(s.lastSessionId).toBeNull()
    expect(s.lastTranscript).toBeNull()

    // The pending gc timer should be cancelled so it can't null-out the
    // restored session a few seconds later.
    vi.advanceTimersByTime(10_000)
    const after = useChatStore.getState()
    expect(after.sessionId).toBe("abc-123")
    expect(after.turns).toHaveLength(3)
  })

  it("Undo buffer gc'd after 5 seconds", () => {
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
      ],
    })

    useChatStore.getState().newSession()
    // Still within the window.
    vi.advanceTimersByTime(4999)
    expect(useChatStore.getState().lastSessionId).toBe("abc-123")

    // Cross the 5s boundary — gc fires.
    vi.advanceTimersByTime(2)
    const s = useChatStore.getState()
    expect(s.lastSessionId).toBeNull()
    expect(s.lastTranscript).toBeNull()
  })

  it("Tab switch preserves chat session and mode (mode-switch actions do not clobber session)", () => {
    // Tab switching is a Workspace concern — at the store level it simply
    // means: nothing happens. We verify that none of the mode-switch actions
    // clobber sessionId / turns, since the widget remains mounted while tabs
    // switch. Covers spec "Chat Session Lifecycle and Reset Triggers" scenario
    // "Tab switch preserves chat session and mode".
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
      mode: "floating",
    })

    // Exercise the full mode-switch cycle.
    useChatStore.getState().openModal()
    useChatStore.getState().dockToFloating()
    useChatStore.getState().minimizeToBubble()
    useChatStore.getState().openFloating()

    const s = useChatStore.getState()
    expect(s.sessionId).toBe("abc-123")
    expect(s.turns).toHaveLength(3)
    expect(s.mode).toBe("floating")
  })
})

describe("useChatStore mode state machine", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetStore()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it("starts in bubble mode with modalReturnMode = null", () => {
    const s = useChatStore.getState()
    expect(s.mode).toBe("bubble")
    expect(s.modalReturnMode).toBeNull()
  })

  it("openFloating: bubble → floating with modalReturnMode null", () => {
    useChatStore.setState({ mode: "bubble", modalReturnMode: null })
    useChatStore.getState().openFloating()
    const s = useChatStore.getState()
    expect(s.mode).toBe("floating")
    expect(s.modalReturnMode).toBeNull()
  })

  it("openFloating: no-op when mode !== 'bubble'", () => {
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
    useChatStore.getState().openFloating()
    expect(useChatStore.getState().mode).toBe("floating")

    useChatStore.setState({ mode: "modal", modalReturnMode: "floating" })
    useChatStore.getState().openFloating()
    expect(useChatStore.getState().mode).toBe("modal")
    expect(useChatStore.getState().modalReturnMode).toBe("floating")
  })

  it("minimizeToBubble: floating → bubble with modalReturnMode null", () => {
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
    useChatStore.getState().minimizeToBubble()
    const s = useChatStore.getState()
    expect(s.mode).toBe("bubble")
    expect(s.modalReturnMode).toBeNull()
  })

  it("minimizeToBubble: no-op when mode !== 'floating'", () => {
    useChatStore.setState({ mode: "modal", modalReturnMode: "bubble" })
    useChatStore.getState().minimizeToBubble()
    expect(useChatStore.getState().mode).toBe("modal")
    expect(useChatStore.getState().modalReturnMode).toBe("bubble")
  })

  it("openModal from bubble: snapshots 'bubble' as modalReturnMode", () => {
    useChatStore.setState({ mode: "bubble", modalReturnMode: null })
    useChatStore.getState().openModal()
    const s = useChatStore.getState()
    expect(s.mode).toBe("modal")
    expect(s.modalReturnMode).toBe("bubble")
  })

  it("openModal from floating: snapshots 'floating' as modalReturnMode", () => {
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
    useChatStore.getState().openModal()
    const s = useChatStore.getState()
    expect(s.mode).toBe("modal")
    expect(s.modalReturnMode).toBe("floating")
  })

  it("openModal while already in modal: no-op, does NOT re-snapshot modalReturnMode", () => {
    // Failure-mode case: multiple ⌘K presses must not overwrite the snapshot.
    useChatStore.setState({ mode: "modal", modalReturnMode: "bubble" })
    useChatStore.getState().openModal()
    expect(useChatStore.getState().mode).toBe("modal")
    expect(useChatStore.getState().modalReturnMode).toBe("bubble")
  })

  it("dockToFloating: modal → floating with modalReturnMode null (ignores prior return mode)", () => {
    // Per spec "Modal dock button always returns to floating" — ignores
    // modalReturnMode regardless of value.
    for (const returnMode of ["bubble", "floating", null] as const) {
      useChatStore.setState({ mode: "modal", modalReturnMode: returnMode })
      useChatStore.getState().dockToFloating()
      const s = useChatStore.getState()
      expect(s.mode).toBe("floating")
      expect(s.modalReturnMode).toBeNull()
    }
  })

  it("dockToFloating: no-op when mode !== 'modal'", () => {
    useChatStore.setState({ mode: "bubble", modalReturnMode: null })
    useChatStore.getState().dockToFloating()
    expect(useChatStore.getState().mode).toBe("bubble")
  })

  it("closeModalToReturnMode: modal → modalReturnMode value", () => {
    useChatStore.setState({ mode: "modal", modalReturnMode: "floating" })
    useChatStore.getState().closeModalToReturnMode()
    expect(useChatStore.getState().mode).toBe("floating")
    expect(useChatStore.getState().modalReturnMode).toBeNull()

    useChatStore.setState({ mode: "modal", modalReturnMode: "bubble" })
    useChatStore.getState().closeModalToReturnMode()
    expect(useChatStore.getState().mode).toBe("bubble")
    expect(useChatStore.getState().modalReturnMode).toBeNull()
  })

  it("closeModalToReturnMode: falls back to bubble when modalReturnMode is null", () => {
    useChatStore.setState({ mode: "modal", modalReturnMode: null })
    useChatStore.getState().closeModalToReturnMode()
    expect(useChatStore.getState().mode).toBe("bubble")
    expect(useChatStore.getState().modalReturnMode).toBeNull()
  })

  it("closeModalToReturnMode: no-op when mode !== 'modal'", () => {
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
    useChatStore.getState().closeModalToReturnMode()
    expect(useChatStore.getState().mode).toBe("floating")
  })

  it("closeModalToBubble: modal → bubble regardless of modalReturnMode", () => {
    for (const returnMode of ["bubble", "floating", null] as const) {
      useChatStore.setState({ mode: "modal", modalReturnMode: returnMode })
      useChatStore.getState().closeModalToBubble()
      const s = useChatStore.getState()
      expect(s.mode).toBe("bubble")
      expect(s.modalReturnMode).toBeNull()
    }
  })

  it("closeModalToBubble: no-op when mode !== 'modal'", () => {
    useChatStore.setState({ mode: "floating", modalReturnMode: null })
    useChatStore.getState().closeModalToBubble()
    expect(useChatStore.getState().mode).toBe("floating")
  })

  it("acceptPromoteSuggestion success collapses to bubble", async () => {
    // Per Task 1.1 alignment finding: acceptPromoteSuggestion previously
    // set `expanded: false`; in the three-mode model it must set
    // `mode: "bubble"` + `modalReturnMode: null`.
    invokeMock.mockResolvedValue("goal-run-1")
    useChatStore.setState({
      mode: "modal",
      modalReturnMode: "floating",
      promoteSuggestion: { reason: "test reason", turnIndex: 0 },
      turns: [
        {
          userText: "promote me",
          events: [],
          startedAt: "t1",
          finishedAt: "t1",
        },
      ],
    })

    vi.useRealTimers()
    await useChatStore.getState().acceptPromoteSuggestion("/v")
    vi.useFakeTimers()

    const s = useChatStore.getState()
    expect(s.promoteSuggestion).toBeNull()
    expect(s.mode).toBe("bubble")
    expect(s.modalReturnMode).toBeNull()
  })
})

describe("provider/profile switch starts a fresh session", () => {
  beforeEach(() => {
    resetStore()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue("chat-run-1")
  })

  it("drops resume + clears transcript when the provider/profile changed", async () => {
    // Session established under codex/azure.
    setActiveProviderProfile("codex", "azure")
    useChatStore.setState({
      sessionId: "azure-sess",
      sessionProviderKey: "codex:azure",
      turns: [{ userText: "earlier", events: [], startedAt: "", finishedAt: "" }],
    })
    // User switched to codex/system, then sends a turn.
    setActiveProviderProfile("codex", "system")
    await useChatStore.getState().spawnTurn("/v", "hello")

    // Resume id dropped (fresh session) + old transcript cleared.
    const call = invokeMock.mock.calls.find((c) => c[0] === "spawn_chat_turn")
    expect((call?.[1] as { sessionId: string | null }).sessionId).toBeNull()
    expect(useChatStore.getState().turns).toEqual([])
    expect(useChatStore.getState().sessionProviderKey).toBe("codex:system")
  })

  it("keeps resuming when the provider/profile is unchanged", async () => {
    setActiveProviderProfile("codex", "system")
    useChatStore.setState({ sessionId: "sess-x", sessionProviderKey: "codex:system" })
    await useChatStore.getState().spawnTurn("/v", "again")
    const call = invokeMock.mock.calls.find((c) => c[0] === "spawn_chat_turn")
    expect((call?.[1] as { sessionId: string | null }).sessionId).toBe("sess-x")
  })
})

describe("terminal outcome surfaces failures", () => {
  beforeEach(() => resetStore())

  function activeTurn(runId: string) {
    useChatStore.setState({
      activeTurn: { vaultPath: "/v", userText: "hi", runId, events: [], cancelling: false, startedAt: "" },
    })
  }

  it("marks the finalized turn with an error when outcome is failed", () => {
    activeTurn("r1")
    useChatStore.getState()._onTerminal({ run_id: "r1", session_id: "s1", outcome: "failed" })
    const turns = useChatStore.getState().turns
    expect(turns).toHaveLength(1)
    expect(turns[0].error).toBeTruthy()
  })

  it("leaves the turn error-free when outcome is succeeded", () => {
    activeTurn("r2")
    useChatStore.getState()._onTerminal({ run_id: "r2", session_id: "s2", outcome: "succeeded" })
    expect(useChatStore.getState().turns[0].error).toBeUndefined()
  })
})
