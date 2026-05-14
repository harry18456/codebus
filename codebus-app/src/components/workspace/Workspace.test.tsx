import { act, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

// Milkdown mocks are needed because the WikiTab path (even when not
// active) is imported through Workspace -> WikiTab -> WikiPreview.
vi.mock("@milkdown/core", () => ({
  Editor: { make: vi.fn(() => ({
    config: vi.fn(function (this: object) { return this }),
    use: vi.fn(function (this: object) { return this }),
    create: vi.fn(() => Promise.resolve({ destroy: vi.fn() })),
  })) },
  rootCtx: "rootCtx",
  defaultValueCtx: "defaultValueCtx",
  editorViewOptionsCtx: "editorViewOptionsCtx",
}))
vi.mock("@milkdown/preset-commonmark", () => ({ commonmark: () => ({}) }))

import { invoke } from "@tauri-apps/api/core"
import type { VaultEntry } from "@/lib/ipc"
import { Workspace } from "./Workspace"
import { useChatStore } from "@/store/chat"
import { useGoalsStore } from "@/store/goals"
import { useWikiStore } from "@/store/wiki"

const invokeMock = vi.mocked(invoke)

const VAULT: VaultEntry = {
  path: "/v",
  display_name: "vault",
  last_opened: "2026-05-13T00:00:00Z",
  is_missing: false,
}

const CHAT_INITIAL_STATE = useChatStore.getState()

function resetChatStore(): void {
  useChatStore.setState({
    sessionId: null,
    turns: [],
    activeTurn: null,
    tokensTotal: { input_tokens: 0, output_tokens: 0 },
    promoteSuggestion: null,
    expanded: false,
    width: CHAT_INITIAL_STATE.width,
    height: CHAT_INITIAL_STATE.height,
    onboardedVaults: new Set<string>(),
    lastTranscript: null,
    lastSessionId: null,
  })
}

describe("Workspace", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    invokeMock.mockResolvedValue([])
    useGoalsStore.setState({ runs: [], activeRun: null })
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
    resetChatStore()
  })

  afterEach(() => {
    useGoalsStore.setState({ runs: [], activeRun: null })
    useWikiStore.setState({
      pages: {},
      currentPath: null,
      body: null,
      _bodyCache: {},
    })
    resetChatStore()
  })

  it("Workspace_mounts_with_goals_tab_default", () => {
    render(<Workspace vault={VAULT} />)
    const goalsTabBtn = screen.getByTestId("workspace-tab-goals")
    expect(goalsTabBtn.getAttribute("data-active")).toBe("true")
    const main = screen.getByTestId("workspace-main")
    expect(main).toContainElement(screen.getByTestId("goals-tab"))
  })

  it("renders the vault display name and path in the sidebar", () => {
    render(<Workspace vault={VAULT} />)
    expect(screen.getByTestId("workspace-vault-name")).toHaveTextContent(
      "vault",
    )
    expect(screen.getByTestId("workspace-vault-path")).toHaveTextContent("/v")
  })

  it("keeps chat widget expanded state across tab switches", () => {
    render(<Workspace vault={VAULT} />)
    // Open the chat widget by toggling the store directly (sidesteps the
    // collapsed-bubble click which lives inside the widget). After expand
    // we navigate Goals → Wiki → Goals and assert the widget is still
    // mounted + still in `expanded` state.
    act(() => {
      useChatStore.getState().toggleExpanded()
    })
    expect(screen.getByTestId("chat-widget")).toHaveAttribute(
      "data-state",
      "expanded",
    )
    fireEvent.click(screen.getByTestId("workspace-tab-wiki"))
    expect(screen.getByTestId("chat-widget")).toHaveAttribute(
      "data-state",
      "expanded",
    )
    fireEvent.click(screen.getByTestId("workspace-tab-goals"))
    expect(screen.getByTestId("chat-widget")).toHaveAttribute(
      "data-state",
      "expanded",
    )
    // The store-level session id stays untouched across tab swaps.
    expect(useChatStore.getState().expanded).toBe(true)
  })

  it("resets chat store for vault on unmount", () => {
    // Seed a session so we can prove resetForVault wiped it.
    useChatStore.setState({
      sessionId: "session-keep",
      turns: [
        {
          userText: "hello",
          events: [],
          startedAt: "2026-05-14T00:00:00Z",
          finishedAt: "2026-05-14T00:00:01Z",
        },
      ],
    })
    // Also seed widget expanded so we can prove vault-switch collapses it.
    useChatStore.setState({ expanded: true })
    const { unmount } = render(<Workspace vault={VAULT} />)
    expect(useChatStore.getState().sessionId).toBe("session-keep")
    unmount()
    // resetForVault clears sessionId + turns AND collapses the widget back
    // to a bubble. Width / height (user resize) intentionally survive.
    expect(useChatStore.getState().sessionId).toBeNull()
    expect(useChatStore.getState().turns).toEqual([])
    expect(useChatStore.getState().expanded).toBe(false)
  })

  it("switches to Goals tab + RunDetailRunning after promote suggestion accepted", async () => {
    // Stub the store action so we don't need to drive the spawn_goal IPC
    // through chat-stream; we only care about the Workspace-level routing
    // side-effects (setActiveTab + setSelectedRunId + setSelectedDetail).
    const PROMOTED_RUN_ID = "run-promoted"
    const acceptSpy = vi
      .spyOn(useChatStore.getState(), "acceptPromoteSuggestion")
      .mockImplementation(async () => {
        // Mirror real store behavior: collapse widget on success.
        useChatStore.setState({ expanded: false, promoteSuggestion: null })
        return PROMOTED_RUN_ID
      })

    // Seed a pending promote suggestion on the assistant's last turn, and
    // park `useGoalsStore.activeRun` so the GoalsArea router renders
    // RunDetailRunning for the promoted run id.
    useChatStore.setState({
      expanded: true,
      promoteSuggestion: { reason: "Make a goal", turnIndex: 0 },
      turns: [
        {
          userText: "let's promote",
          events: [],
          startedAt: "2026-05-14T00:00:00Z",
          finishedAt: "2026-05-14T00:00:01Z",
        },
      ],
    })
    useGoalsStore.setState({
      runs: [],
      activeRun: {
        runId: PROMOTED_RUN_ID,
        goal: "Make a goal",
        startedAt: "2026-05-14T00:00:02Z",
        events: [],
        cancelling: false,
      },
    })

    // Start on the Wiki tab so we can prove the promote-accept handler
    // flips back to Goals.
    render(<Workspace vault={VAULT} />)
    fireEvent.click(screen.getByTestId("workspace-tab-wiki"))
    expect(screen.getByTestId("workspace-tab-wiki")).toHaveAttribute(
      "data-active",
      "true",
    )

    // Trigger the inline promote pill in the transcript.
    const promotePill = await screen.findByTestId("promote-pill")
    const promoteBtn = promotePill.querySelector("button")
    expect(promoteBtn).not.toBeNull()
    await act(async () => {
      fireEvent.click(promoteBtn!)
    })

    await waitFor(() => {
      expect(screen.getByTestId("workspace-tab-goals")).toHaveAttribute(
        "data-active",
        "true",
      )
    })
    // RunDetailRunning is the live view when activeRun.runId === selectedRunId.
    expect(screen.getByTestId("run-detail-running")).toBeInTheDocument()

    acceptSpy.mockRestore()
  })
})
