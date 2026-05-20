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
import { useSettingsStore } from "@/store/settings"
import { useWikiStore } from "@/store/wiki"

const invokeMock = vi.mocked(invoke)

const VAULT: VaultEntry = {
  path: "/v",
  display_name: "vault",
  last_opened: "2026-05-13T00:00:00Z",
  is_missing: false,
}

const CHAT_INITIAL_STATE = useChatStore.getState()
const SETTINGS_INITIAL_STATE = useSettingsStore.getState()

function resetSettingsStore(): void {
  useSettingsStore.setState({
    config: SETTINGS_INITIAL_STATE.config,
    initialConfig: SETTINGS_INITIAL_STATE.initialConfig,
    dirty: false,
    loading: false,
    saving: false,
    error: null,
  })
}

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
    resetSettingsStore()
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
    resetSettingsStore()
  })

  it("loads global config into the settings store on mount", async () => {
    // Route invoke by command name: load_global_config returns a config
    // with a non-default pass_threshold; everything else returns [].
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "load_global_config") {
        return Promise.resolve({ app: { quiz: { pass_threshold: 75 } } })
      }
      return Promise.resolve([])
    })

    render(<Workspace vault={VAULT} />)

    await waitFor(() => {
      expect(
        useSettingsStore.getState().config.app?.quiz?.pass_threshold,
      ).toBe(75)
    })
    const loadCalls = invokeMock.mock.calls.filter(
      (c) => c[0] === "load_global_config",
    )
    expect(loadCalls).toHaveLength(1)
  })

  it("re-selecting the already-active Quiz tab returns to quiz history", async () => {
    render(<Workspace vault={VAULT} />)
    fireEvent.click(screen.getByTestId("workspace-tab-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
    // Enter a quiz flow: + New quiz opens the topic-input view, so the
    // history list is no longer shown.
    fireEvent.click(screen.getByTestId("new-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-topic-input")).toBeInTheDocument(),
    )
    expect(screen.queryByTestId("quiz-history")).not.toBeInTheDocument()
    // Selecting the Quiz tab again while it is already active returns
    // the Quiz tab to its quiz-history view (design D2).
    fireEvent.click(screen.getByTestId("workspace-tab-quiz"))
    await waitFor(() =>
      expect(screen.getByTestId("quiz-history")).toBeInTheDocument(),
    )
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

  // ---- Watcher lifecycle (codebus-fs-watcher tasks 2.6 / 2.3) ----

  it("mount_starts_watcher", async () => {
    invokeMock.mockResolvedValue([])
    render(<Workspace vault={VAULT} />)
    await waitFor(() => {
      const calls = invokeMock.mock.calls.map((c) => c[0])
      expect(calls).toContain("start_vault_watcher")
    })
    const startCall = invokeMock.mock.calls.find(
      (c) => c[0] === "start_vault_watcher",
    )
    expect(startCall?.[1]).toEqual({ vaultPath: VAULT.path })
  })

  it("unmount_stops_watcher", async () => {
    invokeMock.mockResolvedValue([])
    const { unmount } = render(<Workspace vault={VAULT} />)
    await waitFor(() => {
      expect(
        invokeMock.mock.calls.map((c) => c[0]),
      ).toContain("start_vault_watcher")
    })
    unmount()
    const stopCall = invokeMock.mock.calls.find(
      (c) => c[0] === "stop_vault_watcher",
    )
    expect(stopCall?.[1]).toEqual({ vaultPath: VAULT.path })
  })
})
