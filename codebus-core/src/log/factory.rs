//! Log sink factory. Mirrors [`crate::llm::factory`] / [`crate::pii::factory`]
//! shape — serde-tagged enum [`SinkConfig`] dispatched by `match`.
//!
//! Each variant is self-contained: variant-specific fields live inside
//! the variant struct, not in a flat shared `Config`. See
//! `openspec/changes/config-tagged-enum-refactor/design.md` for the
//! pattern rationale.

use crate::log::sink::LogSink;
use crate::log::sinks::{jsonl_sink::JsonlSink, null_sink::NullSink};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tagged-enum config for the log sink plugin domain. Discriminator
/// field is `sink` (matches the YAML form previously used as
/// `sink: <kind>`). Variants are namespaced; per-variant fields are
/// type-checked at deserialise time.
///
/// Example YAML:
///
/// ```yaml
/// sink: jsonl
/// dir: /var/log/codebus
/// ```
///
/// `dir` is optional in YAML — when omitted, the run flow substitutes
/// `<repo>/.codebus/logs/` of the active vault (the spec's default).
/// `build_sink` itself still rejects `None`; the caller (`run_goal` /
/// `run_query` via `main.rs`) is responsible for resolving the default
/// vault path before invoking the factory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sink", rename_all = "snake_case")]
pub enum SinkConfig {
    /// `null` — no-op sink, the 0.2.0 behavior-preserving default.
    Null {},
    /// `jsonl` — date-rotated `runs-YYYY-MM-DD.jsonl` files.
    Jsonl {
        /// Output directory. `None` at the config layer signals "use the
        /// vault-local default"; the run flow resolves this before
        /// calling `build_sink`. `build_sink` rejects an unresolved
        /// `None` because by that layer the path should already be
        /// concrete.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<PathBuf>,
    },
    /// `otel` — OpenTelemetry export. Requires `log-otel` cargo feature.
    Otel {},
}

impl Default for SinkConfig {
    /// Default sink is `Jsonl { dir: None }` — the run flow resolves
    /// `dir: None` to `<repo>/.codebus/logs/`. This matches the
    /// `goals.jsonl` precedent: codebus auto-tracks per-vault metadata
    /// without requiring an explicit opt-in. Users who don't want any
    /// run logging set `log: { sink: null }` in `~/.codebus/config.yaml`.
    fn default() -> Self {
        Self::Jsonl { dir: None }
    }
}

#[derive(Debug)]
pub enum SinkError {
    Setup(String),
    FeatureNotCompiled {
        feature: &'static str,
        hint: &'static str,
    },
}

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkError::Setup(msg) => write!(f, "log sink setup failed: {msg}"),
            SinkError::FeatureNotCompiled { feature, hint } => write!(
                f,
                "log sink requires cargo feature `{feature}` (not compiled). {hint}"
            ),
        }
    }
}

impl std::error::Error for SinkError {}

