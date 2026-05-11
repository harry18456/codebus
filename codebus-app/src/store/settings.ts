import { create } from "zustand"

import {
  type GlobalConfig,
  loadGlobalConfig as loadGlobalConfigIpc,
  saveGlobalConfig as saveGlobalConfigIpc,
} from "@/lib/ipc"
import { type LocalizedError, toLocalizedError } from "@/i18n/errors"

interface SettingsState {
  config: GlobalConfig
  initialConfig: GlobalConfig
  dirty: boolean
  loading: boolean
  saving: boolean
  error: LocalizedError | null
  load: () => Promise<void>
  update: (patch: Partial<GlobalConfig>) => void
  reset: () => void
  save: () => Promise<void>
  clearError: () => void
}

const EMPTY_CONFIG: GlobalConfig = {}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  config: EMPTY_CONFIG,
  initialConfig: EMPTY_CONFIG,
  dirty: false,
  loading: false,
  saving: false,
  error: null,

  async load() {
    set({ loading: true, error: null })
    try {
      const cfg = await loadGlobalConfigIpc()
      set({
        config: cfg,
        initialConfig: cfg,
        dirty: false,
        loading: false,
      })
    } catch (err) {
      set({ loading: false, error: toLocalizedError(err) })
    }
  },

  update(patch) {
    const merged = mergeDeep(get().config, patch)
    set({ config: merged, dirty: true })
  },

  reset() {
    set((state) => ({ config: state.initialConfig, dirty: false, error: null }))
  },

  async save() {
    set({ saving: true, error: null })
    try {
      const cfg = get().config
      await saveGlobalConfigIpc(cfg)
      set({ initialConfig: cfg, dirty: false, saving: false })
    } catch (err) {
      set({ saving: false, error: toLocalizedError(err) })
      throw err
    }
  },

  clearError() {
    set({ error: null })
  },
}))


function mergeDeep<T extends Record<string, unknown>>(target: T, patch: Partial<T>): T {
  const out: Record<string, unknown> = { ...target }
  for (const key of Object.keys(patch)) {
    const value = (patch as Record<string, unknown>)[key]
    const existing = out[key]
    if (
      value &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      existing &&
      typeof existing === "object" &&
      !Array.isArray(existing)
    ) {
      out[key] = mergeDeep(
        existing as Record<string, unknown>,
        value as Record<string, unknown>,
      )
    } else {
      out[key] = value
    }
  }
  return out as T
}
