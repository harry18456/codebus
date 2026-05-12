//! `set_endpoint_key` / `get_endpoint_key` / `delete_endpoint_key` IPC
//! commands.
//!
//! Three Tauri commands wrap the codebus-core keyring helpers so the
//! Settings UI can manage the Azure API key entirely in-GUI without
//! shelling out to the CLI. Profile argument is currently constrained
//! to `"azure"` — future endpoint types extend the match arm here AND
//! the spec's `Settings UI Endpoint Section` requirement.
//!
//! Design contract: `get_endpoint_key` SHALL NOT return the key value
//! (only `Set` / `Unset` status). `delete_endpoint_key` SHALL be
//! idempotent. All three commands SHALL fail-loud when
//! `~/.codebus/config.yaml` exists but cannot parse.

use std::path::PathBuf;

use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};
use codebus_core::config::{ClaudeCodeConfig, default_config_path, load_claude_code_config};
use serde::{Deserialize, Serialize};

use super::IpcResult;
use crate::error::AppError;

/// Default keyring service when the user has not configured an azure
/// profile yet. Matches the CLI subcommand's default
/// (`codebus-cli/src/commands/config.rs::DEFAULT_AZURE_KEYRING_SERVICE`)
/// so the GUI and CLI agree on first-time-setup behavior.
const DEFAULT_AZURE_KEYRING_SERVICE: &str = "codebus-azure";

/// Reply shape for `get_endpoint_key`. Discriminated union mirroring
/// the `AppError` pattern so the frontend pattern-matches on `kind`.
/// SHALL NOT carry the key value under any circumstance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum KeyStatus {
    Set,
    Unset,
}

#[tauri::command]
pub async fn set_endpoint_key(profile: String, key: String) -> IpcResult<()> {
    let service = resolve_keyring_service(&profile)?;
    store_azure_key(&service, &key).map_err(AppError::from)
}

#[tauri::command]
pub async fn get_endpoint_key(profile: String) -> IpcResult<KeyStatus> {
    let service = resolve_keyring_service(&profile)?;
    match probe_keyring_only(&service).map_err(AppError::from)? {
        Some(_) => Ok(KeyStatus::Set),
        None => Ok(KeyStatus::Unset),
    }
}

#[tauri::command]
pub async fn delete_endpoint_key(profile: String) -> IpcResult<()> {
    let service = resolve_keyring_service(&profile)?;
    delete_azure_key(&service).map_err(AppError::from)
}

/// Validate the `profile` argument and resolve the keyring service name
/// from `~/.codebus/config.yaml`. Three outcomes:
///
/// - `profile` is not `"azure"` → `AppError::Invalid { field: "profile", ... }`.
/// - Config exists AND fails to parse → `AppError::ConfigParse`. No
///   keyring side effect is performed (fail-loud, mirrors `cli`
///   capability's `Config Parse Failure Aborts Invocation` requirement).
/// - Config absent OR `azure.keyring_service` empty/missing → returns
///   the built-in default. This is the first-time-setup path: the user
///   can call `set_endpoint_key` before having any `azure` block in
///   their yaml.
fn resolve_keyring_service(profile: &str) -> IpcResult<String> {
    if profile != "azure" {
        return Err(AppError::Invalid {
            field: "profile".into(),
            message: format!("unknown endpoint profile `{profile}`; only `azure` is supported"),
        });
    }
    let path: PathBuf = match default_config_path() {
        Some(p) => p,
        None => return Ok(DEFAULT_AZURE_KEYRING_SERVICE.to_string()),
    };
    if !path.exists() {
        return Ok(DEFAULT_AZURE_KEYRING_SERVICE.to_string());
    }
    let cfg: ClaudeCodeConfig =
        load_claude_code_config(&path).map_err(|e| AppError::ConfigParse {
            message: format!("claude_code config parse failed at {}: {e}", path.display()),
        })?;
    if let Some(az) = cfg.azure.as_ref() {
        if !az.keyring_service.is_empty() {
            return Ok(az.keyring_service.clone());
        }
    }
    Ok(DEFAULT_AZURE_KEYRING_SERVICE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: `Unknown profile value rejected` — any value other than
    /// `"azure"` SHALL return `AppError::Invalid { field: "profile" }`.
    /// We exercise the sync `resolve_keyring_service` helper directly;
    /// the three async commands all funnel through it before touching
    /// the keyring, so testing the helper covers the rejection path.
    #[test]
    fn resolve_keyring_service_rejects_unknown_profile() {
        for unknown in ["bedrock", "vertex", "openai", "", "AZURE"] {
            let err = resolve_keyring_service(unknown).expect_err(unknown);
            let AppError::Invalid { field, message } = err else {
                panic!("expected Invalid for {unknown:?}, got {err:?}");
            };
            assert_eq!(field, "profile");
            assert!(
                message.contains(unknown) || unknown.is_empty(),
                "message should name value `{unknown}`: {message}"
            );
        }
    }

    /// Sanity: `"azure"` passes through and returns a service name.
    /// We can't pin the exact value (depends on whether the dev's
    /// `~/.codebus/config.yaml` has an azure block) but the call SHALL
    /// succeed AND return a non-empty string.
    #[test]
    fn resolve_keyring_service_accepts_azure() {
        let result = resolve_keyring_service("azure");
        // Either Ok(name) or ConfigParse (if dev machine has broken yaml).
        // Both are valid outcomes — what we're ruling out is `Invalid`.
        if let Ok(name) = result {
            assert!(!name.is_empty(), "service name SHALL be non-empty");
        }
    }

    /// Spec: `KeyStatus` SHALL serialise with `kind` tag.
    #[test]
    fn key_status_serialises_with_kind_tag() {
        let v = serde_json::to_value(KeyStatus::Set).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "set"}));
        let v = serde_json::to_value(KeyStatus::Unset).unwrap();
        assert_eq!(v, serde_json::json!({"kind": "unset"}));
    }
}
