import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { create } from "zustand"

import {
  listWikiPages,
  readWikiPage,
  type GoalTerminalPayload,
  type WikiPageMeta,
} from "@/lib/ipc"

/**
 * Wiki state.
 *
 * `pages` is the in-memory page index, keyed by slug. Loaded once at
 * Workspace mount via `listPages` so wikilink resolution can run
 * entirely client-side (per `app-workspace § Wikilink Resolution and
 * Click Behavior`'s "client-side resolution" decision); `read_wiki_page`
 * is only invoked when the navigation lands on a resolvable target.
 *
 * `_bodyCache` keeps already-fetched bodies so re-visiting a page
 * (e.g., via wikilink round trips) does not re-fire `read_wiki_page`.
 *
 * The store also subscribes to the `goal-terminal` Tauri event channel
 * so any goal completion automatically re-runs `listPages` against the
 * last-known vault path — without this the Wiki tab still shows the
 * pre-goal page index even though new pages already exist on disk.
 */
interface WikiState {
  pages: Record<string, WikiPageMeta>
  currentPath: string | null
  body: string | null
  listPages: (vaultPath: string) => Promise<void>
  loadPage: (vaultPath: string, slug: string) => Promise<void>
  reset: () => void
  /** Last vault path passed to `listPages`. Read by `_onTerminal`. */
  _currentVaultPath: string | null
  /** Internal slot exposed for tests; components SHALL NOT call. */
  _bodyCache: Record<string, string>
  /** Internal slot exposed for tests; components SHALL NOT call. */
  _onTerminal: (payload: GoalTerminalPayload) => void
}

function startTerminalSubscription(
  onTerminal: (payload: GoalTerminalPayload) => void,
): void {
  let unlisten: UnlistenFn | null = null
  void listen<GoalTerminalPayload>("goal-terminal", (event) => {
    onTerminal(event.payload)
  }).then((handle) => {
    unlisten = handle
  })
  void unlisten
}

export const useWikiStore = create<WikiState>((set, get) => {
  startTerminalSubscription((payload) => get()._onTerminal(payload))

  return {
    pages: {},
    currentPath: null,
    body: null,
    _bodyCache: {},
    _currentVaultPath: null,

    async listPages(vaultPath) {
      set({ _currentVaultPath: vaultPath })
      const meta = await listWikiPages(vaultPath)
      const index: Record<string, WikiPageMeta> = {}
      for (const page of meta) {
        // Slug collision: last write wins, matching the design
        // "duplicate slug behavior" risk note.
        index[page.slug] = page
      }
      set({ pages: index })
    },

    async loadPage(vaultPath, slug) {
      const cached = get()._bodyCache[slug]
      if (cached !== undefined) {
        set({ currentPath: slug, body: cached })
        return
      }
      const body = await readWikiPage(vaultPath, slug)
      set((state) => ({
        currentPath: slug,
        body,
        _bodyCache: { ...state._bodyCache, [slug]: body },
      }))
    },

    reset() {
      set({
        pages: {},
        currentPath: null,
        body: null,
        _bodyCache: {},
        _currentVaultPath: null,
      })
    },

    _onTerminal(_payload) {
      // Refresh the page index whenever a goal run terminates so
      // newly-created wiki pages show up immediately in the file tree
      // / wikilink resolver. Also invalidate the body cache for the
      // currently-open page since the goal may have rewritten it
      // (read_wiki_page will be re-fired on next visit).
      const state = get()
      const vaultPath = state._currentVaultPath
      if (!vaultPath) return
      const currentPath = state.currentPath
      set((s) => {
        if (currentPath === null) return { _bodyCache: {} }
        const { [currentPath]: _drop, ...rest } = s._bodyCache
        return { _bodyCache: rest }
      })
      void get().listPages(vaultPath)
    },
  }
})
