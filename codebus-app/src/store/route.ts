import { create } from "zustand"

import type { VaultEntry } from "@/lib/ipc"

export type RouteState =
  | { kind: "lobby" }
  | { kind: "workspace-stub"; vault: VaultEntry }

interface RouteStore {
  route: RouteState
  open: (vault: VaultEntry) => void
  back: () => void
}

export const useRouteStore = create<RouteStore>((set) => ({
  route: { kind: "lobby" },
  open(vault) {
    set({ route: { kind: "workspace-stub", vault } })
  },
  back() {
    set({ route: { kind: "lobby" } })
  },
}))
