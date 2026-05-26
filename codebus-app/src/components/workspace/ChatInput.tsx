import { useState, type KeyboardEvent } from "react"

import { useChatStore } from "@/store/chat"
import { useT } from "@/i18n/useT"
import { useLocale } from "@/hooks/useLocale"

/**
 * ChatInput renders the textarea + Send/Stop action for the workspace chat
 * widget (task 6.1 of the v3-app-chat-cmdk change).
 *
 * Behavior:
 * - When `activeTurn` is null (idle): textarea is enabled, the Send button is
 *   shown and triggers `useChatStore.spawnTurn(vaultPath, text)`. Enter
 *   without Shift submits; Shift+Enter inserts a newline (default textarea
 *   behavior, no preventDefault).
 * - When `activeTurn` is non-null (a turn is streaming): the textarea is
 *   disabled and the Send button is replaced by a ⏹ Stop button that calls
 *   `useChatStore.cancelActiveTurn()`.
 */
export interface ChatInputProps {
  vaultPath: string
}

export function ChatInput({ vaultPath }: ChatInputProps) {
  const t = useT()
  const locale = useLocale()
  const [text, setText] = useState("")
  const activeTurn = useChatStore((s) => s.activeTurn)
  const spawnTurn = useChatStore((s) => s.spawnTurn)
  const cancelActiveTurn = useChatStore((s) => s.cancelActiveTurn)

  const isActive = activeTurn !== null

  function handleSend() {
    const trimmed = text.trim()
    if (!trimmed || isActive) return
    setText("")
    void spawnTurn(vaultPath, trimmed)
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div
      data-testid="chat-input-area"
      className="flex items-end gap-2 p-2"
    >
      <textarea
        data-testid="chat-input-textarea"
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        disabled={isActive}
        placeholder={
          locale === "zh"
            ? t("chat.placeholder.tw")
            : t("chat.placeholder.en")
        }
        rows={2}
        className="flex-1 resize-none rounded-md border border-border bg-bg px-2 py-1 text-xs text-fg placeholder:text-fg-tertiary focus:outline-none focus:ring-2 focus:ring-accent-ring disabled:cursor-not-allowed disabled:opacity-60"
      />
      {isActive ? (
        <button
          type="button"
          data-testid="chat-input-stop"
          onClick={() => void cancelActiveTurn()}
          className="shrink-0 rounded-md border border-error/40 bg-error/10 px-3 py-1 text-xs text-error hover:bg-error/20 focus:outline-none focus:ring-2 focus:ring-accent-ring"
        >
          {t("chat.button.stop")}
        </button>
      ) : (
        <button
          type="button"
          data-testid="chat-input-send"
          onClick={handleSend}
          disabled={!text.trim()}
          className="shrink-0 rounded-md border border-accent/40 bg-accent/20 px-3 py-1 text-xs text-accent hover:bg-accent/30 focus:outline-none focus:ring-2 focus:ring-accent-ring disabled:cursor-not-allowed disabled:opacity-50"
        >
          {t("chat.button.send")}
        </button>
      )}
    </div>
  )
}
