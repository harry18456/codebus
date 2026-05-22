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

use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};
use serde::{Deserialize, Serialize};

use super::IpcResult;
use crate::error::AppError;

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
pub async fn set_endpoint_key(service: String, key: String) -> IpcResult<()> {
    let service = validate_service(service)?;
    store_azure_key(&service, &key).map_err(AppError::from)
}

#[tauri::command]
pub async fn get_endpoint_key(service: String) -> IpcResult<KeyStatus> {
    let service = validate_service(service)?;
    match probe_keyring_only(&service).map_err(AppError::from)? {
        Some(_) => Ok(KeyStatus::Set),
        None => Ok(KeyStatus::Unset),
    }
}

#[tauri::command]
pub async fn delete_endpoint_key(service: String) -> IpcResult<()> {
    let service = validate_service(service)?;
    delete_azure_key(&service).map_err(AppError::from)
}

/// Validate the keyring `service` name supplied by the Settings editor.
///
/// The editor passes the active provider's `azure.keyring_service` directly
/// (defaulting to `codebus-claude-azure` / `codebus-codex-azure`), so claude
/// and codex keys land in distinct keyring entries and the GUI writes to
/// exactly the service the user sees — no stale on-disk lookup. An empty /
/// whitespace-only name is rejected rather than silently writing to a blank
/// keyring entry.
fn validate_service(service: String) -> IpcResult<String> {
    let trimmed = service.trim();
    if trimmed.is_empty() {
        return Err(AppError::Invalid {
            field: "service".into(),
            message: "keyring service name must not be empty".into(),
        });
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty / whitespace-only service name is rejected with
    /// `AppError::Invalid { field: "service" }` rather than writing to a
    /// blank keyring entry.
    #[test]
    fn validate_service_rejects_empty() {
        for blank in ["", "   ", "\t"] {
            let err = validate_service(blank.into()).expect_err(blank);
            let AppError::Invalid { field, .. } = err else {
                panic!("expected Invalid for {blank:?}, got {err:?}");
            };
            assert_eq!(field, "service");
        }
    }

    /// A real service name passes through trimmed — claude and codex pass
    /// their own distinct defaults so keys never collide.
    #[test]
    fn validate_service_accepts_and_trims() {
        assert_eq!(validate_service("codebus-codex-azure".into()).unwrap(), "codebus-codex-azure");
        assert_eq!(validate_service("  codebus-claude-azure ".into()).unwrap(), "codebus-claude-azure");
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
