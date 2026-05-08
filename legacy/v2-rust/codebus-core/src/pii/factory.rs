//! PII scanner factory.
//!
//! Tagged-enum config: `ScannerConfig` is a `#[serde(tag = "scanner")]`
//! sum type, one variant per supported scanner. Variant-specific knobs
//! (e.g. `RegexBasic.patterns_extra`) live inside their owning variant
//! so the type system rejects "wrong field on wrong scanner" by
//! construction. `on_hit` lives in every variant per design decision
//! "`on_hit` 在 ScannerConfig 內的定位" — every scanner has its own
//! hit-handling policy and the YAML form (`{ scanner: …, on_hit: … }`)
//! is preserved.
//!
//! See `openspec/changes/config-tagged-enum-refactor/design.md` for the
//! full pattern; the same shape is mirrored across `llm` / `log` / `render`
//! factories.

use crate::pii::provider::{OnHit, PiiScanner};
use crate::pii::scanners::{null_scanner::NullScanner, regex_basic::RegexBasicScanner};
use serde::{Deserialize, Serialize};

// `OnHit` lives in `pii::provider` and intentionally does NOT derive
// serde traits there (that module stays I/O-free). We bridge via local
// helpers so `ScannerConfig` can still round-trip through YAML.
//
// Wire form: `warn` | `skip` | `mask` (lowercase strings).
mod on_hit_serde {
    use super::OnHit;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &OnHit, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match value {
            OnHit::Warn => "warn",
            OnHit::Skip => "skip",
            OnHit::Mask => "mask",
        };
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<OnHit, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "warn" => Ok(OnHit::Warn),
            "skip" => Ok(OnHit::Skip),
            "mask" => Ok(OnHit::Mask),
            other => Err(serde::de::Error::custom(format!(
                "unknown on_hit value `{other}` (expected warn|skip|mask)"
            ))),
        }
    }
}

fn default_on_hit() -> OnHit {
    OnHit::default()
}

/// Tagged-enum config for the PII scanner plugin.
///
/// YAML shape (the `scanner:` key is the discriminator; remaining keys
/// belong to the chosen variant):
///
/// ```yaml
/// pii:
///   scanner: regex_basic
///   on_hit: warn
///   patterns_extra:
///     - 'INTERNAL-\d{6}'
/// ```
///
/// Variants without extra knobs (`Null` / `Presidio` / `Aws`) only carry
/// `on_hit`. Heavy-dep scanners (`Presidio`, `Aws`) are accepted at the
/// config layer regardless of cargo features so unknown discriminators
/// vs. "feature not compiled" stay distinguishable; `build_scanner`
/// returns `ScannerError::FeatureNotCompiled` until the feature lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scanner", rename_all = "snake_case")]
pub enum ScannerConfig {
    /// `null` — no-op scanner, always returns empty matches. The 0.2.0
    /// behavior-preserving default for raw_sync.
    Null {
        #[serde(default = "default_on_hit", with = "on_hit_serde")]
        on_hit: OnHit,
    },
    /// `regex_basic` — built-in regex pack covering common secrets,
    /// optionally augmented with user-supplied raw regex sources.
    RegexBasic {
        #[serde(default = "default_on_hit", with = "on_hit_serde")]
        on_hit: OnHit,
        /// Extra regex sources. Each entry is a raw regex; the scanner
        /// labels matches `"custom-<index>"`. Empty by default.
        #[serde(default)]
        patterns_extra: Vec<String>,
    },
    /// `presidio` — Microsoft Presidio HTTP service. Requires the
    /// `pii-presidio` feature; impl lands in a follow-up change.
    Presidio {
        #[serde(default = "default_on_hit", with = "on_hit_serde")]
        on_hit: OnHit,
    },
    /// `aws` — AWS Comprehend Detect-PII API. Requires the `pii-aws`
    /// feature; impl lands in a follow-up change.
    Aws {
        #[serde(default = "default_on_hit", with = "on_hit_serde")]
        on_hit: OnHit,
    },
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self::Null {
            on_hit: OnHit::Warn,
        }
    }
}

/// Build a scanner from a [`ScannerConfig`].
///
/// `Null` and `RegexBasic` are always available (no feature gating; deps
/// already in tree). Heavy-dep scanners return [`ScannerError::FeatureNotCompiled`]
/// when their feature isn't compiled.
pub fn build_scanner(cfg: ScannerConfig) -> Result<Box<dyn PiiScanner>, ScannerError> {
    match cfg {
        ScannerConfig::Null { .. } => Ok(Box::new(NullScanner::new())),
        ScannerConfig::RegexBasic { patterns_extra, .. } => {
            let scanner = RegexBasicScanner::new(&patterns_extra)
                .map_err(|e| ScannerError::Setup(format!("regex_basic init failed: {e}")))?;
            Ok(Box::new(scanner))
        }
        ScannerConfig::Presidio { .. } => Err(ScannerError::FeatureNotCompiled {
            feature: "pii-presidio",
            hint: "rebuild with: cargo install codebus --features pii-presidio",
        }),
        ScannerConfig::Aws { .. } => Err(ScannerError::FeatureNotCompiled {
            feature: "pii-aws",
            hint: "rebuild with: cargo install codebus --features pii-aws",
        }),
    }
}

#[derive(Debug)]
pub enum ScannerError {
    Setup(String),
    FeatureNotCompiled {
        feature: &'static str,
        hint: &'static str,
    },
}

impl std::fmt::Display for ScannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScannerError::Setup(msg) => write!(f, "scanner setup failed: {msg}"),
            ScannerError::FeatureNotCompiled { feature, hint } => write!(
                f,
                "scanner requires cargo feature `{feature}` (not compiled). {hint}"
            ),
        }
    }
}

