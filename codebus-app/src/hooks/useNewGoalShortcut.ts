import { useEffect } from "react"

/**
 * Bind the bare `N` (no modifier) shortcut to `onFire` while this hook
 * is mounted. Mounted only inside `GoalsTab` so it auto-scopes to "Goals
 * tab is the active tab" (other tabs unmount this component per the
 * Workspace tab-switch re-mount contract).
 *
 * The chip `<kbd>N</kbd>` next to `+ New goal` in the content header row
 * (Phase 4C) labels exactly this binding. Without it the chip would be a
 * visual lie. Discovered via CDP smoke during apply.
 *
 * Modifiers are explicitly excluded — Cmd+N / Ctrl+N belongs to
 * `useNewVaultShortcut` in the Lobby. Typing inside an input / textarea /
 * contenteditable target is also excluded so users can type the letter
 * "n" inside the New Goal modal textarea without re-firing the shortcut.
 */
export function useNewGoalShortcut(onFire: () => void) {
  useEffect(() => {
    function handler(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== "n") return
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
        return
      }
      const target = event.target as HTMLElement | null
      if (target) {
        const tag = target.tagName
        if (tag === "INPUT" || tag === "TEXTAREA") return
        if (target.isContentEditable) return
      }
      event.preventDefault()
      onFire()
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [onFire])
}
