import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { create } from "zustand"

import {
  cancelChatTurn as cancelChatTurnIpc,
  spawnChatTurn as spawnChatTurnIpc,
  spawnGoal as spawnGoalIpc,
  type ChatStreamPayload,
  type ChatTerminalPayload,
  type ChatTurnRunId,
  type TokenUsage,
  type VerbEvent,
} from "@/lib/ipc"

/**
 * One finalized chat turn. Mirrors `useGoalsStore.activeRun.events` per-run,
 * but stored per-turn so the transcript can render alternating user + assistant
 * blocks. `events` keeps the original arrival order so projection helpers
 * (assistant text concatenation, tool_use one-liners, promote-marker stripping)
 * can replay the stream deterministically.
 */
export interface ChatTurn {
  userText: string
  events: VerbEvent[]
  startedAt: string
  finishedAt: string
}

/**
 * Volatile state for the turn currently being streamed. `runId` is briefly
 * `null` between `spawnTurn` call-site and the `spawnChatTurn` IPC resolving
 * (cannot filter `chat-stream` events until the run id is known, so any
 * events received during that gap are dropped — same race window as
 * `useGoalsStore.spawnGoal`).
 *
 * `cancelling` is a frontend-only optimistic flag for the `⏹ Cancelling…`
 * button state; cleared when the terminal event arrives and the turn
 * finalizes into `turns`.
 */
export interface ChatTurnLive {
  vaultPath: string
  userText: string
  runId: ChatTurnRunId | null
  events: VerbEvent[]
  cancelling: boolean
  startedAt: string
}

/**
 * Pending promote suggestion attached to the most recent assistant turn.
 * `turnIndex` points into `turns` so the inline pill renders at the correct
 * message even after subsequent turns are appended.
 */
export interface PromoteSuggestion {
  reason: string
  turnIndex: number
}

interface ChatStore {
  // Session 狀態
  sessionId: string | null
  turns: ChatTurn[]
  activeTurn: ChatTurnLive | null
  tokensTotal: TokenUsage
  promoteSuggestion: PromoteSuggestion | null

  // Widget UI 狀態（per-vault memory-only except onboardedVaults）
  expanded: boolean
  width: number
  height: number
  onboardedVaults: Set<string>

  // Undo 緩衝
  lastTranscript: ChatTurn[] | null
  lastSessionId: string | null

  // Actions
  spawnTurn: (vaultPath: string, text: string) => Promise<void>
  cancelActiveTurn: () => Promise<void>
  newSession: () => void
  undoNewSession: () => void
  toggleExpanded: () => void
  setSize: (width: number, height: number) => void
  dismissPromoteSuggestion: () => void
  acceptPromoteSuggestion: (vaultPath: string) => Promise<string>
  resetForVault: (vaultPath: string) => void
  markOnboarded: (vaultPath: string) => void

  /**
   * Internal slot exposed for tests so the `chat-stream` listener can
   * drive events without going through the Tauri event bus. Components
   * SHALL NOT call this directly.
   */
  _onStreamEvent: (payload: ChatStreamPayload) => void
  /** Internal slot for tests; same caveat as `_onStreamEvent`. */
  _onTerminal: (payload: ChatTerminalPayload) => void
}

const DEFAULT_TOKENS: TokenUsage = { input_tokens: 0, output_tokens: 0 }
const DEFAULT_WIDTH_REM = 22
const DEFAULT_HEIGHT_REM = 32
const UNDO_WINDOW_MS = 5000
const ONBOARDED_KEY_PREFIX = "codebus-chat-onboarded-"

/**
 * Module-level handle for the pending undo gc timer. Kept outside the store
 * because it is purely a side-effect handle (not UI state) and must be
 * clearable from any of: undoNewSession (manual restore), resetForVault
 * (vault switch), or a subsequent newSession call (overlapping toasts).
 */
let undoGcTimer: ReturnType<typeof setTimeout> | null = null

function clearUndoGcTimer(): void {
  if (undoGcTimer !== null) {
    clearTimeout(undoGcTimer)
    undoGcTimer = null
  }
}

