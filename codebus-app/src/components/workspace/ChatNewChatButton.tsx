import { useChatStore } from "@/store/chat"

/**
 * Header-slot "+ New chat" button. Implements the manual reset trigger from
 * the `Chat Session Lifecycle and Reset Triggers` spec:
 *
 *  - Click → `useChatStore.newSession()` clears the live session and stashes
 *    the previous `{ sessionId, turns }` into the undo buffer for 5s.
 *  - The matching `ChatUndoToast` reads that buffer and renders the inline
 *    confirmation + Undo affordance; this button only owns the *trigger*,
 *    keeping the file single-purpose.
 *
 * Copy is hard-coded English on purpose; task 7.2 swaps it for the
 * `chat.session.newChat.label` i18n key.
 */
export function ChatNewChatButton() {
  const newSession = useChatStore((s) => s.newSession)
  return (
    <button
      type="button"
      data-testid="chat-new-chat-button"
      onClick={() => newSession()}
      className="mr-auto rounded-md border border-border px-2 py-0.5 text-[11px] text-fg-secondary hover:bg-bg-hover hover:text-fg focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
    >
      + New chat
    </button>
  )
}
