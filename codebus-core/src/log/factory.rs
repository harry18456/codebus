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
/// retention_days: 30
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sink", rename_all = "snake_case")]
pub enum SinkConfig {
    /// `null` — no-op sink, the 0.2.0 behavior-preserving default.
    Null {},
    /// `jsonl` — date-rotated `.jsonl` files.
    Jsonl {
        /// Output directory. Required at build time. `None` is allowed at
        /// the config layer so the loader / caller can later substitute a
        /// default (e.g. `~/.codebus/log`); `build_sink` rejects `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dir: Option<PathBuf>,
        /// Retention in days. Currently advisory — no rotation logic yet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retention_days: Option<u32>,
    },
    /// `otel` — OpenTelemetry export. Requires `log-otel` cargo feature.
    Otel {},
}

impl Default for SinkConfig {
    fn default() -> Self {
        Self::Null {}
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
    fn default_is_null() {
        assert_eq!(SinkConfig::default(), SinkConfig::Null {});
    }

    #[test]
    fn null_default_round_trips_via_serde_yaml() {
        let cfg = SinkConfig::default();
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        // Tag-only variant produces `sink: 'null'` — serde_yaml quotes the
        // discriminator string to disambiguate from YAML's bare `null`
        // literal. Both quoted and unquoted forms deserialize back to the
        // same variant; we just check round-trip correctness.
        let parsed: SinkConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(parsed, SinkConfig::Null {});
        // Also accept the explicit unquoted form on input.
        let parsed_unquoted: SinkConfig =
            serde_yaml::from_str("sink: \"null\"\n").expect("deserialize unquoted");
        assert_eq!(parsed_unquoted, SinkConfig::Null {});
    }

    #[test]
    fn jsonl_round_trips_with_dir_and_retention() {
        let yaml = "sink: jsonl\ndir: /var/log/codebus\nretention_days: 30\n";
        let parsed: SinkConfig = serde_yaml::from_str(yaml).expect("deserialize");
        match &parsed {
            SinkConfig::Jsonl {
                dir,
                retention_days,
            } => {
                assert_eq!(dir.as_deref(), Some(std::path::Path::new("/var/log/codebus")));
                assert_eq!(*retention_days, Some(30));
            }
            other => panic!("expected Jsonl variant, got {other:?}"),
        }
        // Round-trip: re-serialise and parse again, structural equality.
        let reser = serde_yaml::to_string(&parsed).expect("serialize");
        let reparsed: SinkConfig = serde_yaml::from_str(&reser).expect("re-deserialize");
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn jsonl_omits_unset_fields_when_serialized() {
        let cfg = SinkConfig::Jsonl {
            dir: None,
            retention_days: None,
        };
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        // `skip_serializing_if = Option::is_none` — fields shouldn't appear.
        assert!(!yaml.contains("dir:"), "unexpected `dir:` in {yaml}");
        assert!(
            !yaml.contains("retention_days:"),
            "unexpected `retention_days:` in {yaml}"
        );
    }

    #[test]
    fn jsonl_without_dir_returns_setup_error() {
        let cfg = SinkConfig::Jsonl {
            dir: None,
            retention_days: None,
        };
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
            retention_days: Some(7),
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
