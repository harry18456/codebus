import { useEffect, useState } from "react"

import { useChatStore } from "@/store/chat"

/**
 * 5-second undo affordance for the `+ New chat` reset trigger.
 *
 * Earlier iteration rendered as `position: absolute` overlay at the
 * bottom of the expanded panel, which occluded the `ChatInput` and made
 * the affordance hard to notice. Manual UX verification flipped the
 * design: render as a high-contrast, full-width banner row inside the
 * layout flow (between header-slot and body-slot of `ChatWidget`) so
 * the bar pushes the transcript down ~32px without ever overlapping the
 * input region. A live countdown (`(Ns to undo)`) gives visual urgency
 * since the snapshot is gc'd silently after the 5s window.
 *
 * Visibility is driven *entirely* by the store's undo buffer:
 *  - `useChatStore.newSession()` populates `lastSessionId` /
 *    `lastTranscript` and schedules a 5s gc timer that nulls them back
 *    out.
 *  - This component renders iff either snapshot field is non-null; when
 *    the gc timer fires (or the user clicks Undo) the snapshot clears
 *    and React re-renders the component to `null` — that *is* the
 *    "fade out after 5s" behavior. The countdown below is presentation
 *    only; it does NOT own the gc clock.
 *
 * Copy is hard-coded English; task 7.2 left i18n key wiring to a later
 * polish pass (`chat.toast.startedNewChat` / `chat.toast.undo`).
 */
export function ChatUndoToast() {
  const lastTranscript = useChatStore((s) => s.lastTranscript)
  const lastSessionId = useChatStore((s) => s.lastSessionId)
  const undoNewSession = useChatStore((s) => s.undoNewSession)

  const visible = lastTranscript !== null || lastSessionId !== null

  // Countdown is presentation only — the store owns the 5s gc clock and
  // this counter SHALL never drive the snapshot clear. Reset to 5 every
  // time the buffer flips from empty → populated, decrement once per
  // second, clamp at 0 so a slow render does not show a negative number
  // before the store clears the snapshot.
  const [remaining, setRemaining] = useState(5)
  useEffect(() => {
    if (!visible) return
    setRemaining(5)
    const interval = window.setInterval(() => {
      setRemaining((r) => Math.max(0, r - 1))
    }, 1000)
    return () => window.clearInterval(interval)
  }, [visible])

  if (!visible) return null

  return (
    <div
      data-testid="chat-undo-toast"
      role="status"
      aria-live="polite"
      className="flex flex-none items-center justify-between gap-2 border-b border-accent/40 bg-accent/10 px-3 py-2 text-xs text-fg"
    >
      <span className="flex items-center gap-2">
        <span className="font-medium">🆕 New chat started</span>
        <span
          data-testid="chat-undo-countdown"
          className="font-mono text-[11px] text-fg-tertiary"
        >
          ({remaining}s to undo)
        </span>
      </span>
      <button
        type="button"
        onClick={() => undoNewSession()}
        className="shrink-0 rounded-md border border-accent/60 bg-bg-raised px-3 py-1 text-xs font-medium text-accent hover:bg-accent/20 focus:outline-none focus-visible:ring-2 focus-visible:ring-accent-ring"
      >
        Undo
      </button>
    </div>
  )
}
