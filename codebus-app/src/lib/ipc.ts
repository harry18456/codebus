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