impl std::error::Error for ScannerError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_null_with_warn_on_hit() {
        let cfg = ScannerConfig::default();
        assert_eq!(
            cfg,
            ScannerConfig::Null {
                on_hit: OnHit::Warn
            }
        );
    }

    #[test]
    fn null_default_round_trips_via_serde_yaml() {
        // Minimal form: just the discriminator. `on_hit` falls back to
        // OnHit::default() (== Warn) via #[serde(default)].
        let yaml = "scanner: null\n";
        let cfg: ScannerConfig = serde_yaml::from_str(yaml).expect("deserialize null");
        assert_eq!(
            cfg,
            ScannerConfig::Null {
                on_hit: OnHit::Warn
            }
        );

        // Re-serialize and re-parse to confirm round-trip.
        let dumped = serde_yaml::to_string(&cfg).expect("serialize null");
        let reparsed: ScannerConfig =
            serde_yaml::from_str(&dumped).expect("re-deserialize null");
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn regex_basic_round_trips_with_extra_patterns() {
        let cfg = ScannerConfig::RegexBasic {
            on_hit: OnHit::Warn,
            patterns_extra: vec!["INTERNAL-\\d{6}".into()],
        };

        let dumped = serde_yaml::to_string(&cfg).expect("serialize regex_basic");
        // Spot-check the YAML shape — discriminator + sibling fields, not
        // a nested object.
        assert!(
            dumped.contains("scanner: regex_basic"),
            "expected 'scanner: regex_basic' in:\n{dumped}"
        );
        assert!(
            dumped.contains("patterns_extra:"),
            "expected 'patterns_extra:' in:\n{dumped}"
        );
        assert!(
            dumped.contains("INTERNAL-\\d{6}"),
            "expected pattern body in:\n{dumped}"
        );

        let reparsed: ScannerConfig =
            serde_yaml::from_str(&dumped).expect("re-deserialize regex_basic");
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn presidio_round_trips_minimal() {
        let yaml = "scanner: presidio\n";
        let cfg: ScannerConfig = serde_yaml::from_str(yaml).expect("deserialize presidio");
        assert_eq!(
            cfg,
            ScannerConfig::Presidio {
                on_hit: OnHit::Warn
            }
        );

        let dumped = serde_yaml::to_string(&cfg).expect("serialize presidio");
        let reparsed: ScannerConfig =
            serde_yaml::from_str(&dumped).expect("re-deserialize presidio");
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn aws_round_trips_minimal() {
        let yaml = "scanner: aws\n";
        let cfg: ScannerConfig = serde_yaml::from_str(yaml).expect("deserialize aws");
        assert_eq!(
            cfg,
            ScannerConfig::Aws {
                on_hit: OnHit::Warn
            }
        );

        let dumped = serde_yaml::to_string(&cfg).expect("serialize aws");
        let reparsed: ScannerConfig =
            serde_yaml::from_str(&dumped).expect("re-deserialize aws");
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn build_null_returns_null_scanner() {
        let scanner = build_scanner(ScannerConfig::default()).expect("build null");
        assert_eq!(scanner.name(), "null");
    }

    #[test]
    fn build_regex_basic_returns_regex_scanner() {
        let cfg = ScannerConfig::RegexBasic {
            on_hit: OnHit::Warn,
            patterns_extra: vec![],
        };
        let scanner = build_scanner(cfg).expect("build regex_basic");
        assert_eq!(scanner.name(), "regex_basic");
    }

    #[test]
    fn build_presidio_returns_feature_not_compiled() {
        let cfg = ScannerConfig::Presidio {
            on_hit: OnHit::Warn,
        };
        match build_scanner(cfg) {
            Err(ScannerError::FeatureNotCompiled { feature, .. }) => {
                assert_eq!(feature, "pii-presidio");
            }
            Err(other) => panic!("expected FeatureNotCompiled, got error: {other}"),
            Ok(_) => panic!("expected FeatureNotCompiled, got Ok(scanner)"),
        }
    }

    #[test]
    fn build_aws_returns_feature_not_compiled() {
        let cfg = ScannerConfig::Aws {
            on_hit: OnHit::Warn,
        };
        match build_scanner(cfg) {
            Err(ScannerError::FeatureNotCompiled { feature, .. }) => {
                assert_eq!(feature, "pii-aws");
            }
            Err(other) => panic!("expected FeatureNotCompiled, got error: {other}"),
            Ok(_) => panic!("expected FeatureNotCompiled, got Ok(scanner)"),
        }
    }

    #[test]
    fn regex_basic_with_invalid_pattern_returns_setup_error() {
        let cfg = ScannerConfig::RegexBasic {
            on_hit: OnHit::Warn,
            // Unclosed character class — guaranteed to fail regex compile.
            patterns_extra: vec!["[unclosed".into()],
        };
        match build_scanner(cfg) {
            Err(ScannerError::Setup(msg)) => {
                assert!(
                    msg.contains("regex_basic init failed"),
                    "expected setup error message, got: {msg}"
                );
            }
            Err(other) => panic!("expected Setup error, got error: {other}"),
            Ok(_) => panic!("expected Setup error, got Ok(scanner)"),
        }
    }

    #[test]
    fn regex_basic_yaml_with_skip_on_hit() {
        // Confirms `on_hit` is parsed as a sibling key, not nested.
        let yaml = "\
scanner: regex_basic
on_hit: skip
patterns_extra:
  - 'INTERNAL-\\d{6}'
";
        let cfg: ScannerConfig =
            serde_yaml::from_str(yaml).expect("deserialize regex_basic with skip");
        assert_eq!(
            cfg,
            ScannerConfig::RegexBasic {
                on_hit: OnHit::Skip,
                patterns_extra: vec!["INTERNAL-\\d{6}".into()],
            }
        );
    }
}
