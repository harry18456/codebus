//! Tauri keyring IPC commands. Wraps the `keyring` crate (3.x) to
//! expose three commands — `keyring_set` / `keyring_get` /
//! `keyring_delete` — that store LLM API keys in the OS-native
//! keychain (macOS Keychain Services, Windows Credential Manager,
//! Linux Secret Service / KWallet).
//!
//! Backs spec `keyring-integration` Requirements:
//!   - Tauri keyring plugin commands
//!   - API keys never written to disk or audit logs
//!
//! Trust boundary contract:
//!   - `provider_id` MUST match `^[a-z][a-z0-9-]{2,40}$` (validated
//!     host-side; the renderer cannot bypass).
//!   - The host-side namespace `codebus.<provider_id>.api_key` is
//!     constructed only after validation passes — a rejected id
//!     never touches the OS keychain backend.
//!   - Windows reserved device names (con/prn/aux/nul/comN/lptN)
//!     are rejected as defense-in-depth even if they would match
//!     the regex.
//!   - API keys are never logged. Only opaque error codes
//!     (`KEYRING_INVALID_PROVIDER_ID` / `KEYRING_ENTRY_MISSING` /
//!     `KEYRING_BACKEND_ERROR`) cross the IPC boundary.
//!
//! Red-team coverage in `tests/keyring_redteam.rs`. Cross-platform
//! happy-path PoC in `examples/keyring_poc.rs`.

use serde::Serialize;

/// Logical user name for every keyring entry. The keyring crate's
/// `Entry::new(service, username)` API requires both; we pin
/// username and vary service per provider so credentials stay
/// scoped to this app and visible in the OS keychain UI under a
/// stable identity.
const KEYRING_USER: &str = "codebus";

const NAMESPACE_PREFIX: &str = "codebus.";
const NAMESPACE_SUFFIX: &str = ".api_key";

const PROVIDER_ID_MIN_LEN: usize = 3;
const PROVIDER_ID_MAX_LEN: usize = 41;

pub const KEYRING_INVALID_PROVIDER_ID: &str = "KEYRING_INVALID_PROVIDER_ID";
pub const KEYRING_ENTRY_MISSING: &str = "KEYRING_ENTRY_MISSING";
pub const KEYRING_BACKEND_ERROR: &str = "KEYRING_BACKEND_ERROR";

/// Windows treats these stems as device handles regardless of
/// extension. Even though the OS keychain on Windows is the Credential
/// Manager (not the filesystem), we reject these names so
/// `provider_id` is also safe to surface in any future tooling that
/// might hit the filesystem (logs, error messages, debug exports).
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
    "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8",
    "lpt9",
];

#[derive(Debug, Clone, Serialize)]
pub struct KeyringResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl KeyringResponse {
    fn ok() -> Self {
        Self { ok: true, api_key: None, code: None }
    }

    fn err(code: &'static str) -> Self {
        Self { ok: false, api_key: None, code: Some(code.to_string()) }
    }

    fn ok_with_key(api_key: String) -> Self {
        Self { ok: true, api_key: Some(api_key), code: None }
    }
}

/// Validate `provider_id` against `^[a-z][a-z0-9-]{2,40}$` plus
/// Windows reserved-name rejection. Pure function; no OS calls.
pub fn validate_provider_id(id: &str) -> Result<(), &'static str> {
    let len = id.len();
    if !(PROVIDER_ID_MIN_LEN..=PROVIDER_ID_MAX_LEN).contains(&len) {
        return Err(KEYRING_INVALID_PROVIDER_ID);
    }
    let mut bytes = id.bytes();
    let first = bytes.next().ok_or(KEYRING_INVALID_PROVIDER_ID)?;
    if !first.is_ascii_lowercase() {
        return Err(KEYRING_INVALID_PROVIDER_ID);
    }
    for b in bytes {
        let ok = b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
        if !ok {
            return Err(KEYRING_INVALID_PROVIDER_ID);
        }
    }
    if WINDOWS_RESERVED_NAMES.iter().any(|r| *r == id) {
        return Err(KEYRING_INVALID_PROVIDER_ID);
    }
    Ok(())
}