/**
 * Stable, sync-safe key suffix for the per-vault onboarding flag.
 *
 * The spec describes the key pattern as `codebus-chat-onboarded-<sha1(vault_path)>`
 * for namespacing — the hash algorithm is implementation choice. We pick a
 * base64-style fold over the vault path rather than `crypto.subtle.digest`
 * (which is async) so that the localStorage read remains synchronous at the
 * point the widget needs to decide whether to render the onboarding hint.
 * `btoa` does not accept non-Latin1 input, so the path is first URL-encoded
 * to a Latin1-safe form; the result is then stripped to `[A-Za-z0-9]` so the
 * key remains a valid localStorage key regardless of vault path content.
 *
 * Onboarding flag is a non-secret presence bit, so the lack of cryptographic
 * collision-resistance vs. real SHA-1 is acceptable — at worst two distinct
 * vault paths sharing a suffix would skip each other's hint once.
 */
function vaultKey(vaultPath: string): string {
  // encodeURIComponent → Latin1-safe; btoa → fold; strip non-alphanum.
  const encoded =
    typeof btoa === "function"
      ? btoa(encodeURIComponent(vaultPath))
      : encodeURIComponent(vaultPath)
  return encoded.replace(/[^A-Za-z0-9]/g, "")
}

function readOnboardedVaults(): Set<string> {
  // We only persist the per-vault-key flag value ("1"); the in-memory
  // `onboardedVaults` Set holds the original vault paths so callers can
  // probe it by path. We cannot enumerate paths from the flag alone, so the
  // Set is seeded empty on app reload and re-populated as `markOnboarded`
  // is called — same memory-only semantics design specifies, plus an extra
  // localStorage gate the widget consults at expand time.
  return new Set<string>()
}

/**
 * Public read-side helper for the persisted onboarding flag. The Chat Widget
 * consults this at expand-time to decide whether to render
 * `chat-onboarding-hint` (first expand per vault) or the placeholder
 * (subsequent expands). Kept here so the localStorage key derivation stays
 * encapsulated alongside `markOnboarded`'s writer.
 */
export function readOnboardedFlag(vaultPath: string): boolean {
  try {
    if (typeof localStorage === "undefined") return false
    return localStorage.getItem(ONBOARDED_KEY_PREFIX + vaultKey(vaultPath)) === "1"
  } catch {
    return false
  }
}

function writeOnboardedFlag(vaultPath: string): void {
  try {
    if (typeof localStorage === "undefined") return
    localStorage.setItem(ONBOARDED_KEY_PREFIX + vaultKey(vaultPath), "1")
  } catch {
    // localStorage can throw in private-mode / quota-exceeded; the
    // onboarding flag is best-effort UX so we swallow.
  }
}

/**
 * Subscribe to the `chat-stream` + `chat-terminal` Tauri event channels
 * exactly once per store instance. Mirrors `useGoalsStore`'s subscription
 * pattern — handles are captured for parity with future teardown paths;
 * we deliberately do not call them during the app lifetime since the
 * widget lives for the whole app session.
 */
function startChatStreamSubscription(
  onEvent: (payload: ChatStreamPayload) => void,
  onTerminal: (payload: ChatTerminalPayload) => void,
): void {
  let unlistenStream: UnlistenFn | null = null
  let unlistenTerminal: UnlistenFn | null = null
  void listen<ChatStreamPayload>("chat-stream", (event) => {
    onEvent(event.payload)
  }).then((handle) => {
    unlistenStream = handle
  })
  void listen<ChatTerminalPayload>("chat-terminal", (event) => {
    onTerminal(event.payload)
  }).then((handle) => {
    unlistenTerminal = handle
  })
  void unlistenStream
  void unlistenTerminal
}

