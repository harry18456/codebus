//! OS keyring integration for the Azure API key, with a deterministic
//! `CODEBUS_AZURE_KEY` env fallback.
//!
//! All keyring entries are addressed by `(service, "default")` — the
//! `account` slot is fixed because codebus stores at most one key per
//! profile and exposing an account knob would just create naming
//! ambiguity.
//!
//! `read_azure_key` runs the spec-defined fallback chain:
//!
//! 1. Read the password from the keyring entry `(service, "default")`.
//! 2. If the keyring backend is unavailable OR the entry does not exist,
//!    read the `CODEBUS_AZURE_KEY` env var (non-empty value only).
//! 3. Otherwise return [`KeyringError::EndpointKeyMissing`] — the caller
//!    SHALL surface this to the user and SHALL NOT spawn the agent child.

use keyring::Entry;

/// Fixed `account` value for every codebus keyring entry. Exposing this
/// to users would create naming ambiguity for zero benefit.
pub const KEYRING_ACCOUNT: &str = "default";

/// Env var consulted when the keyring backend is unavailable or empty.
pub const ENV_FALLBACK: &str = "CODEBUS_AZURE_KEY";

#[derive(Debug)]
pub enum KeyringError {
    /// Spawning the agent is impossible because no API key can be resolved
    /// from either source. Error message names both the keyring service
    /// AND the env var so the user knows exactly what to set.
    EndpointKeyMissing {
        service: String,
    },
    /// A keyring backend operation failed for a reason other than
    /// missing-entry (e.g. credential storage write rejected by the OS).
    /// Distinct from `EndpointKeyMissing` so callers can decide whether
    /// to retry / surface separately.
    Backend {
        service: String,
        source: keyring::Error,
    },
}

impl std::fmt::Display for KeyringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyringError::EndpointKeyMissing { service } => write!(
                f,
                "EndpointKeyMissing: no api key found in keyring (service={service}, account={KEYRING_ACCOUNT}) AND env var {ENV_FALLBACK} is unset or empty. Run `codebus config set-key azure` or `export {ENV_FALLBACK}=<your-key>`."
            ),
            KeyringError::Backend { service, source } => {
                write!(f, "keyring backend error for ({service}, {KEYRING_ACCOUNT}): {source}")
            }
        }
    }
}

impl std::error::Error for KeyringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            KeyringError::EndpointKeyMissing { .. } => None,
            KeyringError::Backend { source, .. } => Some(source),
        }
    }
}

/// Pluggable keyring backend. Production uses [`SystemKeyring`] (real OS
/// keyring); tests use [`MockKeyring`] (in-memory `HashMap`). The trait is
/// `pub(crate)` so it does not leak into the public API — external
/// callers SHALL use the top-level `read_azure_key` / `store_azure_key` /
/// `delete_azure_key` functions.
pub(crate) trait KeyringBackend {
    /// `Ok(Some(value))` — entry present.
    /// `Ok(None)` — entry absent OR backend unavailable in a way that is
    /// indistinguishable from absence for read purposes.
    /// `Err(_)` — backend present but errored on the operation.
    fn read(&self, service: &str) -> Result<Option<String>, keyring::Error>;
    fn store(&self, service: &str, value: &str) -> Result<(), keyring::Error>;
    fn delete(&self, service: &str) -> Result<(), keyring::Error>;
}

pub(crate) struct SystemKeyring;

impl KeyringBackend for SystemKeyring {
    fn read(&self, service: &str) -> Result<Option<String>, keyring::Error> {
        let entry = match Entry::new(service, KEYRING_ACCOUNT) {
            Ok(e) => e,
            // PlatformFailure / NoStorageAccess at construction time means
            // the backend itself is unavailable (e.g. headless Linux with
            // no Secret Service). Treat as "absent" so the env fallback
            // chain kicks in.
            Err(keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)) => {
                return Ok(None);
            }
            Err(other) => return Err(other),
        };
        match entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            // Same rationale as construction-time PlatformFailure: degrade
            // to "absent" so the env fallback path can succeed when the
            // user has set CODEBUS_AZURE_KEY.
            Err(keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)) => {
                Ok(None)
            }
            Err(other) => Err(other),
        }
    }

    fn store(&self, service: &str, value: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(service, KEYRING_ACCOUNT)?;
        entry.set_password(value)
    }

    fn delete(&self, service: &str) -> Result<(), keyring::Error> {
        let entry = Entry::new(service, KEYRING_ACCOUNT)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // idempotent
            Err(other) => Err(other),
        }
    }
}

