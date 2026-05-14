import { invoke } from "@tauri-apps/api/core"

/** Vault entry returned by `list_vaults` / `add_vault`. */
export interface VaultEntry {
  path: string
  display_name: string
  last_opened: string
  is_missing: boolean
}

export type AddVaultMode = "detect" | "just_bind" | "re_init"

export interface AddVaultOptions {
  mode: AddVaultMode
}

/**
 * Opaque global config payload mirroring the Rust struct. Specific field
 * typing lands with task 6.1 (AppConfig namespace); for now we just route
 * the JSON shape through.
 */
export interface GlobalConfig {
  claude_code?: unknown
  pii?: unknown
  log?: unknown
  app?: {
    quiz?: {
      pass_threshold?: number
      default_length?: number
    }
  }
  [key: string]: unknown
}

/**
 * Single discriminated-union error type returned by every IPC command,
 * mirroring `AppError` in `error.rs` (serde tag `kind`, snake_case).
 */
export type AppError =
  | { kind: "io"; message: string }
  | { kind: "config_parse"; message: string }
  | { kind: "vault_not_found"; path: string }
  | { kind: "vault_already_exists"; path: string }
  | { kind: "invalid"; field: string; message: string }
  | { kind: "internal"; message: string }

/**
 * `instanceof`-free narrowing helper. Tauri converts thrown `AppError` into
 * a plain object across the IPC boundary, so callers must duck-type rather
 * than rely on a class.
 */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false
  const v = value as { kind?: unknown }
  return (
    typeof v.kind === "string" &&
    [
      "io",
      "config_parse",
      "vault_not_found",
      "vault_already_exists",
      "invalid",
      "internal",
    ].includes(v.kind)
  )
}

/**
 * The exhaustive set of commands the Tauri backend exposes. Keep this list
 * in lockstep with `ipc::REGISTERED_COMMANDS` in `codebus-app-tauri/src/ipc/mod.rs`.
 */
export type IpcCommandName =
  | "list_vaults"
  | "add_vault"
  | "remove_vault"
  | "load_global_config"
  | "save_global_config"
  | "set_endpoint_key"
  | "get_endpoint_key"
  | "delete_endpoint_key"
  | "check_cli_installed"
  | "spawn_goal"
  | "cancel_goal"
  | "list_runs"
  | "get_run_detail"
  | "list_wiki_pages"
  | "read_wiki_page"

/**
 * Endpoint profile selector. Currently only `"azure"` is wired up; future
 * endpoint types extend this union AND the Rust `resolve_keyring_service`
 * match arm AND the spec's `Settings UI Endpoint Section` requirement.
 */
export type EndpointProfile = "azure"

/**
 * Reply shape of `get_endpoint_key`. SHALL NOT carry the key value;
 * verifying a key requires running `codebus query "ping"` in the CLI.
 */
export type KeyStatus = { kind: "set" } | { kind: "unset" }

/**
 * Closed enum of system-profile model aliases. Mirrors `SystemModel` in
 * `codebus-core/src/config/endpoint.rs` — adding a value here without the
 * matching Rust variant is a compile error on the IPC round-trip.
 */
export const SYSTEM_MODELS = [
  "opus-4-7",
  "opus-4-6",
  "haiku-4-5",
  "sonnet-4-6",
] as const
export type SystemModel = (typeof SYSTEM_MODELS)[number]

/**
 * Active profile of the `claude_code` config block.
 */
export type ActiveProfile = "system" | "azure"

export interface SystemVerb {
  model: SystemModel
  effort: string
}

export interface AzureVerb {
  model: string
  effort: string
}

export interface SystemProfile {
  goal: SystemVerb
  query: SystemVerb
  fix: SystemVerb
}

export interface AzureProfile {
  base_url: string
  keyring_service: string
  goal: AzureVerb
  query: AzureVerb
  fix: AzureVerb
}

export interface ClaudeCodeBlock {
  active: ActiveProfile
  system: SystemProfile
  azure: AzureProfile | null
}

