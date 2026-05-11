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
}