/// Build a sink from a [`SinkConfig`].
pub fn build_sink(cfg: SinkConfig) -> Result<Box<dyn LogSink>, SinkError> {
    match cfg {
        SinkConfig::Null {} => Ok(Box::new(NullSink::new())),
        SinkConfig::Jsonl { dir, .. } => {
            let dir = dir.ok_or_else(|| SinkError::Setup("jsonl sink requires `dir`".into()))?;
            Ok(Box::new(JsonlSink::new(dir)))
        }
        SinkConfig::Otel {} => Err(SinkError::FeatureNotCompiled {
            feature: "log-otel",
            hint: "rebuild with: cargo install codebus --features log-otel",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_jsonl_with_dir_none() {
        // Pinning the new default. The run flow downstream resolves
        // `dir: None` to `<repo>/.codebus/logs/` so users get auto
        // logging without opt-in (matches goals.jsonl precedent).
        // Changing the default is a behavior break for every existing
        // user; force any future change to update this test deliberately.
        assert_eq!(SinkConfig::default(), SinkConfig::Jsonl { dir: None });
    }

    #[test]
    fn jsonl_default_serializes_with_only_sink_key() {
        let cfg = SinkConfig::default();
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        // `dir: None` is skipped, so the serialized form is just the
        // discriminator. Round-trip back yields the same default.
        assert!(
            yaml.contains("sink: jsonl"),
            "expected `sink: jsonl`, got: {yaml}"
        );
        assert!(!yaml.contains("dir:"), "dir: None should be skipped");
        let parsed: SinkConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(parsed, SinkConfig::Jsonl { dir: None });
    }

    #[test]
    fn explicit_null_sink_round_trips_via_serde_yaml() {
        // Users opt OUT of run logging via `log: { sink: null }`. Verify
        // the explicit form (de)serializes correctly. serde_yaml quotes
        // the discriminator string `"null"` to disambiguate from YAML's
        // bare `null` literal.
        let cfg = SinkConfig::Null {};
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        let parsed: SinkConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(parsed, SinkConfig::Null {});
        let parsed_unquoted: SinkConfig =
            serde_yaml::from_str("sink: \"null\"\n").expect("deserialize unquoted");
        assert_eq!(parsed_unquoted, SinkConfig::Null {});
    }

    #[test]
    fn jsonl_round_trips_with_dir() {
        let yaml = "sink: jsonl\ndir: /var/log/codebus\n";
        let parsed: SinkConfig = serde_yaml::from_str(yaml).expect("deserialize");
        match &parsed {
            SinkConfig::Jsonl { dir } => {
                assert_eq!(dir.as_deref(), Some(std::path::Path::new("/var/log/codebus")));
            }
            other => panic!("expected Jsonl variant, got {other:?}"),
        }
        // Round-trip: re-serialise and parse again, structural equality.
        let reser = serde_yaml::to_string(&parsed).expect("serialize");
        let reparsed: SinkConfig = serde_yaml::from_str(&reser).expect("re-deserialize");
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn jsonl_silently_ignores_legacy_retention_days_field() {
        // `retention_days` was removed in the token-tracking change. YAML
        // configs in the wild may still carry it; serde + the variant's
        // unknown-field default (no `deny_unknown_fields`) silently drops
        // the field rather than erroring. Verifies graceful migration.
        let yaml = "sink: jsonl\ndir: /var/log/codebus\nretention_days: 30\n";
        let parsed: SinkConfig = serde_yaml::from_str(yaml).expect("deserialize");
        assert_eq!(
            parsed,
            SinkConfig::Jsonl {
                dir: Some(std::path::PathBuf::from("/var/log/codebus")),
            }
        );
    }

    #[test]
    fn jsonl_omits_unset_dir_when_serialized() {
        let cfg = SinkConfig::Jsonl { dir: None };
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        // `skip_serializing_if = Option::is_none` — field shouldn't appear.
        assert!(!yaml.contains("dir:"), "unexpected `dir:` in {yaml}");
    }

    #[test]
    fn jsonl_without_dir_returns_setup_error() {
        let cfg = SinkConfig::Jsonl { dir: None };
        // `Box<dyn LogSink>` doesn't impl Debug, so use match instead of expect_err.
        match build_sink(cfg) {
            Err(SinkError::Setup(msg)) => {
                assert!(msg.contains("dir"), "error message should mention `dir`: {msg}");
            }
            Err(other) => panic!("expected SinkError::Setup, got {other:?}"),
            Ok(_) => panic!("expected error when dir is None"),
        }
    }

    #[test]
    fn jsonl_with_dir_builds_sink() {
        let tmp = std::env::temp_dir().join("codebus-factory-test-jsonl");
        let cfg = SinkConfig::Jsonl {
            dir: Some(tmp.clone()),
        };
        let sink = build_sink(cfg).expect("should build");
        assert_eq!(sink.name(), "jsonl");
    }

    #[test]
    fn null_builds_sink() {
        let sink = build_sink(SinkConfig::Null {}).expect("should build");
        assert_eq!(sink.name(), "null");
    }

    #[test]
    fn otel_returns_feature_not_compiled() {
        match build_sink(SinkConfig::Otel {}) {
            Err(SinkError::FeatureNotCompiled { feature, hint }) => {
                assert_eq!(feature, "log-otel");
                assert!(
                    hint.contains("log-otel"),
                    "hint should reference feature flag: {hint}"
                );
            }
            Err(other) => panic!("expected FeatureNotCompiled, got {other:?}"),
            Ok(_) => panic!("expected FeatureNotCompiled error, got Ok"),
        }
    }

    #[test]
    fn setup_error_display_mentions_setup_failure() {
        let err = SinkError::Setup("test reason".into());
        let s = format!("{err}");
        assert!(s.contains("test reason"));
        assert!(s.contains("log sink setup failed"));
    }

    #[test]
    fn feature_not_compiled_display_mentions_feature() {
        let err = SinkError::FeatureNotCompiled {
            feature: "log-otel",
            hint: "rebuild with feature",
        };
        let s = format!("{err}");
        assert!(s.contains("log-otel"));
        assert!(s.contains("rebuild with feature"));
    }
}
