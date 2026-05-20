import { create } from "zustand"

import {
  type ActiveProfile,
  type AzureProfile,
  type ClaudeCodeBlock,
  type GlobalConfig,
  type SystemProfile,
  DEFAULT_AZURE_KEYRING_SERVICE,
  SYSTEM_PROFILE_DEFAULTS,
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
  /**
   * Get a fully-populated `claude_code` block from the current config,
   * filling defaults for any missing field. Returns the new profile-mode
   * schema (active + system + azure) — never the legacy flat shape.
   * Components mutate the returned object freely (it's a fresh copy) and
   * persist via {@link updateClaudeCode}.
   */
  getClaudeCodeBlock: () => ClaudeCodeBlock
  /**
   * Write the entire `claude_code` block back to the in-memory config.
   * Marks the store dirty so the Save button enables.
   */
  updateClaudeCode: (block: ClaudeCodeBlock) => void
  /**
   * Quiz summary pass/fail threshold (percent), read from
   * `app.quiz.pass_threshold` — the same key the Settings modal binds.
   * Defaults to 80 only when the key is absent (mirrors the
   * default-when-absent pattern used for the shared `quiz.default_length`).
   * The Quiz tab uses this instead of a hardcoded component constant.
   */
  getPassThreshold: () => number
  /**
   * Generated quiz question count, resolved from the shared top-level
   * `quiz.default_length` (authoritative), falling back to the legacy
   * `app.quiz.default_length` for un-migrated configs, then 5 when
   * neither is set. Clamped to the inclusive 3..10 range — the same
   * range the core `quiz` config loader enforces, so an out-of-range
   * configured value silently converges instead of being rejected.
   * The Quiz tab passes this to the generate spawn instead of a
   * hardcoded constant.
   */
  getDefaultLength: () => number
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

  getClaudeCodeBlock() {
    return readClaudeCodeBlock(get().config)
  },

  updateClaudeCode(block) {
    const next = { ...get().config, claude_code: block } as GlobalConfig
    set({ config: next, dirty: true })
  },

  getPassThreshold() {
    return get().config.app?.quiz?.pass_threshold ?? 80
  },

  getDefaultLength() {
    const cfg = get().config
    const raw = cfg.quiz?.default_length ?? cfg.app?.quiz?.default_length ?? 5
    return Math.min(10, Math.max(3, raw))
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


/**
 * Read a profile-shaped `claude_code` block from a possibly-empty / possibly-
 * legacy config payload. Always returns a fully populated `ClaudeCodeBlock`
 * — missing keys are filled with built-in defaults, missing azure block
 * is reported as `null`.
 *
 * Legacy-schema config (top-level `goal` / `query` / `fix` under
 * `claude_code` without `system` / `azure` wrappers) is NOT migrated
 * here — the IPC load path emits a stderr warning and falls back to
 * defaults; this function just reads what the load path produced. UI
 * therefore always sees the new profile shape.
 */
function readClaudeCodeBlock(config: GlobalConfig | null | undefined): ClaudeCodeBlock {
  const raw = (config as { claude_code?: unknown } | null | undefined)?.claude_code as
    | Partial<ClaudeCodeBlock>
    | undefined
  const active: ActiveProfile = raw?.active === "azure" ? "azure" : "system"
  const system: SystemProfile = mergeSystemProfile(raw?.system)
  const azure: AzureProfile | null = readAzureProfile(raw?.azure)
  return { active, system, azure }
}

function mergeSystemProfile(raw: unknown): SystemProfile {
  const r = (raw ?? {}) as Partial<SystemProfile>
  return {
    goal:   r.goal   ?? { ...SYSTEM_PROFILE_DEFAULTS.goal },
    query:  r.query  ?? { ...SYSTEM_PROFILE_DEFAULTS.query },
    fix:    r.fix    ?? { ...SYSTEM_PROFILE_DEFAULTS.fix },
    verify: r.verify ?? { ...SYSTEM_PROFILE_DEFAULTS.verify },
  }
}

function readAzureProfile(raw: unknown): AzureProfile | null {
  if (!raw || typeof raw !== "object") return null
  const r = raw as Partial<AzureProfile>
  // Treat a partial azure block as still surfacing to the UI so the user
  // can finish editing — but pre-fill the keyring_service default when
  // the field is empty / missing (Stage A first-time-setup ergonomics).
  return {
    base_url: r.base_url ?? "",
    keyring_service: r.keyring_service && r.keyring_service.length > 0
      ? r.keyring_service
      : DEFAULT_AZURE_KEYRING_SERVICE,
    goal:   r.goal   ?? { model: "", effort: "high" },
    query:  r.query  ?? { model: "", effort: "low" },
    fix:    r.fix    ?? { model: "", effort: "medium" },
    verify: r.verify ?? { model: "", effort: "high" },
  }
}

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
