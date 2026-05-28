import { create } from "zustand"

import {
  type AddVaultMode,
  type VaultEntry,
  addVault as addVaultIpc,
  isAppError,
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
   * are sub-100ms. The Lobby overlay only renders when this is `true` OR
   * `initFailedError` is set (so the overlay can render its failure UI
   * with retry, per the LoadingOverlay Live Progress spec).
   */
  initInProgress: boolean
  /**
   * Localized init failure (init-heavy modes only; `just_bind` and the
   * detection-dialog `invalid:mode` signal do NOT populate this).
   * Cleared on the next `addVault` call (including retry) and on success.
   */
  initFailedError: LocalizedError | null
  /**
   * Last `addVault` invocation args, used by the LoadingOverlay retry
   * button to re-dispatch the same path + mode after a failure.
   */
  lastAddVaultArgs: { path: string; mode: AddVaultMode } | null
  error: LocalizedError | null
  loadVaults: () => Promise<void>
  addVault: (path: string, mode?: AddVaultMode) => Promise<VaultEntry>
  removeVault: (path: string) => Promise<void>
  markMissing: (path: string) => void
  clearError: () => void
  clearInitFailure: () => void
}

export const useVaultsStore = create<VaultsState>((set, get) => ({
  vaults: [],
  loading: false,
  initInProgress: false,
  initFailedError: null,
  lastAddVaultArgs: null,
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
    set({
      loading: true,
      initInProgress: heavy,
      initFailedError: null,
      lastAddVaultArgs: heavy ? { path, mode } : null,
      error: null,
    })
    try {
      const entry = await addVaultIpc(path, { mode })
      set((state) => ({
        vaults: [...state.vaults, entry],
        loading: false,
        initInProgress: false,
        initFailedError: null,
        lastAddVaultArgs: null,
      }))
      return entry
    } catch (err) {
      const localized = toLocalizedError(err)
      // `invalid:mode` is the detection-dialog signal (NOT an init failure)
      // — App.tsx routes it to <DetectionDialog>, so the overlay MUST drop.
      const isDetectionSignal =
        isAppError(err) && err.kind === "invalid" && err.field === "mode"
      if (heavy && !isDetectionSignal) {
        // Real init failure: keep overlay mounted so LoadingOverlay can
        // render its amber-warm failure UI + retry button.
        set({ loading: false, initFailedError: localized, error: localized })
      } else {
        set({
          loading: false,
          initInProgress: false,
          initFailedError: null,
          lastAddVaultArgs: null,
          error: localized,
        })
      }
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

  clearInitFailure() {
    set({ initFailedError: null, initInProgress: false, lastAddVaultArgs: null })
  },
}))

