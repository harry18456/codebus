import { useEffect } from "react"

import { useRouteStore } from "@/store/route"

/**
 * Bind the Cmd+N (macOS) / Ctrl+N (other) shortcut to `onFire` while the
 * application is in the Lobby route. The handler intentionally does
 * nothing in any other route (spec scenario "keyboard shortcut … in
 * Workspace stub does not trigger").
 */
export function useNewVaultShortcut(onFire: () => void) {
  useEffect(() => {
    function handler(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== "n") return
      if (!(event.metaKey || event.ctrlKey)) return
      const route = useRouteStore.getState().route
      if (route.kind !== "lobby") return
      event.preventDefault()
      onFire()
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [onFire])
}
