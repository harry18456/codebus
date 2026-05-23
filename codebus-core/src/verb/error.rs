//! `VerbError` — the error variants returned by `verb::{goal,query,fix}::run_*`.
//!
//! CLI thin wrappers in `codebus-cli/src/commands/{goal,query,fix}.rs` `match`
//! exhaustively to translate variants into the existing per-verb exit code
//! policy:
//!
//! - `VaultMissing { path }` → exit 2 (query / fix); `run_goal` SHALL
//!   auto-init and never return this variant
//! - `ConfigParse { source }` → exit 2 (fail-loud per `cli` spec)
//! - `Spawn { source }` → exit 1
//! - `Cancelled` → SHALL NOT occur on CLI paths (CLI passes `cancel: None`)
//! - `Internal { message }` → exit 1
//!
//! GUI (`v3-app-workspace-goal`) translates the same variants into UI states:
//! Cancelled → cancelled detail view; VaultMissing → toast + back to lobby;
//! ConfigParse / Spawn / Internal → inline error with diagnostic.

use crate::config::{ConfigLoadError, KeyringError};
use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by any of `verb::goal::run_goal` / `verb::query::run_query`
/// / `verb::fix::run_fix`.
#[derive(Debug, Error)]
pub enum VerbError {
    /// `<repo>/.codebus/` does not exist. Returned by query / fix only;
    /// goal SHALL auto-init instead.
    #[error("vault not found at {path}; run `codebus init` first")]
    VaultMissing { path: PathBuf },

    /// `~/.codebus/config.yaml` exists but fails to parse. `which`
    /// identifies the section (`"claude_code"`, `"log"`, `"lint.fix"`,
    /// `"pii"`) so CLI thin wrappers can emit a section-specific stderr
    /// message preserving byte-equivalent output.
    #[error("{which} config parse failed: {source}")]
    ConfigParse {
        which: &'static str,
        #[source]
        source: ConfigLoadError,
    },

    /// Azure profile API key could not be retrieved from the OS keyring
    /// or env fallback chain. Maps to CLI exit code 3 per the existing
    /// per-verb policy.
    #[error("{source}")]
    KeyringMissing {
        #[source]
        source: KeyringError,
    },

    /// `agent::invoke` returned an `io::Result::Err` — the agent binary
    /// (claude / codex) missing from PATH, fork failure, or similar
    /// process-level error. Provider-neutral since the dispatch layer may
    /// spawn either backend. (Windows note: an npm `.cmd` shim such as
    /// `codex.cmd` is not resolved by a bare `Command::new("codex")`; set
    /// `CODEBUS_CODEX_BIN` / `CODEBUS_CLAUDE_BIN` to the full path.)
    #[error("spawn agent: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },

    /// The cancel signal flag was observed flipped to true during the
    /// run. CLI thin wrappers never set a cancel signal so this variant
    /// only surfaces in GUI callers.
    #[error("cancelled by caller")]
    Cancelled,

    /// The agent child launched but exited non-zero (e.g. codex `exec
    /// resume` rejecting a cross-provider switch). Distinct from `Spawn`
    /// (which is a launch failure) so callers can surface "the turn failed"
    /// rather than silently treating a non-zero exit as success.
    #[error("agent exited with non-zero status{}", .exit_code.map(|c| format!(" ({c})")).unwrap_or_default())]
    AgentFailed { exit_code: Option<i32> },

    /// Catch-all for unrecoverable failures that don't fit the other
    /// variants (e.g., git2 errors during auto_commit, filesystem
    /// failures during raw mirror re-sync).
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl VerbError {
    /// Map the error to the CLI exit code per the existing policy. Used
    /// by thin wrappers — kept as a method here so the mapping has a
    /// single source of truth.
    pub fn cli_exit_code(&self) -> u8 {
        match self {
            VerbError::VaultMissing { .. } => 2,
            VerbError::ConfigParse { .. } => 2,
            VerbError::KeyringMissing { .. } => 3,
            VerbError::Spawn { .. } => 1,
            VerbError::Cancelled => 0,
            VerbError::AgentFailed { .. } => 1,
            VerbError::Internal { .. } => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_missing_displays_path() {
        let err = VerbError::VaultMissing {
            path: PathBuf::from("/tmp/repo/.codebus"),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("/tmp/repo/.codebus"));
        assert!(rendered.contains("run `codebus init` first"));
    }

    #[test]
    fn spawn_wraps_underlying_io_error() {
        let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "claude not found");
        let err = VerbError::Spawn { source: inner };
        let rendered = err.to_string();
        assert!(rendered.contains("spawn agent"));
        assert!(rendered.contains("claude not found"));
    }

    #[test]
    fn cancelled_display_is_terse() {
        let err = VerbError::Cancelled;
        assert_eq!(err.to_string(), "cancelled by caller");
    }

    #[test]
    fn internal_includes_message() {
        let err = VerbError::Internal {
            message: "auto_commit failed: refs/heads/main".into(),
        };
        assert!(err.to_string().contains("auto_commit failed"));
    }

    #[test]
    fn agent_failed_display_includes_exit_code() {
        // Some(n) → Display contains both the literal exit code and the
        // "non-zero status" phrase (per spec verb-library §Verb Error Enum).
        let with_code = VerbError::AgentFailed {
            exit_code: Some(42),
        };
        let s = with_code.to_string();
        assert!(
            s.contains("42"),
            "Display SHALL include exit code; got: {s}"
        );
        assert!(
            s.contains("non-zero status"),
            "Display SHALL include 'non-zero status'; got: {s}"
        );

        // None → Display SHALL NOT include any parenthesised group after
        // "non-zero status" (the conditional `unwrap_or_default()` collapses
        // to empty string). The "non-zero status" phrase SHALL still appear.
        let without_code = VerbError::AgentFailed { exit_code: None };
        let s2 = without_code.to_string();
        assert!(
            !s2.contains('('),
            "Display SHALL NOT include '(' when exit_code is None; got: {s2}"
        );
        assert!(
            s2.contains("non-zero status"),
            "Display SHALL include 'non-zero status' even when exit_code is None; got: {s2}"
        );
    }

    #[test]
    fn config_parse_wraps_yaml_error_with_section_label() {
        let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("\t- bad yaml").unwrap_err();
        let err = VerbError::ConfigParse {
            which: "claude_code",
            source: ConfigLoadError::YamlParse(yaml_err),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("claude_code config parse failed"));
    }

    #[test]
    fn cli_exit_code_mapping_covers_every_variant() {
        let cases = [
            (
                VerbError::VaultMissing {
                    path: PathBuf::from("/x"),
                },
                2,
            ),
            (
                VerbError::Spawn {
                    source: std::io::Error::other("x"),
                },
                1,
            ),
            (VerbError::Cancelled, 0),
            (
                VerbError::AgentFailed { exit_code: Some(2) },
                1,
            ),
            (
                VerbError::Internal {
                    message: "x".into(),
                },
                1,
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.cli_exit_code(), expected);
        }
        // ConfigParse handled separately because constructing a real
        // serde_yaml::Error is not Clone.
        let yaml_err = serde_yaml::from_str::<serde_yaml::Value>("\t-").unwrap_err();
        let err = VerbError::ConfigParse {
            which: "log",
            source: ConfigLoadError::YamlParse(yaml_err),
        };
        assert_eq!(err.cli_exit_code(), 2);
    }
}
