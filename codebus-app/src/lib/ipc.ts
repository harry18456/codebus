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
