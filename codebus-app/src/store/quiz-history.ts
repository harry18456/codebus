import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { create } from "zustand"

import { listQuizAttempts, type QuizAttemptMeta } from "@/lib/ipc"

/**
 * Quiz history state — provides sidebar `Quiz` nav row with a
 * store-driven `attempts.length` count without forcing QuizTab to
 * surface its component-local attempts state via prop drilling.
 *
 * Spec: app-workspace § Workspace Sidebar Nav Row Visual Contract
 * (Quiz count source).
 *
 * Lifecycle is owned by `Workspace`: mount calls `loadAttempts(vault)`,
 * unmount calls `reset()`. The store also subscribes to the
 * `quiz-changed` watcher channel — the same channel `QuizTab`
 * subscribes to — so on-disk attempt writes refresh the count without
 * any UI interaction. The watcher is per-vault on the Rust side
 * (Workspace.tsx mounts `start_vault_watcher`), so the only thing the
 * store needs to know to refresh is its own remembered vaultPath.
 */
export interface QuizHistoryState {
  vaultPath: string | null
  attempts: QuizAttemptMeta[]
  loading: boolean
  loadAttempts: (vaultPath: string) => Promise<void>
  reset: () => void
}

function startQuizChangedSubscription(onChanged: () => void): void {
  let unlisten: UnlistenFn | null = null
  void listen("quiz-changed", () => onChanged()).then((handle) => {
    unlisten = handle
  })
  void unlisten
}

export const useQuizHistoryStore = create<QuizHistoryState>((set, get) => {
  startQuizChangedSubscription(() => {
    const vaultPath = get().vaultPath
    if (!vaultPath) return
    void get().loadAttempts(vaultPath)
  })

  return {
    vaultPath: null,
    attempts: [],
    loading: false,

    async loadAttempts(vaultPath) {
      set({ vaultPath, loading: true })
      try {
        const attempts = await listQuizAttempts(vaultPath)
        set({ attempts, loading: false })
      } catch {
        set({ loading: false })
      }
    },

    reset() {
      set({ vaultPath: null, attempts: [], loading: false })
    },
  }
})
