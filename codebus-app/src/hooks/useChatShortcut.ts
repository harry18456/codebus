import { useEffect } from "react"

import { useChatStore } from "@/store/chat"

/**
 * Bind the Cmd+K (macOS) / Ctrl+K (Windows/Linux) shortcut to toggle the
 * chat widget's expanded state. The hook is intentionally Workspace-only:
 * Lobby never imports this hook, so the keydown listener is never
 * registered there (spec scenario "Shortcut inactive in Lobby").
 *
 * On match, the event's default action is prevented and
 * `useChatStore.toggleExpanded()` is invoked.
 */
export function useChatShortcut() {
  useEffect(() => {
    function handler(event: KeyboardEvent) {
      if (event.key !== "k" && event.key !== "K") return
      if (!(event.metaKey || event.ctrlKey)) return
      event.preventDefault()
      useChatStore.getState().toggleExpanded()
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [])
}
