//! Tagged-enum log sink factory. Discriminator field is `sink` matching the
//! YAML form `log: { sink: "jsonl" | "null" }`.
//!
//! v3-run-log carry from v2 with two simplifications:
//!   - Drop `Otel` variant (no real consumer; revisit when there is one).
//!   - Drop the `feature = "log-otel"` cargo feature flag plumbing.
//!
//! The `Jsonl { dir: None }` default is intentional: the verb command
//! resolves `None` to `<vault>/.codebus/log/`, giving users automatic
//! per-vault history without explicit opt-in. Users who don't want any
//! run logging set `log: { sink: "null" }` in `~/.codebus/config.yaml`.

use crate::log::sink::LogSink;
use crate::log::sinks::{jsonl_sink::JsonlSink, null_sink::NullSink};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sink", rename_all = "snake_case")]
pub enum SinkConfig {
    /// User-facing wire form is `"none"` — the YAML literal `null` would
    /// dispatch to this variant via serde's snake_case rename and silently
    /// opt the user out when they may have meant a typo. Aligns with the
    /// `pii.scanner: none` foot-gun avoidance pattern shipped in v3-config.
    #[serde(rename = "none")]
    Null {},
    /// `jsonl` — date-rotated `runs-YYYY-MM-DD.jsonl` files.
    Jsonl {
        /// Output directory. `None` at the config layer signals "use the
        /// vault-local default"; the verb command resolves this before
        /// calling `build_sink`. `build_sink` itself rejects `None` because
        /// by that layer the path should already be concrete.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<PathBuf>,
    },
}

impl Default for SinkConfig {
    fn default() -> Self {
        Self::Jsonl { dir: None }
    }
}

#[derive(Debug)]
pub enum SinkError {
    Setup(String),
}

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkError::Setup(msg) => write!(f, "log sink setup failed: {msg}"),
        }
    }
}

impl std::error::Error for SinkError {}

/// Build a sink from a [`SinkConfig`]. Returns `Err(SinkError::Setup)` when
/// `Jsonl { dir: None }` is supplied (caller is expected to resolve the
/// default vault-local path before invoking the factory).
pub fn build_sink(cfg: SinkConfig) -> Result<Box<dyn LogSink>, SinkError> {
    match cfg {
        SinkConfig::Null {} => Ok(Box::new(NullSink::new())),
        SinkConfig::Jsonl { dir } => {
            let dir = dir.ok_or_else(|| {
                SinkError::Setup("jsonl sink requires `dir` (resolve before build_sink)".into())
            })?;
            Ok(Box::new(JsonlSink::new(dir)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_is_jsonl_with_dir_none() {
        // Pinning the default. The verb command downstream resolves
        // `dir: None` to `<vault>/.codebus/log/` so users get auto logging
        // without opt-in. Changing the default is a behavior break for every
        // existing user; force any future change to update this test
        // deliberately.
        assert_eq!(SinkConfig::default(), SinkConfig::Jsonl { dir: None });
    }

    #[test]
    fn build_null_returns_null_sink() {
        let sink = build_sink(SinkConfig::Null {}).unwrap();
        assert_eq!(sink.name(), "null");
    }

    #[test]
    fn build_jsonl_with_dir_returns_jsonl_sink() {
        let cfg = SinkConfig::Jsonl {
            dir: Some(PathBuf::from("/tmp/codebus-factory-test")),
        };
        let sink = build_sink(cfg).unwrap();
        assert_eq!(sink.name(), "jsonl");
    }

    #[test]
    fn build_jsonl_without_dir_returns_setup_error() {
        match build_sink(SinkConfig::Jsonl { dir: None }) {
            Err(SinkError::Setup(msg)) => {
                assert!(msg.contains("dir"), "error should mention `dir`: {msg}");
            }
            Ok(_) => panic!("expected SinkError::Setup, got Ok"),
        }
    }

    #[test]
    fn jsonl_default_serializes_with_only_sink_key() {
        let cfg = SinkConfig::default();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        assert!(yaml.contains("sink: jsonl"), "yaml: {yaml}");
        assert!(!yaml.contains("dir:"), "dir: None should be skipped: {yaml}");
    }

    #[test]
    fn explicit_none_round_trips() {
        // Users opt out via `log: { sink: none }`. Mirror of pii.scanner foot-gun
        // avoidance: bare YAML null does NOT match the variant (would dispatch
        // confusingly given snake_case rename of `Null` to `null`).
        let parsed: SinkConfig = serde_yaml::from_str("sink: none\n").unwrap();
        assert_eq!(parsed, SinkConfig::Null {});
    }

    #[test]
    fn bare_null_does_not_match_none_variant() {
        // `sink: null` (YAML literal) SHALL fail to parse, not silently
        // dispatch to Null variant. Caller falls back to default after
        // stderr warning.
        let result: Result<SinkConfig, _> = serde_yaml::from_str("sink: null\n");
        assert!(result.is_err(), "bare null SHALL not match `none` rename");
    }

    #[test]
    fn jsonl_with_dir_round_trips() {
        let yaml = "sink: jsonl\ndir: /var/log/codebus\n";
        let parsed: SinkConfig = serde_yaml::from_str(yaml).unwrap();
        match parsed {
            SinkConfig::Jsonl { dir } => {
                assert_eq!(dir.as_deref(), Some(Path::new("/var/log/codebus")));
            }
            other => panic!("expected Jsonl, got {other:?}"),
        }
    }
}
