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
    expanded: INITIAL_STATE.expanded,
    width: INITIAL_STATE.width,
    height: INITIAL_STATE.height,
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

  it("Vault switch resets the chat session", () => {
    // Seed an "active" session for V1.
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
      expanded: true,
      width: 30,
      height: 40,
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
    // Spec: vault switch collapses the widget back to a bubble so the
    // next vault opens in a clean visual state. Width / height survive
    // (user resize preference persists across the lobby round-trip).
    expect(s.expanded).toBe(false)
    expect(s.width).toBe(30)
    expect(s.height).toBe(40)
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

  it("Tab switch preserves chat session (store state untouched without a trigger)", () => {
    // Tab switching is a Workspace concern — at the store level it simply
    // means: nothing happens. We verify that none of the non-reset actions
    // (toggleExpanded, setSize) clobber sessionId / turns, since the widget
    // remains mounted while tabs switch.
    useChatStore.setState({
      sessionId: "abc-123",
      turns: [
        { userText: "q1", events: [], startedAt: "t1", finishedAt: "t1" },
        { userText: "q2", events: [], startedAt: "t2", finishedAt: "t2" },
        { userText: "q3", events: [], startedAt: "t3", finishedAt: "t3" },
      ],
      expanded: true,
    })

    // Simulate widget surviving across tab toggles + a resize.
    useChatStore.getState().toggleExpanded()
    useChatStore.getState().toggleExpanded()
    useChatStore.getState().setSize(28, 36)

    const s = useChatStore.getState()
    expect(s.sessionId).toBe("abc-123")
    expect(s.turns).toHaveLength(3)
    expect(s.expanded).toBe(true)
    expect(s.width).toBe(28)
    expect(s.height).toBe(36)
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
