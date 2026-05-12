//! Single discriminated union surfaced to the frontend by every IPC command.
//!
//! Serde-tagged with `kind` (snake_case) so the TypeScript wrapper can
//! pattern-match without inferring the shape. Per design.md "IPC error 型別:
//! single discriminated union".

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppError {
    #[error("io: {message}")]
    Io { message: String },

    #[error("config parse: {message}")]
    ConfigParse { message: String },

    #[error("vault not found: {path}")]
    VaultNotFound { path: String },

    #[error("vault already exists: {path}")]
    VaultAlreadyExists { path: String },

    #[error("invalid {field}: {message}")]
    Invalid { field: String, message: String },

    #[error("internal: {message}")]
    Internal { message: String },
}

impl AppError {
    pub fn io(err: impl std::fmt::Display) -> Self {
        AppError::Io {
            message: err.to_string(),
        }
    }

    pub fn config_parse(err: impl std::fmt::Display) -> Self {
        AppError::ConfigParse {
            message: err.to_string(),
        }
    }

    pub fn internal(err: impl std::fmt::Display) -> Self {
        AppError::Internal {
            message: err.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::io(err)
    }
}

impl From<codebus_core::config::keyring::KeyringError> for AppError {
    /// `KeyringError::Backend` surfaces as `Internal` so the frontend can
    /// distinguish it from validation / parse errors and render an
    /// "underlying keyring backend failed" toast. `EndpointKeyMissing` is
    /// not produced by the store / delete code paths used in the keyring
    /// IPC commands (those bypass the env-fallback chain in `read_azure_key`),
    /// but we map it defensively to `Internal` in case future code routes
    /// it through this conversion.
    fn from(err: codebus_core::config::keyring::KeyringError) -> Self {
        AppError::Internal {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn io_variant_serializes_with_kind_and_message() {
        let err = AppError::Io {
            message: "disk full".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({"kind": "io", "message": "disk full"}),
        );
    }

    #[test]
    fn config_parse_variant_serializes_with_kind_and_message() {
        let err = AppError::ConfigParse {
            message: "bad yaml".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({"kind": "config_parse", "message": "bad yaml"}),
        );
    }

    #[test]
    fn vault_not_found_variant_has_path_field() {
        let err = AppError::VaultNotFound {
            path: "/tmp/x".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({"kind": "vault_not_found", "path": "/tmp/x"}),
        );
    }

    #[test]
    fn vault_already_exists_variant_has_path_field() {
        let err = AppError::VaultAlreadyExists {
            path: "/tmp/x".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({"kind": "vault_already_exists", "path": "/tmp/x"}),
        );
    }

    #[test]
    fn invalid_variant_has_field_and_message() {
        let err = AppError::Invalid {
            field: "app.quiz.pass_threshold".into(),
            message: "must be 50–100".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({
                "kind": "invalid",
                "field": "app.quiz.pass_threshold",
                "message": "must be 50–100",
            }),
        );
    }

    #[test]
    fn internal_variant_serializes_with_kind_and_message() {
        let err = AppError::Internal {
            message: "oops".into(),
        };
        assert_eq!(
            to_value(&err).unwrap(),
            json!({"kind": "internal", "message": "oops"}),
        );
    }

    #[test]
    fn round_trips_through_json() {
        let err = AppError::Invalid {
            field: "x".into(),
            message: "y".into(),
        };
        let s = serde_json::to_string(&err).unwrap();
        let parsed: AppError = serde_json::from_str(&s).unwrap();
        assert!(matches!(parsed, AppError::Invalid { .. }));
    }

    #[test]
    fn from_io_error_maps_to_io_variant() {
        let io: std::io::Error = std::io::ErrorKind::NotFound.into();
        let app: AppError = io.into();
        assert!(matches!(app, AppError::Io { .. }));
    }

    /// Spec: `app-shell / AppError Discriminated Union` — keyring backend
    /// failures surface as `Internal { message }` so the frontend can
    /// render a generic "keyring unavailable" toast.
    #[test]
    fn keyring_backend_failure_maps_to_internal() {
        use codebus_core::config::keyring::KeyringError;
        // Use the public `EndpointKeyMissing` variant — it's constructible
        // without a `keyring::Error` instance (which is opaque) and exercises
        // the same `From` impl path. The mapping target is identical for both
        // variants of `KeyringError`.
        let err = KeyringError::EndpointKeyMissing {
            service: "codebus-test".into(),
        };
        let app: AppError = err.into();
        let AppError::Internal { message } = app else {
            panic!("expected Internal variant");
        };
        assert!(
            message.contains("codebus-test"),
            "Internal message should preserve service name: {message}"
        );
    }
}