fn canonical_service_name(provider_id: &str) -> String {
    let mut s = String::with_capacity(
        NAMESPACE_PREFIX.len() + provider_id.len() + NAMESPACE_SUFFIX.len(),
    );
    s.push_str(NAMESPACE_PREFIX);
    s.push_str(provider_id);
    s.push_str(NAMESPACE_SUFFIX);
    s
}

fn entry_for(provider_id: &str) -> Result<keyring::Entry, &'static str> {
    let service = canonical_service_name(provider_id);
    keyring::Entry::new(&service, KEYRING_USER).map_err(|e| {
        log::warn!("keyring::Entry::new failed for {service}: {e}");
        KEYRING_BACKEND_ERROR
    })
}

pub fn keyring_set_inner(provider_id: &str, api_key: &str) -> KeyringResponse {
    if let Err(code) = validate_provider_id(provider_id) {
        return KeyringResponse::err(code);
    }
    let entry = match entry_for(provider_id) {
        Ok(e) => e,
        Err(code) => return KeyringResponse::err(code),
    };
    match entry.set_password(api_key) {
        Ok(()) => KeyringResponse::ok(),
        Err(e) => {
            log::warn!("keyring set_password failed for {provider_id}: {e}");
            KeyringResponse::err(KEYRING_BACKEND_ERROR)
        }
    }
}

pub fn keyring_get_inner(provider_id: &str) -> KeyringResponse {
    if let Err(code) = validate_provider_id(provider_id) {
        return KeyringResponse::err(code);
    }
    let entry = match entry_for(provider_id) {
        Ok(e) => e,
        Err(code) => return KeyringResponse::err(code),
    };
    match entry.get_password() {
        Ok(value) => KeyringResponse::ok_with_key(value),
        Err(keyring::Error::NoEntry) => KeyringResponse::err(KEYRING_ENTRY_MISSING),
        Err(e) => {
            log::warn!("keyring get_password failed for {provider_id}: {e}");
            KeyringResponse::err(KEYRING_BACKEND_ERROR)
        }
    }
}

pub fn keyring_delete_inner(provider_id: &str) -> KeyringResponse {
    if let Err(code) = validate_provider_id(provider_id) {
        return KeyringResponse::err(code);
    }
    let entry = match entry_for(provider_id) {
        Ok(e) => e,
        Err(code) => return KeyringResponse::err(code),
    };
    match entry.delete_credential() {
        Ok(()) => KeyringResponse::ok(),
        // Deleting a non-existent entry is success — idempotent contract
        // tied to "delete from never-set id returns success" in tasks.md
        // task 1.4. The renderer can treat both Ok(_) and NoEntry as
        // "the entry is not present after this call".
        Err(keyring::Error::NoEntry) => KeyringResponse::ok(),
        Err(e) => {
            log::warn!("keyring delete_credential failed for {provider_id}: {e}");
            KeyringResponse::err(KEYRING_BACKEND_ERROR)
        }
    }
}

#[tauri::command]
pub async fn keyring_set(provider_id: String, api_key: String) -> KeyringResponse {
    keyring_set_inner(&provider_id, &api_key)
}

#[tauri::command]
pub async fn keyring_get(provider_id: String) -> KeyringResponse {
    keyring_get_inner(&provider_id)
}

#[tauri::command]
pub async fn keyring_delete(provider_id: String) -> KeyringResponse {
    keyring_delete_inner(&provider_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_service_name_format() {
        assert_eq!(
            canonical_service_name("openai-default"),
            "codebus.openai-default.api_key"
        );
    }

    #[test]
    fn validate_provider_id_accepts_min_length() {
        assert!(validate_provider_id("abc").is_ok());
    }

    #[test]
    fn validate_provider_id_accepts_max_length() {
        let s = "a".to_string() + &"a".repeat(40);
        assert_eq!(s.len(), 41);
        assert!(validate_provider_id(&s).is_ok());
    }

    #[test]
    fn validate_provider_id_rejects_at_42_chars() {
        let s = "a".to_string() + &"a".repeat(41);
        assert_eq!(s.len(), 42);
        assert!(validate_provider_id(&s).is_err());
    }
}
