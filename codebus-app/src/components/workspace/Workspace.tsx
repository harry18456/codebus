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
import { useRouteStore } from "@/store/route"
import { useVaultsStore } from "@/store/vaults"
import { useWikiStore } from "@/store/wiki"
import { useChatShortcut } from "@/hooks/useChatShortcut"

import { ChatWidget } from "./ChatWidget"
import { GoalsTab } from "./GoalsTab"
import { QuizTab } from "./QuizTab"
import { RunDetailCancelled, RunDetailInterrupted } from "./RunDetailCancelled"
import { RunDetailDone } from "./RunDetailDone"
import { RunDetailRunning } from "./RunDetailRunning"
import { WikiTab } from "./WikiTab"

interface WorkspaceProps {
  vault: VaultEntry
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
export function Workspace({ vault }: WorkspaceProps) {
  const back = useRouteStore((s) => s.back)
  const loadVaults = useVaultsStore((s) => s.loadVaults)
  const refreshRuns = useGoalsStore((s) => s.refreshRuns)
  const goalsReset = useGoalsStore((s) => s.reset)
  const activeRun = useGoalsStore((s) => s.activeRun)
  const listPages = useWikiStore((s) => s.listPages)
  const loadPage = useWikiStore((s) => s.loadPage)
  const wikiReset = useWikiStore((s) => s.reset)

  const [activeTab, setActiveTab] = useState<TabId>("goals")
  // task 5.3 — when set (via wiki preview [Quiz me on this]), the Quiz
  // tab consumes it to start the Page flow (skip planning).
  const [pendingQuizPage, setPendingQuizPage] = useState<string | null>(null)
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null)
  const [selectedDetail, setSelectedDetail] = useState<RunDetail | null>(null)

  // Bind Cmd+K / Ctrl+K to toggle the chat widget. The hook scopes the
  // window listener to Workspace mount/unmount so the shortcut is inert in
  // the Lobby — see `useChatShortcut` for the spec scenario this enforces.
  useChatShortcut()

  useEffect(() => {
    void refreshRuns(vault.path).catch(() => {})
    void listPages(vault.path).catch(() => {})
    const vaultPath = vault.path
    return () => {
      goalsReset()
      wikiReset()
      // Drop chat session + transcript + token tally + pending promote
      // suggestion when the user leaves the vault. Widget UI prefs
      // (expanded, width, height, onboardedVaults) intentionally survive
      // per spec's `Session Reset Behaviors` table.
      useChatStore.getState().resetForVault(vaultPath)
    }
  }, [vault.path, refreshRuns, listPages, goalsReset, wikiReset])

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
          className="text-left text-[12px] text-fg-tertiary hover:text-fg focus:outline-none focus:ring-2 focus:ring-accent-ring"
        >
          ← Back to Lobby
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
            title={`${vault.path}\n\nClick to open in file explorer`}
            className="block w-full truncate text-left font-mono text-[11px] text-fg-tertiary hover:text-accent hover:underline focus:outline-none focus:ring-2 focus:ring-accent-ring"
          >
            {vault.path}
          </button>
        </div>
        <nav className="mt-4 flex flex-col gap-1">
          <TabButton
            id="goals"
            label="Goals"
            activeTab={activeTab}
            onSelect={(t) => {
              setActiveTab(t)
              setSelectedRunId(null)
              setSelectedDetail(null)
            }}
          />
          <TabButton
            id="wiki"
            label="Wiki"
            activeTab={activeTab}
            onSelect={(t) => setActiveTab(t)}
          />
          <TabButton
            id="quiz"
            label="Quiz"
            activeTab={activeTab}
            onSelect={(t) => setActiveTab(t)}
          />
        </nav>
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
          />
        )}
        {activeTab === "wiki" && (
          <WikiTab
            vaultPath={vault.path}
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
          />
        )}
      </section>
      {/*
       * ChatWidget lives at Workspace level (sibling of the tab-bound
       * `<section>`) so it survives tab switches — collapsing the widget,
       * opening Wiki, then returning to Goals keeps the transcript,
       * sessionId, and `expanded` state intact. The widget pins itself via
       * fixed position so this DOM placement does not affect layout, but
       * the React subtree must NOT live inside an `activeTab` conditional
       * or it would unmount and lose its state.
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
  label: string
  activeTab: TabId
  onSelect: (tab: TabId) => void
}

function TabButton({ id, label, activeTab, onSelect }: TabButtonProps) {
  const active = activeTab === id
  return (
    <button
      type="button"
      data-testid={`workspace-tab-${id}`}
      data-active={active}
      onClick={() => onSelect(id)}
      className={cn(
        "rounded-sm px-2 py-1 text-left text-[12px]",
        active
          ? "bg-accent/20 text-accent"
          : "text-fg-secondary hover:bg-bg-hover hover:text-fg",
        "focus:outline-none focus:ring-2 focus:ring-accent-ring",
      )}
    >
      {label}
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
}: GoalsAreaProps) {
  if (selectedRunId === null) {
    return (
      <GoalsTab
        vaultPath={vaultPath}
        onSelectRun={onSelectRun}
        onSpawnedRun={onSelectRunId}
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
        Loading…
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
      return (
        <RunDetailCancelled
          detail={selectedDetail}
          vaultPath={vaultPath}
          onBack={onBack}
          onRetrySpawned={onSelectRunId}
        />
      )
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
