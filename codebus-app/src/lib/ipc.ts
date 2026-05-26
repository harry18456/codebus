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
  /**
   * Unified agent config: `active_provider` selects the agent CLI, and each
   * provider's endpoint block lives under `providers.<name>`. The claude
   * provider's block ({ active, system, azure }) is read/written by the
   * settings store via `agent.providers.claude`.
   */
  agent?: {
    active_provider?: string
    providers?: { claude?: unknown }
  }
  pii?: unknown
  log?: unknown
  /**
   * Shared top-level quiz namespace (authoritative). `default_length`
   * is the generated question count, range 3–10, default 5. The legacy
   * `app.quiz.default_length` is tolerated as a fallback for
   * un-migrated configs.
   */
  quiz?: {
    default_length?: number
  }
  app?: {
    quiz?: {
      pass_threshold?: number
      default_length?: number
    }
    /**
     * User-selected locale override. `"zh"` and `"en"` pin the UI to that
     * language; `null` (or an absent key, including in legacy configs) means
     * "auto-detect from `navigator.language`". Validated at the UI write
     * layer via {@link parseLocaleOverride} — the Rust side passes the value
     * through unchanged.
     */
    locale_override?: "zh" | "en" | null
    [key: string]: unknown
  }
  [key: string]: unknown
}

/**
 * Read and validate `config.app.locale_override`. Returns `null` when the
 * key is absent or explicitly `null` (auto-detect); returns the locale
 * string when set to `"zh"` or `"en"`. Throws on any other value — callers
 * (settings load path, settings modal) MUST surface the error rather than
 * silently coerce to a fallback.
 *
 * Backs spec *Settings Language Override*: the schema accepts exactly three
 * values, and legacy configs without the key resolve to auto-detect.
 */
