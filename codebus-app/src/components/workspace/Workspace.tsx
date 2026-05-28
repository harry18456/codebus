import { useCallback, useEffect, useState } from "react"

import {
  getRunDetail,
  type RunDetail,
  type RunLogSummary,
  type VaultEntry,
} from "@/lib/ipc"
import { cn } from "@/lib/cn"
import { useChatStore } from "@/store/chat"
import { useGoalsStore } from "@/store/goals"
import { useQuizHistoryStore } from "@/store/quiz-history"
import { useRouteStore } from "@/store/route"
import { useSettingsStore } from "@/store/settings"
import { useVaultsStore } from "@/store/vaults"
import { useVaultWatcherStatusStore } from "@/store/vault-watcher-status"
import { useWikiStore } from "@/store/wiki"
import { useChatShortcut } from "@/hooks/useChatShortcut"
import { useWatcherEvent } from "@/hooks/useWatcherEvent"
import { useT } from "@/i18n/useT"
import {
  invoke as tauriInvoke,
} from "@tauri-apps/api/core"

import { Settings } from "lucide-react"

import { ChatWidget } from "./ChatWidget"
import { GoalsTab } from "./GoalsTab"
import { QuizTab } from "./QuizTab"
import { RunDetailInterrupted } from "./RunDetailInterrupted"
import { RunDetailDone } from "./RunDetailDone"
import { RunDetailRunning } from "./RunDetailRunning"
import { WikiTab } from "./WikiTab"

interface WorkspaceProps {
  vault: VaultEntry
  /**
   * Opens the application-shell `<SettingsModal>` from the Workspace
   * sidebar footer's Settings button. Owned by `AppShell` so Lobby
   * BottomStrip and Workspace sidebar share one modal instance.
   * Spec: app-shell § Settings Modal Invocation From Workspace Sidebar Footer.
   * Optional only so legacy unit tests that render `<Workspace>` in
   * isolation can omit it; production callers (`App.tsx`) MUST pass it.
   */
  onOpenSettings?: () => void
}

type TabId = "goals" | "wiki" | "quiz"

/**
 * Spec: app-workspace § Workspace Layout and Tab Navigation.
 *
 * Top-level Workspace shell that replaces `WorkspaceStub`. Renders a
 * left sidebar (vault display name + path + ← Back to Lobby + three
 * tab buttons) plus a main area that switches on the active tab.
 *
 * Mount: refresh goal runs and pre-load the wiki page index so the
 * Goals overview and the Wiki tab tree have data immediately.
 * Unmount: clear both stores so a fresh vault open starts clean.
 */