export const useChatStore = create<ChatStore>((set, get) => {
  startChatStreamSubscription(
    (payload) => get()._onStreamEvent(payload),
    (payload) => get()._onTerminal(payload),
  )

  return {
    sessionId: null,
    turns: [],
    activeTurn: null,
    tokensTotal: { ...DEFAULT_TOKENS },
    promoteSuggestion: null,

    expanded: false,
    width: DEFAULT_WIDTH_REM,
    height: DEFAULT_HEIGHT_REM,
    onboardedVaults: readOnboardedVaults(),

    lastTranscript: null,
    lastSessionId: null,

    async spawnTurn(vaultPath, text) {
      const startedAt = new Date().toISOString()
      const { sessionId } = get()
      // Optimistic placeholder so the transcript can render the user prompt
      // + a streaming buffer before the IPC resolves; the runId is patched
      // in once the spawn returns.
      set({
        activeTurn: {
          vaultPath,
          userText: text,
          runId: null,
          events: [],
          cancelling: false,
          startedAt,
        },
      })
      const runId = await spawnChatTurnIpc(vaultPath, text, sessionId)
      set((state) =>
        state.activeTurn
          ? { activeTurn: { ...state.activeTurn, runId } }
          : {},
      )
    },

    async cancelActiveTurn() {
      const active = get().activeTurn
      if (!active || active.runId === null) return
      // Flip the local cancelling flag synchronously so the button
      // transitions to its disabled `Cancelling…` state immediately.
      set((state) =>
        state.activeTurn
          ? { activeTurn: { ...state.activeTurn, cancelling: true } }
          : {},
      )
      await cancelChatTurnIpc(active.runId)
    },

    newSession() {
      const { sessionId, turns } = get()
      // Stash the soon-to-be-cleared session into the undo buffer for the
      // 5s window. Replace any existing pending gc so overlapping
      // newSession clicks don't leave a stale gc timer pointing at a
      // mismatched buffer.
      clearUndoGcTimer()
      set({
        lastSessionId: sessionId,
        lastTranscript: turns,
        sessionId: null,
        turns: [],
        activeTurn: null,
        tokensTotal: { ...DEFAULT_TOKENS },
        promoteSuggestion: null,
      })
      undoGcTimer = setTimeout(() => {
        undoGcTimer = null
        set({ lastSessionId: null, lastTranscript: null })
      }, UNDO_WINDOW_MS)
    },

    undoNewSession() {
      // Restore from the undo buffer + cancel the pending gc so the
      // restored session isn't nulled out a few seconds later.
      clearUndoGcTimer()
      set((state) => ({
        sessionId: state.lastSessionId,
        turns: state.lastTranscript ?? [],
        lastSessionId: null,
        lastTranscript: null,
      }))
    },

    toggleExpanded() {
      set((state) => ({ expanded: !state.expanded }))
    },

    setSize(width, height) {
      // Clamping is the UI layer's responsibility (different aesthetics on
      // expand vs. drag-resize); store keeps the raw values.
      set({ width, height })
    },

    dismissPromoteSuggestion() {
      set({ promoteSuggestion: null })
    },

    async acceptPromoteSuggestion(vaultPath) {
      const { promoteSuggestion, turns } = get()
      if (!promoteSuggestion) {
        throw new Error("acceptPromoteSuggestion called with no pending suggestion")
      }
      const transcript = buildTranscriptDump(turns, promoteSuggestion.reason)
      try {
        const runId = await spawnGoalIpc(vaultPath, transcript)
        // On success, clear the pill + collapse the widget per spec.
        set({ promoteSuggestion: null, expanded: false })
        return runId
      } catch (error) {
        // Leave promoteSuggestion intact so the UI can render an inline
        // error + the user can retry once the active goal finishes.
        throw error
      }
    },

    resetForVault(_vaultPath) {
      // Vault switch reset trigger — drop session + transcript + undo
      // buffer + token tally + promote pill AND collapse the widget back
      // to the bubble so the next vault opens in a clean visual state.
      // Width / height / onboardedVaults survive (user resize / per-vault
      // localStorage flag carry across the lobby round-trip).
      clearUndoGcTimer()
      set({
        sessionId: null,
        turns: [],
        activeTurn: null,
        tokensTotal: { ...DEFAULT_TOKENS },
        promoteSuggestion: null,
        lastSessionId: null,
        lastTranscript: null,
        expanded: false,
      })
    },

    markOnboarded(vaultPath) {
      // Set is immutable-style updated to keep zustand subscribers firing.
      set((state) => {
        if (state.onboardedVaults.has(vaultPath)) return {}
        const next = new Set(state.onboardedVaults)
        next.add(vaultPath)
        return { onboardedVaults: next }
      })
      writeOnboardedFlag(vaultPath)
    },

    _onStreamEvent(payload) {
      set((state) => {
        if (!state.activeTurn || state.activeTurn.runId !== payload.run_id) {
          return {}
        }
        const event = payload.event
        // Accumulate usage events into the session total so the header
        // `Nk ↑` reads sum across every turn.
        let tokensTotal = state.tokensTotal
        if (event.kind === "stream" && event.data.kind === "usage") {
          const u = event.data
          tokensTotal = {
            input_tokens: tokensTotal.input_tokens + u.input_tokens,
            output_tokens: tokensTotal.output_tokens + u.output_tokens,
            cache_read_tokens:
              (tokensTotal.cache_read_tokens ?? 0) + (u.cache_read_tokens ?? 0),
            cache_write_tokens:
              (tokensTotal.cache_write_tokens ?? 0) + (u.cache_write_tokens ?? 0),
            reasoning_tokens:
              (tokensTotal.reasoning_tokens ?? 0) + (u.reasoning_tokens ?? 0),
          }
        }
        // Capture a promote suggestion as soon as the lifecycle event lands;
        // turnIndex points at the slot the active turn will occupy once it
        // finalizes (current turns.length, since the active turn appends).
        let promoteSuggestion = state.promoteSuggestion
        if (event.kind === "lifecycle" && event.data.kind === "promote_suggestion") {
          promoteSuggestion = {
            reason: event.data.reason,
            turnIndex: state.turns.length,
          }
        }
        return {
          activeTurn: {
            ...state.activeTurn,
            events: [...state.activeTurn.events, event],
          },
          tokensTotal,
          promoteSuggestion,
        }
      })
    },

    _onTerminal(payload) {
      set((state) => {
        if (!state.activeTurn || state.activeTurn.runId !== payload.run_id) {
          return {}
        }
        // Finalize the active turn into the transcript. The backend
        // captures the claude `session_id` from the runner's
        // `ChatTurnReport` and ships it via the terminal payload; if
        // `payload.session_id` is null (turn never reached init) keep
        // any previously known sessionId untouched.
        const finalized: ChatTurn = {
          userText: state.activeTurn.userText,
          events: state.activeTurn.events,
          startedAt: state.activeTurn.startedAt,
          finishedAt: new Date().toISOString(),
        }
        const nextSessionId =
          payload.session_id ?? state.sessionId
        return {
          turns: [...state.turns, finalized],
          activeTurn: null,
          sessionId: nextSessionId,
        }
      })
    },
  }
})

