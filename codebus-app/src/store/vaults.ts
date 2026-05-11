import { create } from "zustand"

import {
  type AddVaultMode,
  type VaultEntry,
  addVault as addVaultIpc,
  listVaults as listVaultsIpc,
  removeVault as removeVaultIpc,
} from "@/lib/ipc"
import { type LocalizedError, toLocalizedError } from "@/i18n/errors"

interface VaultsState {
  vaults: VaultEntry[]
  loading: boolean
  /**
   * Distinct from `loading` because vault init via `run_init` takes seconds
   * (raw_sync, nested git init, skill bundles, …) while plain list/remove
   * are sub-100ms. The Lobby overlay only renders when this is `true`.
   */
  initInProgress: boolean
  error: LocalizedError | null
  loadVaults: () => Promise<void>
  addVault: (path: string, mode?: AddVaultMode) => Promise<VaultEntry>
  removeVault: (path: string) => Promise<void>
  markMissing: (path: string) => void
  clearError: () => void
}

export const useVaultsStore = create<VaultsState>((set, get) => ({
  vaults: [],
  loading: false,
  initInProgress: false,
  error: null,

  async loadVaults() {
    set({ loading: true, error: null })
    try {
      const list = await listVaultsIpc()
      set({ vaults: list, loading: false })
    } catch (err) {
      set({ loading: false, error: toLocalizedError(err) })
    }
  },

  async addVault(path, mode = "detect") {
    // Modes that actually trigger run_init under the hood — these are the
    // ones worth surfacing as "Boarding…" in the UI. `just_bind` skips init.
    const heavy = mode === "detect" || mode === "re_init"
    set({ loading: true, initInProgress: heavy, error: null })
    try {
      const entry = await addVaultIpc(path, { mode })
      set((state) => ({
        vaults: [...state.vaults, entry],
        loading: false,
        initInProgress: false,
      }))
      return entry
    } catch (err) {
      set({ loading: false, initInProgress: false, error: toLocalizedError(err) })
      throw err
    }
  },

  async removeVault(path) {
    const previous = get().vaults
    set((state) => ({ vaults: state.vaults.filter((v) => v.path !== path) }))
    try {
      await removeVaultIpc(path)
    } catch (err) {
      // Roll back on failure so the UI stays consistent with backend.
      set({ vaults: previous, error: toLocalizedError(err) })
      throw err
    }
  },

  markMissing(path) {
    set((state) => ({
      vaults: state.vaults.map((v) =>
        v.path === path ? { ...v, is_missing: true } : v,
      ),
    }))
  },

  clearError() {
    set({ error: null })
  },
}))

