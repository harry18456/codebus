import { create } from "zustand"

import {
  type ActiveProfile,
  type AzureProfile,
  type ClaudeCodeBlock,
  type CodexAzureProfile,
  type CodexBlock,
  type CodexSystemProfile,
  type GlobalConfig,
  type HooksConfig,
  type SystemProfile,
  DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE,
  DEFAULT_CODEX_AZURE_KEYRING_SERVICE,
  HOOKS_CONFIG_DEFAULTS,
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
   * Provider-keyed read of `agent.providers.<id>` (raw block, no default
   * filling — callers that need defaults use the provider's own reader).
   */
  getProviderBlock: (id: string) => unknown
  /**
   * Provider-keyed write: set `agent.providers.<id>` to `block` AND
   * `agent.active_provider` to `id`, preserving sibling provider blocks.
   * Marks the store dirty. This is the registry-driven replacement for the
   * claude-only `updateClaudeCode`; `updateClaudeCode` delegates here.
   */
  updateProviderBlock: (id: string, block: unknown) => void
  /**
   * Get a fully-populated `CodexBlock` from `agent.providers.codex`, filling
   * defaults for missing fields (empty model strings — codex has no built-in
   * model defaults; api_version empty; keyring_service default). Mirrors
   * {@link getClaudeCodeBlock} for the codex provider.
   */
  getCodexBlock: () => CodexBlock
  /**
   * Set `agent.active_provider` to `id`, preserving all provider blocks and
   * other config. Used by the Settings provider selector to switch providers
   * without mutating any endpoint block.
   */
  setActiveProvider: (id: string) => void
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
  /**
   * Number of built-in PII patterns the active scanner ships with, read from
   * the backend-injected `__pii_pattern_count` payload key (see
   * `load_global_config`). Returns `null` until the config has loaded so the
   * Settings UI can degrade to a neutral placeholder instead of a hard-coded
   * number — satisfying the app-shell "PII pattern count is dynamic" rule.
   */
  getPiiPatternCount: () => number | null
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
    // Registry-driven: delegate to the provider-keyed writer (claude is just
    // the provider with id "claude"). Preserved for existing callers/tests.
    get().updateProviderBlock("claude", block)
  },

  getProviderBlock(id) {
    const cfg = get().config as {
      agent?: { providers?: Record<string, unknown> }
    }
    return cfg.agent?.providers?.[id]
  },

  getCodexBlock() {
    return readCodexBlock(get().config)
  },

  setActiveProvider(id) {
    const cur = get().config
    const curAgent = (cur.agent ?? {}) as {
      active_provider?: string
      providers?: Record<string, unknown>
    }
    const next = {
      ...cur,
      agent: { ...curAgent, active_provider: id },
    } as GlobalConfig
    set({ config: next, dirty: true })
  },

  updateProviderBlock(id, block) {
    // The selected provider's endpoint block lives at `agent.providers.<id>`.
    // Preserve sibling provider blocks and set `active_provider` to `id`.
    const cur = get().config
    const curAgent = (cur.agent ?? {}) as {
      active_provider?: string
      providers?: Record<string, unknown>
    }
    const next = {
      ...cur,
      agent: {
        ...curAgent,
        active_provider: id,
        providers: { ...(curAgent.providers ?? {}), [id]: block },
      },
    } as GlobalConfig
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

  getPiiPatternCount() {
    const raw = get().config.__pii_pattern_count
    return typeof raw === "number" ? raw : null
  },

  reset() {
    set((state) => ({ config: state.initialConfig, dirty: false, error: null }))
  },

  async save() {
    set({ saving: true, error: null })
    try {
      // config-save-robustness: strip blank PII rules before persisting (see
      // sanitizeForSave). The backend save_global_config filters too; doing it
      // here keeps the sent payload and the post-save in-memory state honest.
      const cfg = sanitizeForSave(get().config)
      await saveGlobalConfigIpc(cfg)
      set({ config: cfg, initialConfig: cfg, dirty: false, saving: false })
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
 * Read the claude provider's endpoint block from `agent.providers.claude`.
 * Always returns a fully populated `ClaudeCodeBlock` — missing keys are
 * filled with built-in defaults, missing azure block is reported as `null`.
 *
 * The unified schema has no legacy fallback: a config still using the old
 * top-level `claude_code` block (or no `agent` block at all) reads as absent
 * here and yields defaults — exactly what the CLI loader does, so UI and CLI
 * agree.
 */
function readClaudeCodeBlock(config: GlobalConfig | null | undefined): ClaudeCodeBlock {
  const raw = (
    config as
      | { agent?: { providers?: { claude?: unknown } } }
      | null
      | undefined
  )?.agent?.providers?.claude as Partial<ClaudeCodeBlock> | undefined
  const active: ActiveProfile = raw?.active === "azure" ? "azure" : "system"
  const system: SystemProfile = mergeSystemProfile(raw?.system)
  const azure: AzureProfile | null = readAzureProfile(raw?.azure)
  return { active, system, azure }
}

/**
 * Read the codex provider's endpoint block from `agent.providers.codex`,
 * filling defaults. Codex has no built-in model defaults (free-text), so
 * missing models default to empty strings; the azure profile carries
 * `api_version` and a default `keyring_service`.
 */
function readCodexBlock(config: GlobalConfig | null | undefined): CodexBlock {
  const raw = (
    config as { agent?: { providers?: { codex?: unknown } } } | null | undefined
  )?.agent?.providers?.codex as Partial<CodexBlock> | undefined
  const active: ActiveProfile = raw?.active === "azure" ? "azure" : "system"
  const sys = (raw?.system ?? {}) as Partial<CodexSystemProfile>
  const system: CodexSystemProfile = {
    goal: sys.goal ?? { model: "", effort: "high" },
    query: sys.query ?? { model: "", effort: "low" },
    fix: sys.fix ?? { model: "", effort: "medium" },
    verify: sys.verify ?? { model: "", effort: "high" },
  }
  return { active, system, azure: readCodexAzure(raw?.azure) }
}

function readCodexAzure(raw: unknown): CodexAzureProfile | null {
  if (!raw || typeof raw !== "object") return null
  const r = raw as Partial<CodexAzureProfile>
  return {
    base_url: r.base_url ?? "",
    api_version: r.api_version ?? "",
    keyring_service:
      r.keyring_service && r.keyring_service.length > 0
        ? r.keyring_service
        : DEFAULT_CODEX_AZURE_KEYRING_SERVICE,
    goal: r.goal ?? { model: "", effort: "high" },
    query: r.query ?? { model: "", effort: "low" },
    fix: r.fix ?? { model: "", effort: "medium" },
    verify: r.verify ?? { model: "", effort: "high" },
  }
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

/**
 * `pretooluse-image-block-toggle`: read a `HooksConfig` from a possibly-
 * empty / partial `hooks` section. Mirrors {@link mergeSystemProfile} —
 * absent / non-object input falls back to the built-in defaults; a
 * `read_image_block` value that is not a boolean is also treated as
 * absent (fail-safe default true). Forward-compat: unknown sibling
 * subkeys (e.g. a future `hooks.write_image_block`) are ignored here;
 * the IPC round-trip preserves them at the save layer.
 */
function mergeHooksConfig(raw: unknown): HooksConfig {
  if (!raw || typeof raw !== "object") {
    return { ...HOOKS_CONFIG_DEFAULTS }
  }
  const r = raw as { read_image_block?: unknown }
  const block =
    typeof r.read_image_block === "boolean"
      ? r.read_image_block
      : HOOKS_CONFIG_DEFAULTS.read_image_block
  return { read_image_block: block }
}

/**
 * Read the `hooks` config block from a possibly-empty global config.
 * Always returns a fully populated `HooksConfig`. Callers SHALL use
 * this helper instead of indexing `config.hooks` directly so missing /
 * malformed input degrades to the safe default consistently.
 */
export function readHooksConfig(config: GlobalConfig | null | undefined): HooksConfig {
  const raw = (config as { hooks?: unknown } | null | undefined)?.hooks
  return mergeHooksConfig(raw)
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
      : DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE,
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

/**
 * config-save-robustness: strip empty / whitespace-only `pii.patterns_extra`
 * entries before persisting. A blank PII rule (an unfilled editor row) is an
 * empty regex; left in the saved file it makes the next mirror scan match
 * zero-width at every character. Mirrors the backend `save_global_config`
 * filter. Returns a new object when it removes anything; never mutates input.
 */
function sanitizeForSave(config: GlobalConfig): GlobalConfig {
  const pii = config.pii
  if (!pii || typeof pii !== "object") return config
  const extras = (pii as { patterns_extra?: unknown }).patterns_extra
  if (!Array.isArray(extras)) return config
  const filtered = extras.filter(
    (p) => typeof p !== "string" || p.trim().length > 0,
  )
  if (filtered.length === extras.length) return config
  return {
    ...config,
    pii: { ...(pii as Record<string, unknown>), patterns_extra: filtered },
  }
}