/** Default keyring service when the user has not configured one yet. */
export const DEFAULT_AZURE_KEYRING_SERVICE = "codebus-azure"

/** Built-in defaults mirroring `SystemProfile::default()` in Rust. */
export const SYSTEM_PROFILE_DEFAULTS: SystemProfile = {
  goal: { model: "opus-4-6", effort: "high" },
  query: { model: "haiku-4-5", effort: "low" },
  fix: { model: "sonnet-4-6", effort: "medium" },
}

/**
 * Validation result for a `ClaudeCodeBlock` keyed by which field failed.
 * Empty array = block is currently saveable.
 *
 * Mirrors the rules codebus-core enforces on load:
 * - When `active === "azure"`, the entire azure block MUST be populated
 *   (base_url + keyring_service + every verb's deployment model name).
 * - When `active === "system"`, the azure block MAY be partial (cold
 *   storage) so no azure-side validation runs.
 * - System profile inputs are always required because `SystemModel` is
 *   a closed enum (frontend dropdown enforces the values).
 *
 * The frontend uses this to disable the Save button + render
 * `aria-invalid` on the offending inputs.
 */
export interface ClaudeCodeValidationError {
  field: string
  message: string
}

export function validateClaudeCodeBlock(
  block: ClaudeCodeBlock,
): ClaudeCodeValidationError[] {
  const errors: ClaudeCodeValidationError[] = []
  if (block.active === "azure") {
    const az = block.azure
    if (!az) {
      errors.push({
        field: "claude_code.azure",
        message: "Azure profile is required when active=azure",
      })
      return errors
    }
    if (!az.base_url.trim()) {
      errors.push({
        field: "claude_code.azure.base_url",
        message: "base_url is required when active=azure",
      })
    }
    if (!az.keyring_service.trim()) {
      errors.push({
        field: "claude_code.azure.keyring_service",
        message: "keyring_service is required when active=azure",
      })
    }
    for (const verb of ["goal", "query", "fix"] as const) {
      if (!az[verb].model.trim()) {
        errors.push({
          field: `claude_code.azure.${verb}.model`,
          message: `${verb} deployment name is required when active=azure`,
        })
      }
    }
  }
  return errors
}

/**
 * Generic typed invoke. Restricts the command name to the exhaustive union
 * above so any typo or stray command in the frontend is a compile error.
 */
async function invokeTyped<T>(
  command: IpcCommandName,
  args?: Record<string, unknown>,
): Promise<T> {
  return invoke<T>(command, args)
}

export async function listVaults(): Promise<VaultEntry[]> {
  return invokeTyped<VaultEntry[]>("list_vaults")
}

export async function addVault(
  path: string,
  options: AddVaultOptions = { mode: "detect" },
): Promise<VaultEntry> {
  return invokeTyped<VaultEntry>("add_vault", { path, options })
}

export async function removeVault(path: string): Promise<void> {
  return invokeTyped<void>("remove_vault", { path })
}

export async function loadGlobalConfig(): Promise<GlobalConfig> {
  return invokeTyped<GlobalConfig>("load_global_config")
}

export async function saveGlobalConfig(config: GlobalConfig): Promise<void> {
  return invokeTyped<void>("save_global_config", { config })
}

/**
 * Store an API key in the OS keyring for the given endpoint profile.
 * The key value is forwarded only to the Tauri command and SHALL NOT be
 * persisted anywhere in the frontend (no Zustand state, no localStorage,
 * no DOM attribute) beyond the call-site that originated it.
 */
export async function setEndpointKey(
  profile: EndpointProfile,
  key: string,
): Promise<void> {
  return invokeTyped<void>("set_endpoint_key", { profile, key })
}

/**
 * Return whether the keyring entry exists for the given profile. The
 * backend SHALL NOT return the key value — only a discriminated-union
 * status. Verifying the key value requires running the CLI verb.
 */
export async function getEndpointKey(
  profile: EndpointProfile,
): Promise<KeyStatus> {
  return invokeTyped<KeyStatus>("get_endpoint_key", { profile })
}