// ---------------------------------------------------------------------------
// Public functions (production callers)
// ---------------------------------------------------------------------------

/// Process-wide lock guarding tests that mutate `CODEBUS_AZURE_KEY`.
/// Exposed `pub(crate)` so tests in sibling modules (e.g.
/// `claude_code::tests`) can serialise alongside `keyring::tests`.
/// Production code SHALL NOT touch this lock.
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Resolve the Azure API key for `service` using the keyring → env fallback
/// chain defined in the module docs. Returns `EndpointKeyMissing` when no
/// source has a value.
pub fn read_azure_key(service: &str) -> Result<String, KeyringError> {
    read_azure_key_with(service, &SystemKeyring)
}

/// Store `value` as the keyring password for `(service, "default")`.
/// Overwrites any existing entry.
pub fn store_azure_key(service: &str, value: &str) -> Result<(), KeyringError> {
    SystemKeyring.store(service, value).map_err(|source| {
        KeyringError::Backend {
            service: service.to_string(),
            source,
        }
    })
}

/// Remove the keyring entry for `(service, "default")`. Idempotent —
/// returns `Ok(())` whether or not the entry existed.
pub fn delete_azure_key(service: &str) -> Result<(), KeyringError> {
    SystemKeyring.delete(service).map_err(|source| {
        KeyringError::Backend {
            service: service.to_string(),
            source,
        }
    })
}

/// Probe the keyring without consulting the `CODEBUS_AZURE_KEY` env
/// fallback. Returns `Ok(Some(value))` when the keyring entry exists,
/// `Ok(None)` when it does not (including when the backend itself is
/// unavailable — there is no keyring state to surface), and `Err` only
/// on a backend operation failure that is NOT "missing entry" or
/// "backend unavailable".
///
/// Used by `codebus config get-key` to report `set` / `unset` based
/// purely on keyring state. Surfacing env-fallback values here would
/// mask a missing keyring entry and confuse `delete-key`'s semantics.
pub fn probe_keyring_only(service: &str) -> Result<Option<String>, KeyringError> {
    SystemKeyring.read(service).map_err(|source| {
        KeyringError::Backend {
            service: service.to_string(),
            source,
        }
    })
}

// ---------------------------------------------------------------------------
// Internals + test backends
// ---------------------------------------------------------------------------

