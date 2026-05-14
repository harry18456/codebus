import { create } from "zustand"

import type { VaultEntry } from "@/lib/ipc"

/**
 * Workspace route state.
 *
 * v3-app-workspace-goal renames `workspace-stub` to `workspace` so the
 * shipped Workspace shell mounts in the same slot the stub used to
 * occupy. The vault payload shape is unchanged; only the discriminator
 * string moves. App.tsx narrows on `kind === "lobby"` so the rename
 * does not touch the routing branch itself — only TypeScript callers
 * that pattern-match the discriminator string need updating.
 */
export type RouteState =
  | { kind: "lobby" }
  | { kind: "workspace"; vault: VaultEntry }

interface RouteStore {
  route: RouteState
  open: (vault: VaultEntry) => void
  back: () => void
}

export const useRouteStore = create<RouteStore>((set) => ({
  route: { kind: "lobby" },
  open(vault) {
    set({ route: { kind: "workspace", vault } })
  },
  back() {
    set({ route: { kind: "lobby" } })
  },
}))