/**
 * Remove the keyring entry for the given profile. Idempotent — succeeds
 * whether or not an entry existed.
 */
export async function deleteEndpointKey(
  profile: EndpointProfile,
): Promise<void> {
  return invokeTyped<void>("delete_endpoint_key", { profile })
}

/**
 * Reply shape of `check_cli_installed`. Probes whether the agentic CLI
 * binary for the given provider is installed and reachable by spawning
 * `<binary> --version`. Failure paths (binary missing, non-zero exit)
 * collapse into `NotInstalled` — the user sees the same UX regardless.
 */
export type CliStatus =
  | { kind: "installed"; version: string }
  | { kind: "not_installed" }

/**
 * Agentic-CLI provider identifier. Currently only `"claude_code"` is
 * supported; future Codex / Gemini integrations extend this union AND
 * the Rust `check_cli_installed` match arm.
 */
export type AgenticProvider = "claude_code"

/**
 * Probe whether the agentic CLI binary for `provider` is installed and
 * reachable. Returns `{ kind: "installed", version }` when the probe
 * succeeds, otherwise `{ kind: "not_installed" }`.
 */
export async function checkCliInstalled(
  provider: AgenticProvider,
): Promise<CliStatus> {
  return invokeTyped<CliStatus>("check_cli_installed", { provider })
}

// ===========================================================================
// Workspace IPC surface (v3-app-workspace-goal)
// ===========================================================================

/**
 * Token usage normalized across providers. Mirrors `codebus_core::log::TokenUsage`.
 * The optional fields are absent when the provider does not have the
 * corresponding concept (e.g., legacy OpenAI has no cache notion).
 */
export interface TokenUsage {
  input_tokens: number
  output_tokens: number
  cache_read_tokens?: number
  cache_write_tokens?: number
  reasoning_tokens?: number
  extras?: unknown
}

/**
 * Run-log summary projection returned by `list_runs` / embedded in
 * `RunDetail`. `run_id` is the run's `started_at` slug (`:` → `-`).
 * Virtual interrupted entries (no on-disk RunLog row) project into the
 * same shape with `outcome === "interrupted"` and empty `finished_at`.
 */
export interface RunLogSummary {
  run_id: string
  mode: string
  goal: string
  model?: string
  effort?: string
  started_at: string
  finished_at: string
  tokens: TokenUsage
  wiki_changed: boolean
  lint_error_count: number
  lint_warn_count: number
  outcome: string
  session_id?: string
}

/** Closed set of legal RunLog outcomes the GUI distinguishes. */
export type RunOutcome =
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "interrupted"

/** Tagged stream payload type matching codebus-core `StreamEvent`. */
export type StreamEvent =
  | { kind: "thought"; text: string }
  | { kind: "tool_use"; name: string; input: unknown }
  | { kind: "tool_result"; output: string; is_error: boolean }
  | ({ kind: "usage" } & TokenUsage)

/** Tagged banner payload type matching codebus-core `VerbBanner`. */
export type VerbBanner =
  | { kind: "start"; repo_path: string }
  | { kind: "goal"; goal_text: string }
  | { kind: "sync_start" }
  | {
      kind: "sync_done"
      files: number
      mib: number
      elapsed_ms: number
    }
  | {
      kind: "pii_summary"
      scanner: string
      scanned: number
      hits: number
      action: string
    }
  | { kind: "lint_start" }
  | {
      kind: "lint_done"
      errors: number
      warns: number
      elapsed_ms: number
    }
  | { kind: "commit_done"; sha7: string }
  | { kind: "done"; wiki_path: string }
  | { kind: "hint"; wiki_path: string }

/** Tagged lifecycle payload type matching codebus-core `VerbLifecycleEvent`. */
export type VerbLifecycleEvent =
  | { kind: "spawn_start"; verb: string }
  | { kind: "spawn_end"; verb: string; exit_code: number | null }
  | { kind: "fix_iteration_start"; iteration: number }
  | { kind: "lint_final"; error_count: number; warn_count: number }
  | { kind: "promote_suggestion"; reason: string }