export function parseLocaleOverride(
  config: GlobalConfig | null | undefined,
): "zh" | "en" | null {
  const raw = (config as { app?: { locale_override?: unknown } } | null | undefined)
    ?.app?.locale_override
  if (raw === undefined || raw === null) return null
  if (raw === "zh" || raw === "en") return raw
  throw new Error(
    `Invalid app.locale_override: expected "zh" | "en" | null, got ${JSON.stringify(raw)}`,
  )
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
  | "get_obsidian_vault_id"
  | "open_wiki_in_obsidian"
  | "spawn_chat_turn"
  | "cancel_chat_turn"
  | "spawn_quiz_plan"
  | "spawn_quiz_generate"
  | "cancel_quiz"
  | "list_quiz_attempts"
  | "read_quiz_attempt"
  | "read_quiz_events"
  | "read_quiz_progress"
  | "write_quiz_progress"

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
 * Suggested system-profile model aliases for the combobox. The system model
 * is a FREE STRING (codebus-core relaxed `SystemModel` to a string + a
 * `claude-` prefix translation), so a newly-released Claude model works
 * without a code change — these are quick-pick hints, not a closed set.
 */
export const SYSTEM_MODELS = [
  "opus-4-7",
  "opus-4-6",
  "sonnet-4-6",
  "haiku-4-5",
] as const

/**
 * Closed enum of valid `effort` values surfaced by the Settings UI
 * dropdown — mirrors the Claude Code CLI `--effort` accepted set
 * (low / medium / high / xhigh / max / auto). The Rust side keeps
 * `effort: String` for yaml backward compatibility, so this enum is
 * enforced only at the UI layer via `validateClaudeCodeBlock`. Order
 * is fixed (ascending strength, with `auto` last as the "let the
 * model decide" sentinel) and matches the option order rendered by
 * the `<select>` per spec `Settings UI Endpoint Section` scenarios.
 */
export const SYSTEM_EFFORTS = [
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
  "auto",
] as const
export type SystemEffort = (typeof SYSTEM_EFFORTS)[number]

function isSystemEffort(value: string): value is SystemEffort {
  return (SYSTEM_EFFORTS as readonly string[]).includes(value)
}

/**
 * Active profile of the `claude_code` config block.
 */
export type ActiveProfile = "system" | "azure"

export interface SystemVerb {
  /** Free string (alias like `opus-4-7` or full `claude-…` id). */
  model: string
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
  /**
   * `verify-stage-independent-model`: dedicated sub-block for the
   * content-verify spawn shared by quiz and goal verbs. Required in
   * the active profile (see spec `claude-code-config` `Endpoint
   * Profile Schema`). Default is `opus-4-6 / high` — strongest
   * reasoning model + highest effort, encoding the "expensive
   * verification" design intent.
   */
  verify: SystemVerb
}

export interface AzureProfile {
  base_url: string
  keyring_service: string
  goal: AzureVerb
  query: AzureVerb
  fix: AzureVerb
  /**
   * `verify-stage-independent-model`: azure-side mirror of
   * `SystemProfile.verify` — the deployment name + effort used by
   * the content-verify spawn when `active === "azure"`. Required
   * in active profile.
   */
  verify: AzureVerb
}

export interface ClaudeCodeBlock {
  active: ActiveProfile
  system: SystemProfile
  azure: AzureProfile | null
}

/**
 * Codex provider verb settings. Unlike claude's `SystemVerb`, `model` is an
 * arbitrary string (codex model names are NOT a closed enum) and `effort` is
 * a free string forwarded as `model_reasoning_effort`.
 */
export interface CodexVerb {
  model: string
  effort: string
}

export interface CodexSystemProfile {
  goal: CodexVerb
  query: CodexVerb
  fix: CodexVerb
  verify: CodexVerb
}

/**
 * Codex Azure profile — the Responses-API variant. Carries `api_version`
 * (claude's azure does not) alongside `base_url` and `keyring_service`.
 */
export interface CodexAzureProfile {
  base_url: string
  api_version: string
  keyring_service: string
  goal: CodexVerb
  query: CodexVerb
  fix: CodexVerb
  verify: CodexVerb
}

export interface CodexBlock {
  active: ActiveProfile
  system: CodexSystemProfile
  azure: CodexAzureProfile | null
}

/**
 * `pretooluse-image-block-toggle`: runtime gate for `codebus hook
 * check-read`. When `read_image_block` is true (default), the hook
 * subcommand blocks image / binary file extensions per the existing
 * blocklist. When false, the hook short-circuits to allow.
 */
export interface HooksConfig {
  read_image_block: boolean
}

/** Built-in defaults mirroring `HooksConfig::default()` in Rust. */
export const HOOKS_CONFIG_DEFAULTS: HooksConfig = {
  read_image_block: true,
}

/**
 * Default azure keyring service per provider, used to pre-fill the editor
 * when the config has no `keyring_service` yet. Claude and codex default to
 * DISTINCT names so their keys never collide in the OS keyring; the field
 * stays editable, so a user wanting a shared key can type `codebus-azure`.
 */
export const DEFAULT_CLAUDE_AZURE_KEYRING_SERVICE = "codebus-claude-azure"
export const DEFAULT_CODEX_AZURE_KEYRING_SERVICE = "codebus-codex-azure"

/** Built-in defaults mirroring `SystemProfile::default()` in Rust. */
export const SYSTEM_PROFILE_DEFAULTS: SystemProfile = {
  goal: { model: "opus-4-6", effort: "high" },
  query: { model: "haiku-4-5", effort: "low" },
  fix: { model: "sonnet-4-6", effort: "medium" },
  verify: { model: "opus-4-6", effort: "high" },
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
 * - System profile `model` is a free string (not validated here); the
 *   combobox offers `SYSTEM_MODELS` as suggestions but any value is allowed.
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
    for (const verb of ["goal", "query", "fix", "verify"] as const) {
      if (!az[verb].model.trim()) {
        errors.push({
          field: `claude_code.azure.${verb}.model`,
          message: `${verb} deployment name is required when active=azure`,
        })
      }
    }
  }
  // Effort enum check applies to BOTH profiles regardless of `active`
  // so cold-storage values cannot silently carry a legacy / non-enum
  // value through Save (spec `Settings UI Endpoint Section`).
  for (const verb of ["goal", "query", "fix", "verify"] as const) {
    if (!isSystemEffort(block.system[verb].effort)) {
      errors.push({
        field: `claude_code.system.${verb}.effort`,
        message: `${verb} effort must be one of ${SYSTEM_EFFORTS.join(" / ")}`,
      })
    }
  }
  if (block.azure) {
    for (const verb of ["goal", "query", "fix", "verify"] as const) {
      if (!isSystemEffort(block.azure[verb].effort)) {
        errors.push({
          field: `claude_code.azure.${verb}.effort`,
          message: `${verb} effort must be one of ${SYSTEM_EFFORTS.join(" / ")}`,
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
 * Store an API key in the OS keyring under the given `service` name. The
 * caller passes the active provider's `azure.keyring_service` (defaulting to
 * `codebus-claude-azure` / `codebus-codex-azure`) so claude and codex keys
 * occupy distinct entries. The key value is forwarded only to the Tauri
 * command and SHALL NOT be persisted anywhere in the frontend beyond the
 * call-site that originated it.
 */
export async function setEndpointKey(
  service: string,
  key: string,
): Promise<void> {
  return invokeTyped<void>("set_endpoint_key", { service, key })
}

/**
 * Return whether a keyring entry exists under `service`. The backend SHALL
 * NOT return the key value — only a discriminated-union status. Verifying the
 * key value requires running the CLI verb.
 */
export async function getEndpointKey(service: string): Promise<KeyStatus> {
  return invokeTyped<KeyStatus>("get_endpoint_key", { service })
}

/**
 * Remove the keyring entry under `service`. Idempotent — succeeds whether or
 * not an entry existed.
 */
export async function deleteEndpointKey(service: string): Promise<void> {
  return invokeTyped<void>("delete_endpoint_key", { service })
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
export type AgenticProvider = "claude_code" | "codex"

/**
 * Client-side validation for a `CodexBlock`. Mirrors `parse_codex_yaml`:
 * when `active === "azure"`, `base_url` / `api_version` / `keyring_service`
 * and every verb's deployment `model` MUST be non-empty; when
 * `active === "system"`, every verb's `model` MUST be non-empty. Codex model
 * strings are arbitrary (no closed-enum check) and `effort` is free-form (no
 * enum check) — the only rule is non-empty required fields on the active
 * profile. Empty array = saveable.
 */
export function validateCodexBlock(block: CodexBlock): ClaudeCodeValidationError[] {
  const errors: ClaudeCodeValidationError[] = []
  const verbs = ["goal", "query", "fix", "verify"] as const
  if (block.active === "azure") {
    const az = block.azure
    if (!az) {
      errors.push({
        field: "codex.azure",
        message: "Azure profile is required when active=azure",
      })
      return errors
    }
    if (!az.base_url.trim()) {
      errors.push({ field: "codex.azure.base_url", message: "base_url is required when active=azure" })
    }
    if (!az.api_version.trim()) {
      errors.push({ field: "codex.azure.api_version", message: "api_version is required when active=azure" })
    }
    if (!az.keyring_service.trim()) {
      errors.push({ field: "codex.azure.keyring_service", message: "keyring_service is required when active=azure" })
    }
    for (const verb of verbs) {
      if (!az[verb].model.trim()) {
        errors.push({ field: `codex.azure.${verb}.model`, message: `${verb} deployment name is required when active=azure` })
      }
    }
  } else {
    for (const verb of verbs) {
      if (!block.system[verb].model.trim()) {
        errors.push({ field: `codex.system.${verb}.model`, message: `${verb} model is required when active=system` })
      }
    }
  }
  return errors
}

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
  | { kind: "quiz_scope_planned"; pages: string[] }
  | { kind: "quiz_no_match"; reason: string }

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

/**
 * RunId issued by `spawn_chat_turn` — always prefixed with `chat-` so
 * the same `active_runs` map can hold goal and chat entries side-by-side
 * (spec `Tauri IPC Commands for Chat Turn Lifecycle`).
 */
export type ChatTurnRunId = string

/** Payload of one `chat-stream` Tauri event tick. */
export interface ChatStreamPayload {
  run_id: ChatTurnRunId
  event: VerbEvent
}

/**
 * Coarse outcome classification surfaced from the Rust side. Mirrors
 * `chats::ChatTurnOutcome` and matches `RunLog.outcome` values for the
 * chat mode: `succeeded` when the turn completed cleanly, `cancelled`
 * when the cancel flag fired, `failed` for any other error or panic.
 */
export type ChatTurnOutcome = "succeeded" | "cancelled" | "failed"

/**
 * Payload emitted exactly once on the `chat-terminal` Tauri channel
 * after a chat turn's background thread exits (success / fail / cancel /
 * panic). Frontend uses this to flip `useChatStore.activeTurn` to null,
 * finalize the turn in the transcript, AND record the claude
 * `session_id` so the next `spawnChatTurn` can pass it back for
 * `--resume <id>`. `session_id` is `null` on terminal paths that never
 * reached the init phase (e.g., spawn failure before stream-json
 * init); the frontend keeps any previously known sessionId in that
 * case.
 */
export interface ChatTerminalPayload {
  run_id: ChatTurnRunId
  session_id: string | null
  outcome: ChatTurnOutcome
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
 * Spawn one chat turn in the given vault. Pass `null` for `sessionId`
 * on the first turn of a REPL session; pass the `sessionId` returned by
 * the previous turn for subsequent turns so the backend issues
 * `--resume <id>` to the claude CLI. Returns the new chat run id
 * (always prefixed `chat-`).
 *
 * Rejects with `AppError::Invalid { field: "active_runs" }` when another
 * chat turn is already active in the session; chat turns DO NOT block on
 * a concurrent active goal run (and vice versa).
 */
export async function spawnChatTurn(
  vaultPath: string,
  text: string,
  sessionId: string | null,
): Promise<ChatTurnRunId> {
  return invokeTyped<ChatTurnRunId>("spawn_chat_turn", {
    vaultPath,
    text,
    sessionId,
  })
}

/**
 * Flip the cancel flag for an in-progress chat turn. Idempotent — the
 * call succeeds whether or not the turn has already terminated. The
 * session itself (claude `session_id`) is NOT discarded by cancel; the
 * next `spawnChatTurn` call MAY pass the same session id to resume.
 */
export async function cancelChatTurn(runId: ChatTurnRunId): Promise<void> {
  return invokeTyped<void>("cancel_chat_turn", { runId })
}

// ---- Quiz (v3-app-quiz task 5.2) ------------------------------------------

/** Payload of one `quiz-stream` Tauri event tick (plan or generate). */
export interface QuizStreamPayload {
  run_id: string
  event: VerbEvent
}

/**
 * Terminal payload on `quiz-plan-terminal`. `result.kind` drives the
 * frontend: `scope` → show the page list with confirm/revise controls;
 * `no_match` → show the reason and stop (no generate, no file);
 * `failed`/`cancelled` → surface the failure.
 */
export type QuizPlanResult =
  | { kind: "scope"; pages: string[] }
  | { kind: "no_match"; reason: string }
  | { kind: "failed"; message: string }
  | { kind: "cancelled" }

export interface QuizPlanTerminalPayload {
  run_id: string
  result: QuizPlanResult
}

/**
 * Terminal payload on `quiz-generate-terminal`. On success carries the
 * fence-stripped `quiz_md` (for the answering view, task 5.4),
 * `planned_pages`, and `events_log` (for history persistence, task 5.5).
 */
export type QuizGenerateResult =
  | {
      kind: "succeeded"
      quiz_md: string
      planned_pages: string[]
      events_log: string | null
      /** Persisted attempt path; null if the write failed (non-fatal). */
      quiz_file: string | null
    }
  | { kind: "failed"; message: string }
  | { kind: "cancelled" }

/**
 * Trigger provenance for `spawnQuizGenerate` — mapped server-side to the
 * core `QuizTrigger` for slug + frontmatter (design D4/D7). Goal flow
 * passes `ai_planned` with the topic; the wiki-preview Page flow passes
 * `wiki_preview` with the target page path.
 */
export type QuizTriggerArg =
  | { kind: "ai_planned"; topic: string }
  | { kind: "wiki_preview"; target_page: string }

export interface QuizGenerateTerminalPayload {
  run_id: string
  result: QuizGenerateResult
}

/**
 * Start the quiz plan spawn (Goal flow). Streams `VerbEvent`s on
 * `quiz-stream`; emits one `QuizPlanTerminalPayload` on
 * `quiz-plan-terminal`. Does NOT start generation — the frontend
 * interposes the confirm gate and calls `spawnQuizGenerate` separately.
 */
export async function spawnQuizPlan(
  vaultPath: string,
  topic: string,
): Promise<string> {
  return invokeTyped<string>("spawn_quiz_plan", { vaultPath, topic })
}

/**
 * Start the quiz generate spawn against a confirmed page list. Streams
 * `VerbEvent`s on `quiz-stream`; emits one `QuizGenerateTerminalPayload`
 * on `quiz-generate-terminal`.
 */
export async function spawnQuizGenerate(
  vaultPath: string,
  pages: string[],
  questionCount: number,
  trigger: QuizTriggerArg,
): Promise<string> {
  return invokeTyped<string>("spawn_quiz_generate", {
    vaultPath,
    pages,
    questionCount,
    trigger,
  })
}

/** Flip the cancel flag for a quiz plan/generate run. Idempotent. */
export async function cancelQuiz(runId: string): Promise<void> {
  return invokeTyped<void>("cancel_quiz", { runId })
}

/**
 * One persisted quiz attempt's metadata (task 5.5). The frontend groups
 * these by `slug` (page or topic). `path` opens the attempt markdown;
 * `events_log` drives the view-generation-log affordance.
 */
export interface QuizAttemptMeta {
  slug: string
  quiz_id: string
  trigger: string
  topic: string | null
  target_page: string | null
  events_log: string | null
  path: string
}

/**
 * Scan `<vault>/.codebus/quiz/` and return attempt metadata, newest
 * first. A missing quiz directory yields an empty list.
 */
export async function listQuizAttempts(
  vaultPath: string,
): Promise<QuizAttemptMeta[]> {
  return invokeTyped<QuizAttemptMeta[]>("list_quiz_attempts", { vaultPath })
}

/** Read a persisted quiz attempt's markdown (path must be under quiz/). */
export async function readQuizAttempt(
  vaultPath: string,
  path: string,
): Promise<string> {
  return invokeTyped<string>("read_quiz_attempt", { vaultPath, path })
}

/**
 * Read an attempt's generate-spawn events.jsonl as an ordered
 * `EventEnvelope` list so the view-generation-log affordance can replay
 * it through the existing agent stream rendering. `path` must resolve
 * under the vault `.codebus/` tree (backend rejects otherwise).
 */
export async function readQuizEvents(
  vaultPath: string,
  path: string,
): Promise<EventEnvelope[]> {
  return invokeTyped<EventEnvelope[]>("read_quiz_events", { vaultPath, path })
}

/** The user's choice for a question — the exact spec letters. */
export type Choice = "A" | "B" | "C" | "D"

/** Answering lifecycle. An absent sidecar reads as `"not_started"`. */
export type QuizStatus = "not_started" | "in_progress" | "completed"

/** One answered question (1-based `q`, client-side `correct` grade). */
export interface QuizAnswer {
  q: number
  selected: Choice
  correct: boolean
}

/**
 * The per-attempt progress sidecar's non-derivable state (design D1).
 * Total / answered / correct / score / pass-fail are NOT here — recompute
 * them from `answers` + the attempt markdown.
 */
/** Precise resume position (design D3 final). Optional/legacy-tolerant. */
export interface QuizCursor {
  q: number
  revealed: boolean
}

export interface QuizProgress {
  schema_version: number
  answers: QuizAnswer[]
  status: QuizStatus
  started_at: string | null
  completed_at: string | null
  /**
   * Question the user is viewing + whether it was submitted. Written on
   * every submit and every Next; absent on legacy/prior-build sidecars
   * (callers then fall back to "last answered, revealed").
   */
  cursor?: QuizCursor | null
}

/**
 * Read the progress sidecar for an attempt. An absent or malformed
 * sidecar resolves to the not-started state (never throws for that).
 * `path` must resolve under the vault `.codebus/` tree.
 */
export async function readQuizProgress(
  vaultPath: string,
  path: string,
): Promise<QuizProgress> {
  return invokeTyped<QuizProgress>("read_quiz_progress", { vaultPath, path })
}

/**
 * Atomically persist answering progress to an attempt's sidecar. `path`
 * must resolve under the vault `.codebus/` tree (backend rejects
 * otherwise).
 */
export async function writeQuizProgress(
  vaultPath: string,
  path: string,
  progress: QuizProgress,
): Promise<void> {
  return invokeTyped<void>("write_quiz_progress", { vaultPath, path, progress })
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

/**
 * Resolve the Obsidian vault id (16-char SHA-256 prefix) registered for the
 * vault's wiki directory, or `null` when the vault is not registered in
 * Obsidian. The store calls this once when a vault's wiki loads to decide
 * whether to render `[Open in Obsidian]`. A backend `AppError` (obsidian.json
 * present but unparseable) is fail-soft — callers treat it identically to
 * `null` and hide the button.
 */
export async function getObsidianVaultId(
  vaultPath: string,
): Promise<string | null> {
  return invokeTyped<string | null>("get_obsidian_vault_id", { vaultPath })
}

/**
 * Open the wiki page identified by `slug` in Obsidian. The backend
 * re-resolves the vault id, locates the page file, builds the
 * `obsidian://open?vault=<id>&file=<rel>` URL, and hands it to the OS.
 * Rejects with `AppError::Invalid { field: "obsidian" }` when the vault is
 * no longer registered, or `{ field: "slug" }` when no page matches.
 */
export async function openWikiInObsidian(
  vaultPath: string,
  slug: string,
): Promise<void> {
  return invokeTyped<void>("open_wiki_in_obsidian", { vaultPath, slug })
}
