//! Tauri-to-sidecar provider key dispatch — D-033 Change B §2.5.
//!
//! After the sidecar handshake completes, the host needs to push the
//! API keys collected from the OS keychain into sidecar memory via
//! `POST /internal/startup-config`. This module owns that wiring:
//!
//! - `collect_provider_keys` — pure read-side over `keyring::*_inner`.
//!   Skips `KEYRING_ENTRY_MISSING` entries (the user has not stored
//!   that provider's key yet); aborts on `KEYRING_INVALID_PROVIDER_ID`
//!   so a renderer-supplied bad id never produces a partial dict.
//! - `push_startup_config` — POSTs to the sidecar loopback with
//!   bearer auth. Retries the network attempt once on transport
//!   failure; passes through HTTP status errors (the sidecar's
//!   `STARTUP_ALREADY_CONFIGURED` 409 is non-retryable).
//! - `push_startup_config_cmd` — Tauri command facade; the renderer
//!   supplies `provider_ids` (typically derived from the persisted
//!   `llm.providers[]` config or the onboarding wizard's submission).
//!
//! Backs SHALL clauses in
//! `openspec/changes/provider-settings-and-onboarding/specs/keyring-integration/spec.md`
//!   Requirement: Tauri-to-sidecar startup key injection
//!
//! Trust boundary contract:
//!   - The bearer + port come from `SidecarState::handshake` (set
//!     during `sidecar_handshake`); the renderer cannot supply them.
//!   - api_key values cross only over the loopback HTTP boundary;
//!     they are never logged here and the error variants carry
//!     codes / status numbers, never raw payload.

use std::collections::HashMap;
use std::time::Duration;

use crate::keyring::{
    keyring_get_inner, KEYRING_BACKEND_ERROR, KEYRING_ENTRY_MISSING,
    KEYRING_INVALID_PROVIDER_ID,
};
use crate::SidecarState;

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("invalid provider_id from renderer")]
    InvalidProviderId,
    #[error("keyring backend error")]
    KeyringBackend,
    #[error("handshake not yet completed")]
    HandshakeMissing,
    #[error("transport error after retry: {0}")]
    Transport(String),
    #[error("sidecar rejected: HTTP {0}")]
    HttpStatus(u16),
}

impl DispatchError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidProviderId => "INVALID_PROVIDER_ID",
            Self::KeyringBackend => "KEYRING_BACKEND_ERROR",
            Self::HandshakeMissing => "HANDSHAKE_MISSING",
            Self::Transport(_) => "TRANSPORT_ERROR",
            Self::HttpStatus(_) => "SIDECAR_REJECTED",
        }
    }
}

/// Walk `provider_ids` through the keyring backend and assemble the
/// `{ provider_id: api_key }` map that `POST /internal/startup-config`
/// accepts. Missing entries are silently skipped (the user simply
/// has not configured that provider yet); invalid ids fail the whole
/// collect so a partial dict is never produced.
pub fn collect_provider_keys(
    provider_ids: &[String],
) -> Result<HashMap<String, String>, DispatchError> {
    let mut out = HashMap::with_capacity(provider_ids.len());
    for id in provider_ids {
        let resp = keyring_get_inner(id);
        if resp.ok {
            if let Some(value) = resp.api_key {
                out.insert(id.clone(), value);
            }
            continue;
        }
        match resp.code.as_deref() {
            Some(KEYRING_ENTRY_MISSING) => continue,
            Some(KEYRING_INVALID_PROVIDER_ID) => return Err(DispatchError::InvalidProviderId),
            Some(KEYRING_BACKEND_ERROR) | _ => return Err(DispatchError::KeyringBackend),
        }
    }
    Ok(out)
}

/// POST `{ provider_keys: ... }` to the sidecar's
/// `/internal/startup-config` endpoint with bearer auth. Retries the
/// network attempt once on transport failure; HTTP status errors
/// (non-2xx) are returned without retry — the sidecar's
/// `STARTUP_ALREADY_CONFIGURED` 409 is intentionally non-idempotent.
pub async fn push_startup_config(
    base_url: &str,
    bearer: &str,
    keys: &HashMap<String, String>,
) -> Result<(), DispatchError> {
    let body = serde_json::json!({ "provider_keys": keys });
    let url = format!("{base_url}/internal/startup-config");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| DispatchError::Transport(e.to_string()))?;

    let mut last_transport_err: Option<String> = None;
    for attempt in 0..2 {
        match client
            .post(&url)
            .bearer_auth(bearer)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => return Err(DispatchError::HttpStatus(resp.status().as_u16())),
            Err(e) => {
                log::warn!(
                    "push_startup_config attempt {} transport error: {e}",
                    attempt + 1
                );
                last_transport_err = Some(e.to_string());
            }
        }
    }
    Err(DispatchError::Transport(
        last_transport_err.unwrap_or_else(|| "unknown".to_string()),
    ))
}

#[tauri::command]
pub async fn push_startup_config_cmd(
    state: tauri::State<'_, SidecarState>,
    provider_ids: Vec<String>,
) -> Result<(), String> {
    let handshake = {
        let guard = state
            .handshake
            .lock()
            .map_err(|e| format!("handshake mutex poisoned: {e}"))?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| DispatchError::HandshakeMissing.code().to_string())?
    };

    let keys = collect_provider_keys(&provider_ids).map_err(|e| e.code().to_string())?;
    let base_url = format!("http://127.0.0.1:{}", handshake.port);
    push_startup_config(&base_url, &handshake.bearer, &keys)
        .await
        .map_err(|e| e.code().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring::{keyring_delete_inner, keyring_set_inner};

    fn unique_id(suffix: &str) -> String {
        format!("disp-{}-{}", std::process::id(), suffix)
    }

    struct CleanupGuard(String);
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            let _ = keyring_delete_inner(&self.0);
        }
    }

    #[test]
    fn collect_returns_empty_for_no_ids() {
        let keys = collect_provider_keys(&[]).expect("empty input must be Ok");
        assert!(keys.is_empty());
    }

    #[test]
    fn collect_skips_missing_entries() {
        let id = unique_id("skip");
        // Ensure id is not in keyring (no prior set).
        let keys = collect_provider_keys(&[id]).expect("missing must be skipped, not error");
        assert!(keys.is_empty());
    }

    #[test]
    fn collect_returns_set_value() {
        let id = unique_id("got");
        let _g = CleanupGuard(id.clone());
        assert!(keyring_set_inner(&id, "sk-collect-value").ok);

        let keys = collect_provider_keys(&[id.clone()]).expect("present id must collect");
        assert_eq!(keys.get(&id).map(String::as_str), Some("sk-collect-value"));
    }

    #[test]
    fn collect_rejects_invalid_id() {
        let bad = "BAD_ID".to_string();
        let err = collect_provider_keys(&[bad]).expect_err("invalid id must abort collect");
        assert!(matches!(err, DispatchError::InvalidProviderId));
    }

    #[test]
    fn collect_partial_dict_aborts_on_invalid() {
        // First id is valid + set; second is invalid. The whole call
        // must abort — never produce a partial dict.
        let ok_id = unique_id("ok");
        let bad_id = "INVALID".to_string();
        let _g = CleanupGuard(ok_id.clone());
        assert!(keyring_set_inner(&ok_id, "sk").ok);

        let err = collect_provider_keys(&[ok_id, bad_id])
            .expect_err("invalid id mid-stream must abort");
        assert!(matches!(err, DispatchError::InvalidProviderId));
    }
}