/**
 * Build the `spawn_goal` transcript string for a promote click. Format
 * mirrors the chat-verb CLI design so the GUI + CLI promote paths produce
 * identical goal text:
 *
 * ```text
 * Based on this conversation:
 *
 * <user>: ...
 * <assistant>: ...(text chunks concatenated)
 * ...
 *
 * Write: <reason>
 * ```
 *
 * Tool use, thoughts, and the `[CODEBUS_PROMOTE_SUGGESTION] ...` marker line
 * are stripped — only `StreamEvent::Text` chunks contribute to the
 * `<assistant>:` body.
 */
function buildTranscriptDump(turns: ChatTurn[], reason: string): string {
  const lines: string[] = ["Based on this conversation:", ""]
  for (const turn of turns) {
    lines.push(`<user>: ${turn.userText}`)
    const assistantText = turn.events
      .filter((e): e is Extract<VerbEvent, { kind: "stream" }> => e.kind === "stream")
      .map((e) => e.data)
      .filter(
        (d): d is Extract<typeof d, { kind: "thought" }> => d.kind === "thought",
      )
      .map((d) => d.text)
      .join("")
    lines.push(`<assistant>: ${assistantText}`)
  }
  lines.push("")
  lines.push(`Write: ${reason}`)
  return lines.join("\n")
}
