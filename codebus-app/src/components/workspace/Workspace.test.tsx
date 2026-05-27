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
import { useQuizHistoryStore } from "@/store/quiz-history"
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
    useQuizHistoryStore.setState({
      vaultPath: null,
      attempts: [],
      loading: false,
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
    useQuizHistoryStore.setState({
      vaultPath: null,
      attempts: [],
      loading: false,
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

  it("loads quiz history attempts on mount and resets them on unmount", async () => {
    const attempt = {
      slug: "session-vs-token",
      quiz_id: "q1",
      trigger: "topic",
      topic: "session vs token",
      target_page: null,
      events_log: null,
      path: "/v/.codebus/quiz/session-vs-token/q1.md",
    }
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "list_quiz_attempts") return Promise.resolve([attempt])
      return Promise.resolve([])
    })

    const { unmount } = render(<Workspace vault={VAULT} />)
    await waitFor(() => {
      const state = useQuizHistoryStore.getState()
      expect(state.vaultPath).toBe("/v")
      expect(state.attempts).toHaveLength(1)
    })

    unmount()
    const after = useQuizHistoryStore.getState()
    expect(after.vaultPath).toBeNull()
    expect(after.attempts).toEqual([])
  })

  it("renders the vault display name and path in the sidebar", () => {
    render(<Workspace vault={VAULT} />)
    expect(screen.getByTestId("workspace-vault-name")).toHaveTextContent(
      "vault",
    )
    expect(screen.getByTestId("workspace-vault-path")).toHaveTextContent("/v")
  })

  describe("sidebar nav row visual contract", () => {
    it("renders each nav row with emoji prefix (aria-hidden), label and right-aligned mono count", () => {
      useGoalsStore.setState({
        runs: [
          { run_id: "r1", kind: "goal", outcome: "succeeded", title: "t", started_at: "x", finished_at: "y" },
          { run_id: "r2", kind: "goal", outcome: "succeeded", title: "t2", started_at: "x", finished_at: "y" },
        ] as any,
      })
      useWikiStore.setState({
        pages: { a: { slug: "a", path: "/p", title: "A" } as any },
      })
      useQuizHistoryStore.setState({
        vaultPath: "/v",
        attempts: [{ slug: "s", quiz_id: "q1", trigger: "topic", topic: null, target_page: null, events_log: null, path: "/p" }] as any,
        loading: false,
      })
      render(<Workspace vault={VAULT} />)

      const goalsTab = screen.getByTestId("workspace-tab-goals")
      const wikiTab = screen.getByTestId("workspace-tab-wiki")
      const quizTab = screen.getByTestId("workspace-tab-quiz")

      for (const [tab, emoji, label, count] of [
        [goalsTab, "🚏", "Goals", "2"],
        [wikiTab, "📂", "Wiki", "1"],
        [quizTab, "🎓", "Quiz", "1"],
      ] as const) {
        expect(tab.textContent).toContain(emoji)
        expect(tab.textContent).toContain(label)
        const countSpan = tab.querySelector('[data-testid$="-count"]') as HTMLElement
        expect(countSpan).toBeTruthy()
        expect(countSpan.textContent).toBe(count)
        // count uses font-mono tabular-nums tertiary fg
        expect(countSpan.className).toMatch(/font-mono/)
        expect(countSpan.className).toMatch(/tabular-nums/)
        expect(countSpan.className).toMatch(/text-fg-tertiary/)
        // emoji is wrapped in aria-hidden span (filter out the empty
        // left-bar span on the active row, which is also aria-hidden).
        const emojiSpan = Array.from(tab.querySelectorAll("span")).find(
          (s) =>
            s.getAttribute("aria-hidden") === "true" &&
            s.textContent === emoji,
        )
        expect(emojiSpan).toBeTruthy()
      }
    })

    it("displays literal 0 when the underlying store is empty", () => {
      render(<Workspace vault={VAULT} />)
      for (const id of ["goals", "wiki", "quiz"] as const) {
        const tab = screen.getByTestId(`workspace-tab-${id}`)
        const countSpan = tab.querySelector(`[data-testid="workspace-tab-${id}-count"]`) as HTMLElement
        expect(countSpan).toBeTruthy()
        expect(countSpan.textContent).toBe("0")
      }
    })

    it("active row shows a left amber bar that follows the active tab without residue", () => {
      render(<Workspace vault={VAULT} />)
      const goalsBar = () =>
        screen
          .getByTestId("workspace-tab-goals")
          .querySelector('[data-testid="workspace-tab-goals-bar"]')
      const wikiBar = () =>
        screen
          .getByTestId("workspace-tab-wiki")
          .querySelector('[data-testid="workspace-tab-wiki-bar"]')
      const quizBar = () =>
        screen
          .getByTestId("workspace-tab-quiz")
          .querySelector('[data-testid="workspace-tab-quiz-bar"]')

      // Initial: Goals active.
      expect(goalsBar()).toBeTruthy()
      expect(wikiBar()).toBeNull()
      expect(quizBar()).toBeNull()

      fireEvent.click(screen.getByTestId("workspace-tab-wiki"))
      expect(wikiBar()).toBeTruthy()
      expect(goalsBar()).toBeNull()
      expect(quizBar()).toBeNull()

      fireEvent.click(screen.getByTestId("workspace-tab-quiz"))
      expect(quizBar()).toBeTruthy()
      expect(goalsBar()).toBeNull()
      expect(wikiBar()).toBeNull()
    })

    it("emoji prefix characters are not present in the i18n message values for tab labels", async () => {
      const { messages } = await import("@/i18n/messages")
      for (const lang of [messages.en, messages.zh]) {
        for (const key of [
          "workspace.tab.goals",
          "workspace.tab.wiki",
          "workspace.tab.quiz",
        ] as const) {
          const value = (lang as Record<string, string>)[key]
          expect(value).toBeDefined()
          expect(value).not.toMatch(/[🚏📂🎓]/u)
        }
      }
    })
  })

  describe("sidebar section label policy", () => {
    it("does not render a VAULT or any section label between vault path and the first nav row", () => {
      render(<Workspace vault={VAULT} />)
      const sidebar = screen.getByTestId("workspace-sidebar")
      expect(sidebar.textContent ?? "").not.toMatch(/VAULT/)
      // No section label-style element with caps tracking between vault
      // path block and the nav region.
      const pathBlock = screen.getByTestId("workspace-vault-path")
      const nav = sidebar.querySelector("nav")
      expect(nav).toBeTruthy()
      // Walk from pathBlock's parent to nav and ensure no element with
      // uppercase tracking text exists between them.
      let between: Element | null | undefined = pathBlock.parentElement?.nextElementSibling
      while (between && between !== nav) {
        expect(between.textContent ?? "").not.toMatch(/VAULT|Vault/)
        between = between.nextElementSibling
      }
    })
  })

  describe("sidebar footer", () => {
    it("renders a Settings button and a ⌘K kbd chip, with no refresh button", () => {
      render(<Workspace vault={VAULT} />)
      const footer = screen.getByTestId("workspace-sidebar-footer")
      expect(footer).toBeInTheDocument()
      const settingsBtn = screen.getByTestId("workspace-sidebar-settings")
      expect(footer).toContainElement(settingsBtn)
      const kbdChip = screen.getByTestId("workspace-sidebar-kbd")
      expect(footer).toContainElement(kbdChip)
      expect(kbdChip.textContent).toContain("⌘")
      expect(kbdChip.textContent).toContain("K")
      expect(kbdChip.getAttribute("aria-hidden")).toBe("true")
      expect(
        screen.queryByTestId("workspace-sidebar-refresh"),
      ).not.toBeInTheDocument()
    })

    it("clicking sidebar Settings button invokes onOpenSettings", () => {
      const onOpenSettings = vi.fn()
      render(<Workspace vault={VAULT} onOpenSettings={onOpenSettings} />)
      fireEvent.click(screen.getByTestId("workspace-sidebar-settings"))
      expect(onOpenSettings).toHaveBeenCalledTimes(1)
    })
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