pub(crate) fn read_azure_key_with(
    service: &str,
    backend: &dyn KeyringBackend,
) -> Result<String, KeyringError> {
    match backend.read(service) {
        Ok(Some(v)) if !v.is_empty() => return Ok(v),
        Ok(_) => { /* fall through to env */ }
        Err(source) => {
            return Err(KeyringError::Backend {
                service: service.to_string(),
                source,
            });
        }
    }
    match std::env::var(ENV_FALLBACK) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(KeyringError::EndpointKeyMissing {
            service: service.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-process keyring stand-in. Each variant simulates a different
    /// real-world state without ever touching the OS keyring.
    pub(super) enum MockKeyring {
        /// Entry present with the given value.
        Present(Mutex<HashMap<String, String>>),
        /// Entry not found.
        Absent,
        /// Backend itself is unavailable (e.g. headless Linux). Read
        /// returns `Ok(None)` so the env fallback can take over.
        Unavailable,
    }

    impl MockKeyring {
        fn with(service: &str, value: &str) -> Self {
            let mut m = HashMap::new();
            m.insert(service.to_string(), value.to_string());
            MockKeyring::Present(Mutex::new(m))
        }
    }

    impl KeyringBackend for MockKeyring {
        fn read(&self, service: &str) -> Result<Option<String>, keyring::Error> {
            match self {
                MockKeyring::Present(m) => Ok(m.lock().unwrap().get(service).cloned()),
                MockKeyring::Absent => Ok(None),
                MockKeyring::Unavailable => Ok(None),
            }
        }
        fn store(&self, service: &str, value: &str) -> Result<(), keyring::Error> {
            match self {
                MockKeyring::Present(m) => {
                    m.lock().unwrap().insert(service.to_string(), value.to_string());
                    Ok(())
                }
                MockKeyring::Absent | MockKeyring::Unavailable => Err(keyring::Error::NoEntry),
            }
        }
        fn delete(&self, service: &str) -> Result<(), keyring::Error> {
            match self {
                MockKeyring::Present(m) => {
                    m.lock().unwrap().remove(service);
                    Ok(())
                }
                MockKeyring::Absent | MockKeyring::Unavailable => Ok(()),
            }
        }
    }

    fn with_env<F: FnOnce()>(value: Option<&str>, f: F) {
        let _g = super::TEST_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var(ENV_FALLBACK).ok();
        unsafe {
            match value {
                Some(v) => std::env::set_var(ENV_FALLBACK, v),
                None => std::env::remove_var(ENV_FALLBACK),
            }
        }
        f();
        unsafe {
            match prev {
                Some(v) => std::env::set_var(ENV_FALLBACK, v),
                None => std::env::remove_var(ENV_FALLBACK),
            }
        }
    }

    /// Spec: keyring hit → env var NOT consulted (keyring wins).
    #[test]
    fn keyring_hit_does_not_consult_env() {
        let backend = MockKeyring::with("codebus-azure", "sk-from-keyring");
        with_env(Some("sk-from-env-should-be-ignored"), || {
            let key = read_azure_key_with("codebus-azure", &backend).unwrap();
            assert_eq!(key, "sk-from-keyring");
        });
    }

    /// Spec: keyring absent + env set → env fallback wins.
    #[test]
    fn keyring_absent_falls_back_to_env() {
        let backend = MockKeyring::Absent;
        with_env(Some("sk-from-env"), || {
            let key = read_azure_key_with("codebus-azure", &backend).unwrap();
            assert_eq!(key, "sk-from-env");
        });
    }

    /// Spec: keyring backend unavailable + env set → env fallback wins.
    #[test]
    fn keyring_unavailable_falls_back_to_env() {
        let backend = MockKeyring::Unavailable;
        with_env(Some("sk-from-env"), || {
            let key = read_azure_key_with("codebus-azure", &backend).unwrap();
            assert_eq!(key, "sk-from-env");
        });
    }

    /// Spec: both sources absent → `EndpointKeyMissing` AND error message
    /// names the service AND the env var.
    #[test]
    fn neither_source_returns_endpoint_key_missing_naming_both_sources() {
        let backend = MockKeyring::Absent;
        with_env(None, || {
            let err = read_azure_key_with("codebus-azure", &backend).unwrap_err();
            match err {
                KeyringError::EndpointKeyMissing { ref service } => {
                    assert_eq!(service, "codebus-azure");
                }
                other => panic!("expected EndpointKeyMissing, got {other:?}"),
            }
            let msg = format!("{err}");
            assert!(msg.contains("codebus-azure"), "missing service: {msg}");
            assert!(msg.contains("CODEBUS_AZURE_KEY"), "missing env var: {msg}");
        });
    }

    /// Spec: empty env value treated as absent (does not satisfy fallback).
    #[test]
    fn empty_env_value_treated_as_absent() {
        let backend = MockKeyring::Absent;
        with_env(Some(""), || {
            let err = read_azure_key_with("codebus-azure", &backend).unwrap_err();
            assert!(matches!(err, KeyringError::EndpointKeyMissing { .. }));
        });
    }

    /// Spec: empty keyring value also treated as absent.
    #[test]
    fn empty_keyring_value_falls_through_to_env() {
        let backend = MockKeyring::with("codebus-azure", "");
        with_env(Some("sk-from-env"), || {
            let key = read_azure_key_with("codebus-azure", &backend).unwrap();
            assert_eq!(key, "sk-from-env");
        });
    }
}