/**
 * Top-level event emitted by `verb::*::run_*` orchestration. Frontend
 * receives this via the `goal-stream` Tauri event channel wrapped in a
 * `GoalStreamPayload { run_id, event }`.
 */
export type VerbEvent =
  | { kind: "banner"; data: VerbBanner }
  | { kind: "stream"; data: StreamEvent }
  | { kind: "lifecycle"; data: VerbLifecycleEvent }

/**
 * One line of an events-*.jsonl file. `ts` is captured at append time
 * (RFC 3339 UTC); `event` is the originating `VerbEvent` payload.
 */
export interface EventEnvelope {
  ts: string
  event: VerbEvent
}

/** Payload of one `goal-stream` Tauri event tick. */
export interface GoalStreamPayload {
  run_id: string
  event: VerbEvent
}

/**
 * Payload emitted exactly once on the `goal-terminal` Tauri channel
 * after a spawn's background thread exits (success / fail / cancel /
 * panic). Frontend uses this to clear `useGoalsStore.activeRun` and
 * refresh the runs list so the new RunLog row is picked up.
 */
export interface GoalTerminalPayload {
  run_id: string
}

/** Detail bundle returned by `get_run_detail`. */
export interface RunDetail {
  summary: RunLogSummary
  events: EventEnvelope[]
}

/** Tagged enum for `list_runs` mode filter. */
export type ModeFilter = { kind: "goal" } | { kind: "all" }

/** Wiki page metadata returned by `list_wiki_pages`. */
export interface WikiPageMeta {
  slug: string
  path: string
  title: string
}

/**
 * Spawn a background goal run. Returns the run id (= started_at slug)
 * so the caller can switch to the Running detail view before any
 * `goal-stream` event has arrived. Rejects with `AppError::Invalid
 * { field: "active_runs" }` when another goal run is already active.
 */
export async function spawnGoal(
  vaultPath: string,
  goalText: string,
): Promise<string> {
  return invokeTyped<string>("spawn_goal", {
    vaultPath,
    goalText,
  })
}

/**
 * Flip the cancel flag for a given run. Idempotent — succeeds even
 * when the run has already terminated and the entry is gone from
 * `active_runs`.
 */
export async function cancelGoal(runId: string): Promise<void> {
  return invokeTyped<void>("cancel_goal", { runId })
}

/**
 * List runs in a vault, optionally filtered by mode. Includes virtual
 * `outcome === "interrupted"` entries synthesized from orphan
 * events-*.jsonl files (no matching RunLog row).
 */
export async function listRuns(
  vaultPath: string,
  modeFilter: ModeFilter,
): Promise<RunLogSummary[]> {
  return invokeTyped<RunLogSummary[]>("list_runs", {
    vaultPath,
    modeFilter,
  })
}

/**
 * Load the full detail bundle for a single run: the summary plus a
 * one-shot tail-replay of the corresponding events-*.jsonl file. v1
 * reads the file synchronously (typical run is < 200 events / 500 KB).
 */
export async function getRunDetail(
  vaultPath: string,
  runId: string,
): Promise<RunDetail> {
  return invokeTyped<RunDetail>("get_run_detail", {
    vaultPath,
    runId,
  })
}

/**
 * Enumerate wiki pages in a vault. Files without a parseable
 * frontmatter `title` fall back to the slug as the title.
 */
export async function listWikiPages(
  vaultPath: string,
): Promise<WikiPageMeta[]> {
  return invokeTyped<WikiPageMeta[]>("list_wiki_pages", { vaultPath })
}

/**
 * Read a wiki page's body with the leading `---\n...\n---\n`
 * frontmatter block stripped. Rejects with `AppError::Invalid
 * { field: "page_slug" }` when no file matches the slug.
 */
export async function readWikiPage(
  vaultPath: string,
  pageSlug: string,
): Promise<string> {
  return invokeTyped<string>("read_wiki_page", {
    vaultPath,
    pageSlug,
  })
}
