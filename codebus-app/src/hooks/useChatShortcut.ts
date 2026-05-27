import { useEffect } from "react"

import { useChatStore } from "@/store/chat"

/**
 * Bind the Cmd+K (macOS) / Ctrl+K (Windows/Linux) shortcut to open the
 * chat widget in `modal` mode (per spec "Chat Widget Toggle Shortcut").
 * The hook is intentionally Workspace-only: Lobby never imports this
 * hook, so the keydown listener is never registered there.
 *
 * `openModal()` snapshots the current mode (`bubble` or `floating`) into
 * `modalReturnMode` so Esc / backdrop click can restore it. When the
 * widget is already in modal mode the action is a no-op — repeated ⌘K
 * presses do NOT overwrite the existing snapshot.
 */
export function useChatShortcut() {
  useEffect(() => {
    function handler(event: KeyboardEvent) {
      if (event.key !== "k" && event.key !== "K") return
      if (!(event.metaKey || event.ctrlKey)) return
      event.preventDefault()
      useChatStore.getState().openModal()
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [])
}