export function Workspace({ vault, onOpenSettings }: WorkspaceProps) {
  const t = useT()
  const back = useRouteStore((s) => s.back)
  const loadVaults = useVaultsStore((s) => s.loadVaults)
  const refreshRuns = useGoalsStore((s) => s.refreshRuns)
  const goalsReset = useGoalsStore((s) => s.reset)
  const activeRun = useGoalsStore((s) => s.activeRun)
  const listPages = useWikiStore((s) => s.listPages)
  const loadPage = useWikiStore((s) => s.loadPage)
  const wikiPages = useWikiStore((s) => s.pages)
  const wikiReset = useWikiStore((s) => s.reset)
  const loadQuizAttempts = useQuizHistoryStore((s) => s.loadAttempts)
  const quizHistoryReset = useQuizHistoryStore((s) => s.reset)

  const [activeTab, setActiveTab] = useState<TabId>("goals")
  // Monotonic counter bumped when the user selects the Quiz tab while it
  // is already the active tab. QuizTab watches it and returns to its
  // quiz-history view (design D2). Initial 0 is inert. Selecting Quiz
  // from a different tab is a normal tab switch and does NOT bump it.
  const [quizHomeSignal, setQuizHomeSignal] = useState(0)
  // task 5.3 — when set (via wiki preview [Quiz me on this]), the Quiz
  // tab consumes it to start the Page flow (skip planning).
  const [pendingQuizPage, setPendingQuizPage] = useState<string | null>(null)
  // wiki-page-reader-v1.1 / WP5 + WK-EMPTY: signal carried from the Wiki
  // tab edit-hint footer ("Run a goal" link) or the Wiki empty hero CTA
  // through to the Goals tab so the existing NewGoalModal opens (and
  // optionally pre-fills its goal description). `nonce` lets GoalsTab
  // distinguish "open with the same prefill again" from a stale signal.
  const [pendingNewGoalPrefill, setPendingNewGoalPrefill] = useState<{
    text: string
    nonce: number
  } | null>(null)
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [selectedDetail, setSelectedDetail] = useState<RunDetail | null>(null)

  // Bind Cmd+K / Ctrl+K to toggle the chat widget. The hook scopes the
  // window listener to Workspace mount/unmount so the shortcut is inert in
  // the Lobby — see `useChatShortcut` for the spec scenario this enforces.
  useChatShortcut()

  // Load persisted global config once at workspace startup so the Quiz
  // tab's pass threshold and generated question count reflect saved
  // settings without requiring the Settings modal to have been opened
  // (design D3). Guard: only when the store is still at its empty
  // initial config — never refight an in-flight load or clobber unsaved
  // edits. `getState()` is non-reactive, so the effect runs once.
  useEffect(() => {
    const s = useSettingsStore.getState()
    if (!s.loading && !s.dirty && Object.keys(s.config).length === 0) {
      void s.load().catch(() => {})
    }
  }, [])

  useEffect(() => {
    void refreshRuns(vault.path).catch(() => {})
    void listPages(vault.path).catch(() => {})
    // Sidebar Quiz nav count seam — see `useQuizHistoryStore`. Loaded on
    // mount + cleared on unmount so the count tracks vault scope without
    // forcing QuizTab to expose its component-local attempts state.
    void loadQuizAttempts(vault.path).catch(() => {})
    const vaultPath = vault.path
    return () => {
      goalsReset()
      wikiReset()
      quizHistoryReset()
      // Drop chat session + transcript + token tally + pending promote
      // suggestion when the user leaves the vault. resetForVault also
      // returns the widget to bubble mode (mode + modalReturnMode reset);
      // `onboardedVaults` survives per spec "Chat Session Lifecycle and
      // Reset Triggers" (per-vault localStorage flag).
      useChatStore.getState().resetForVault(vaultPath)
    }
  }, [
    vault.path,
    refreshRuns,
    listPages,
    loadQuizAttempts,
    goalsReset,
    wikiReset,
    quizHistoryReset,
  ])

  // When the goal thread finishes, `useGoalsStore.activeRun` flips
  // back to null via the `goal-terminal` channel. If the user was
  // sitting in the Running detail for that same run, fetch its
  // RunDetail so the Workspace can transition to the terminal view
  // (Done / Cancelled / Failed) automatically.
  useEffect(() => {
    if (!selectedRunId) return
    if (selectedDetail) return
    if (activeRun?.runId === selectedRunId) return
    let cancelled = false
    void getRunDetail(vault.path, selectedRunId)
      .then((detail) => {
        if (!cancelled) setSelectedDetail(detail)
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [selectedRunId, selectedDetail, activeRun, vault.path])

  // Watcher integration: external edits to the open run's
  // `events-<run>.jsonl` (terminal-spawned runs, live appends) SHALL
  // re-fetch the detail so the displayed timeline stays current. The
  // payload's run_id is compared against the currently selected run;
  // mismatches are ignored to avoid churning unrelated state. Spec:
  // `Goals Tab Subscribes To Watcher Events`.
  useEffect(
    () =>
      useWatcherEvent("goal-run-changed", (payload) => {
        if (!selectedRunId) return
        if (payload.run_id !== selectedRunId) return
        let cancelled = false
        void getRunDetail(vault.path, selectedRunId)
          .then((detail) => {
            if (!cancelled) setSelectedDetail(detail)
          })
          .catch(() => {})
        // No cancellation handle needed: useWatcherEvent's cleanup
        // tears down the subscription on unmount, so subsequent
        // promise resolutions race only against state setters that
        // React already gates on mount.
        void cancelled
      }),
    [selectedRunId, vault.path],
  )

  // Subscribe to `vault-watcher-error` so a failed watcher startup
  // populates the disabled-status store. The banner in each tab reads
  // from that store and surfaces the failure. The subscription is
  // session-scoped (cleanup on unmount) per spec.
  useEffect(
    () =>
      useWatcherEvent("vault-watcher-error", (payload) => {
        useVaultWatcherStatusStore
          .getState()
          .markDisabled(payload.vault_path, payload.reason)
      }),
    [],
  )

  // Per-vault watcher lifecycle bound to Workspace mount/unmount.
  // Spec: `Workspace Manages Per-Vault Watcher Lifecycle`. Vault
  // switch (new Workspace mount for a different vault) releases the
  // prior watcher via the cleanup return, then starts the new one.
  useEffect(() => {
    const path = vault.path
    void tauriInvoke("start_vault_watcher", { vaultPath: path })
    return () => {
      void tauriInvoke("stop_vault_watcher", { vaultPath: path })
    }
  }, [vault.path])

  const onSelectRun = useCallback(
    async (run: RunLogSummary) => {
      setSelectedRunId(run.run_id)
      // Running rows are driven by the live `activeRun` buffer, not by
      // an on-disk RunDetail (the events file is still being appended).
      if (run.outcome === "running") {
        setSelectedDetail(null)
        return
      }
      try {
        const detail = await getRunDetail(vault.path, run.run_id)
        setSelectedDetail(detail)
      } catch {
        setSelectedDetail(null)
      }
    },
    [vault.path],
  )

  const onBackToList = useCallback(() => {
    setSelectedRunId(null)
    setSelectedDetail(null)
  }, [])

  /**
   * After spawn / retry resolves, jump straight into the new
   * Running detail view instead of dropping the user back to the
   * Goals overview. activeRun is already populated by spawnGoal
   * optimistically, so the Workspace router will pick
   * `RunDetailRunning` on the next render.
   */
  const onSelectRunId = useCallback((runId: string) => {
    setSelectedRunId(runId)
    setSelectedDetail(null)
  }, [])

  const onSelectPage = useCallback(
    (slug: string) => {
      setActiveTab("wiki")
      void loadPage(vault.path, slug)
    },
    [vault.path, loadPage],
  )

  /**
   * After an inline `[Promote to goal]` click in the chat transcript
   * resolves, jump the user into RunDetailRunning for the freshly
   * spawned goal. The chat store has already collapsed the widget +
   * cleared the suggestion, so Workspace only owns the routing bits.
   */
  const handlePromoteSuccess = useCallback((runId: string) => {
    setActiveTab("goals")
    setSelectedRunId(runId)
    setSelectedDetail(null)
  }, [])

  function handleBack() {
    back()
    void loadVaults()
  }

  // Sidebar nav counts come from stores so the count tracks live store
  // changes (goal spawn / watcher-driven wiki refresh / quiz attempt write)
  // without prop drilling. Spec: app-workspace § Workspace Sidebar Nav
  // Row Visual Contract (count source).
  const goalsCount = useGoalsStore((s) => s.runs.length)
  const wikiCount = useWikiStore((s) => Object.keys(s.pages).length)
  const quizCount = useQuizHistoryStore((s) => s.attempts.length)

  return (
    <main data-testid="workspace" className="flex h-full w-full">
      <aside
        data-testid="workspace-sidebar"
        data-tauri-drag-region
        className="flex w-[200px] flex-col gap-2 border-r border-border bg-bg-sunken p-4"
      >
        <button
          type="button"
          onClick={handleBack}
          data-testid="workspace-back"
          className="text-left text-meta text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
        >
          {t("workspace.backToLobby")}
        </button>
        <div className="mt-2 border-t border-border pt-2">
          <div
            data-testid="workspace-vault-name"
            className="text-sm font-semibold"
          >
            {vault.display_name}
          </div>
          <button
            type="button"
            data-testid="workspace-vault-path"
            onClick={() => void openVaultInFiles(vault.path)}
            title={t("workspace.sidebar.vaultPathHint", { path: vault.path })}
            className="block w-full truncate text-left font-mono text-meta text-fg-tertiary hover:text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            {vault.path}
          </button>
        </div>
        {/*
         * Spec: app-workspace § Workspace Sidebar Section Label Policy.
         * The sidebar nav region SHALL NOT render any section label above
         * the three tab rows (the design v1 mock's `VAULT` label is
         * deliberately not adopted — only 3 ungrouped tabs, so a section
         * heading would be visual noise). Do not re-introduce.
         */}
        <nav className="mt-4 flex flex-col gap-1">
          <TabButton
            id="goals"
            emoji="🚏"
            label={t("workspace.tab.goals")}
            count={goalsCount}
            activeTab={activeTab}
            onSelect={(next) => {
              setActiveTab(next)
              setSelectedRunId(null)
              setSelectedDetail(null)
            }}
          />
          <TabButton
            id="wiki"
            emoji="📂"
            label={t("workspace.tab.wiki")}
            count={wikiCount}
            activeTab={activeTab}
            onSelect={(next) => setActiveTab(next)}
          />
          <TabButton
            id="quiz"
            emoji="🎓"
            label={t("workspace.tab.quiz")}
            count={quizCount}
            activeTab={activeTab}
            onSelect={(next) => {
              if (activeTab === "quiz") {
                // Already on Quiz — re-selecting acts as "home": bump
                // the signal so QuizTab returns to quiz history (D2).
                setQuizHomeSignal((n) => n + 1)
              } else {
                setActiveTab(next)
              }
            }}
          />
        </nav>
        <div
          data-testid="workspace-sidebar-footer"
          className="mt-auto flex items-center justify-between border-t border-border pt-2"
        >
          <button
            type="button"
            data-testid="workspace-sidebar-settings"
            aria-label={t("bottomStrip.settings")}
            title={t("bottomStrip.settings")}
            onClick={() => onOpenSettings?.()}
            className="flex items-center gap-1.5 rounded-sm text-meta text-fg-secondary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            <Settings className="h-3.5 w-3.5" />
          </button>
          <span
            data-testid="workspace-sidebar-kbd"
            aria-hidden="true"
            className="flex items-center gap-0.5 font-mono text-[10px] text-fg-tertiary"
          >
            <kbd className="rounded-sm border border-border bg-bg-raised px-1 py-px">
              ⌘
            </kbd>
            <kbd className="rounded-sm border border-border bg-bg-raised px-1 py-px">
              K
            </kbd>
          </span>
        </div>
      </aside>
      <section
        data-testid="workspace-main"
        className="flex flex-1 flex-col"
      >
        {activeTab === "goals" && (
          <GoalsArea
            vaultPath={vault.path}
            selectedRunId={selectedRunId}
            selectedDetail={selectedDetail}
            activeRunId={activeRun?.runId ?? null}
            onSelectRun={onSelectRun}
            onSelectRunId={onSelectRunId}
            onBack={onBackToList}
            onSelectPage={onSelectPage}
            pendingNewGoalPrefill={pendingNewGoalPrefill}
            onPendingNewGoalConsumed={() => setPendingNewGoalPrefill(null)}
          />
        )}
        {activeTab === "wiki" && (
          <WikiTab
            vaultPath={vault.path}
            onRequestNewGoal={(prefilled) => {
              setPendingNewGoalPrefill({ text: prefilled, nonce: Date.now() })
              setActiveTab("goals")
            }}
            onWikiEmptyCta={() => {
              setPendingNewGoalPrefill({ text: "", nonce: Date.now() })
              setActiveTab("goals")
            }}
            onQuizMeOnThis={(path) => {
              setPendingQuizPage(path)
              setActiveTab("quiz")
            }}
          />
        )}
        {activeTab === "quiz" && (
          <QuizTab
            vaultPath={vault.path}
            pendingPage={pendingQuizPage}
            onPendingConsumed={() => setPendingQuizPage(null)}
            wikiPages={wikiPages}
            onOpenWikiPage={onSelectPage}
            quizHomeSignal={quizHomeSignal}
          />
        )}
      </section>
      {/*
       * ChatWidget lives at Workspace level (sibling of the tab-bound
       * `<section>`) so it survives tab switches — minimizing to bubble,
       * opening Wiki, then returning to Goals keeps the transcript,
       * sessionId, and `mode` state intact. The widget pins itself via
       * fixed position (or radix Portal for modal mode) so this DOM
       * placement does not affect layout, but the React subtree must
       * NOT live inside an `activeTab` conditional or it would unmount
       * and lose its state.
       */}
      <ChatWidget
        vaultPath={vault.path}
        onPromoteSuccess={handlePromoteSuccess}
        onWikiLinkClick={onSelectPage}
      />
    </main>
  )
}

/**
 * Open the vault path in the OS file explorer via the Tauri opener
 * plugin. Errors are swallowed because failure (missing plugin in
 * the test harness, OS handler unavailable) should not crash the UI.
 */
async function openVaultInFiles(path: string): Promise<void> {
  try {
    const { openPath } = await import("@tauri-apps/plugin-opener")
    await openPath(path)
  } catch (err) {
    console.error("openPath failed", err)
  }
}

interface TabButtonProps {
  id: TabId
  /** Inline emoji prefix; component-encoded, not sourced from i18n. */
  emoji: string
  label: string
  /** Store-driven count rendered right-aligned, mono / tabular-nums. */
  count: number
  activeTab: TabId
  onSelect: (tab: TabId) => void
}

/**
 * Sidebar nav row. Spec: app-workspace § Workspace Sidebar Nav Row Visual
 * Contract. Three segments: optional 2px left amber bar (active only),
 * emoji + label, right-aligned mono count. Active state uses the left
 * bar as the dominant signal; whole-row accent-tint fill was removed so
 * the bar reads cleanly (per spec).
 */
function TabButton({
  id,
  emoji,
  label,
  count,
  activeTab,
  onSelect,
}: TabButtonProps) {
  const active = activeTab === id
  return (
    <button
      type="button"
      data-testid={`workspace-tab-${id}`}
      data-active={active}
      onClick={() => onSelect(id)}
      className={cn(
        "relative flex w-full items-center gap-2 rounded-sm px-2 py-1 text-left text-meta",
        active
          ? "text-fg"
          : "text-fg-secondary hover:bg-bg-hover hover:text-fg",
        "focus:outline-none focus:ring-2 focus:ring-accent-ring",
      )}
    >
      {active && (
        <span
          data-testid={`workspace-tab-${id}-bar`}
          aria-hidden="true"
          className="absolute inset-y-1 left-0 w-[2px] rounded-sm bg-accent"
        />
      )}
      <span aria-hidden="true">{emoji}</span>
      <span className="flex-1 truncate">{label}</span>
      <span
        data-testid={`workspace-tab-${id}-count`}
        className="font-mono tabular-nums text-meta text-fg-tertiary"
      >
        {count}
      </span>
    </button>
  )
}

interface GoalsAreaProps {
  vaultPath: string
  selectedRunId: string | null
  selectedDetail: RunDetail | null
  activeRunId: string | null
  onSelectRun: (run: RunLogSummary) => void
  /** Switch the detail view to the given run id (used by spawn / retry). */
  onSelectRunId: (runId: string) => void
  onBack: () => void
  onSelectPage: (slug: string) => void
  /**
   * wiki-page-reader-v1.1: when the Wiki tab edit hint or empty CTA fires
   * Workspace bumps a pending prefill signal; GoalsArea forwards it to
   * GoalsTab which opens its NewGoalModal pre-filled (or empty).
   */
  pendingNewGoalPrefill?: { text: string; nonce: number } | null
  onPendingNewGoalConsumed?: () => void
}

/**
 * Goals tab content router: shows the overview list when nothing is
 * selected, otherwise dispatches to the matching detail view based on
 * the selected run's outcome.
 */
function GoalsArea({
  vaultPath,
  selectedRunId,
  selectedDetail,
  activeRunId,
  onSelectRun,
  onSelectRunId,
  onBack,
  onSelectPage,
  pendingNewGoalPrefill,
  onPendingNewGoalConsumed,
}: GoalsAreaProps) {
  const t = useT()
  if (selectedRunId === null) {
    return (
      <GoalsTab
        vaultPath={vaultPath}
        onSelectRun={onSelectRun}
        onSpawnedRun={onSelectRunId}
        pendingNewGoalPrefill={pendingNewGoalPrefill ?? null}
        onPendingNewGoalConsumed={onPendingNewGoalConsumed}
      />
    )
  }
  // Running detail: driven by useGoalsStore.activeRun (live buffer).
  if (activeRunId === selectedRunId) {
    return <RunDetailRunning onBack={onBack} />
  }
  if (!selectedDetail) {
    return (
      <div className="flex h-full items-center justify-center text-fg-tertiary">
        {t("workspace.runDetail.loading")}
      </div>
    )
  }
  switch (selectedDetail.summary.outcome) {
    case "succeeded":
      return (
        <RunDetailDone
          detail={selectedDetail}
          onBack={onBack}
          onSelectPage={onSelectPage}
        />
      )
    case "cancelled":
    case "failed":
    case "interrupted":
      return (
        <RunDetailInterrupted
          detail={selectedDetail}
          vaultPath={vaultPath}
          onBack={onBack}
          onRetrySpawned={onSelectRunId}
        />
      )
    default:
      return null
  }
}
