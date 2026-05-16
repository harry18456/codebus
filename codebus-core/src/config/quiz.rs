//! `quiz.*` shared config loader for the v3-app-quiz Shared Quiz Config
//! Namespace requirement.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! quiz:
//!   default_length: 5   # integer 3..=10, default 5
//! ```
//!
//! This is a **shared** namespace: both the `codebus quiz` CLI and the
//! codebus-app read `quiz.default_length`. It deliberately lives outside the
//! `app.*` namespace (which is app-only and CLI-ignored) — see the
//! `app-shell` AppConfig Namespace Isolation requirement, superseded by
//! v3-app-quiz so that `default_length` is shared rather than app-only.
//!
//! Forward-compat tolerance mirrors the `pii.*` / `lint.fix.*` loaders:
//! missing file, missing `quiz` section, and missing `default_length` field
//! all fall through to the default of 5 without stderr output. Unknown keys
//! inside `quiz` are silently ignored. An out-of-range `default_length`
//! (outside 3..=10) is rejected as a `ConfigLoadError::YamlParse` so the
//! caller applies the standard warn-and-default fallback, mirroring the
//! `pii.on_hit` unknown-discriminator contract.
//!
//! The `run_quiz` library function never calls this loader — `question_count`
//! is always caller-injected. Only the CLI and app callers read this key.

use serde::Deserialize;
use serde::de::Error as _;
use std::fs;
use std::path::Path;

/// Default quiz length when the key is absent.
pub const DEFAULT_QUIZ_LENGTH: u8 = 5;

/// Inclusive valid range for `quiz.default_length`.
pub const QUIZ_LENGTH_MIN: u8 = 3;
pub const QUIZ_LENGTH_MAX: u8 = 10;

/// Effective shared quiz configuration after merging file + defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuizConfig {
    pub default_length: u8,
}

impl Default for QuizConfig {
    fn default() -> Self {
        Self {
            default_length: DEFAULT_QUIZ_LENGTH,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    quiz: Option<QuizSection>,
}

#[derive(Debug, Default, Deserialize)]
struct QuizSection {
    /// Range-validated at deserialize time: an out-of-range integer raises a
    /// `serde` error, which the loader surfaces as `YamlParse` so the caller
    /// warn-and-defaults (same contract as an unknown `pii.on_hit` value).
    #[serde(default, deserialize_with = "deserialize_default_length")]
    default_length: Option<u8>,
}

fn deserialize_default_length<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<u8>::deserialize(deserializer)?;
    if let Some(v) = raw {
        if !(QUIZ_LENGTH_MIN..=QUIZ_LENGTH_MAX).contains(&v) {
            return Err(D::Error::custom(format!(
                "quiz.default_length must be between {QUIZ_LENGTH_MIN} and {QUIZ_LENGTH_MAX}, got {v}"
            )));
        }
    }
    Ok(raw)
}

/// Load `quiz.*` config from `path`. Returns the default (`default_length =
/// 5`) when the file does not exist OR the `quiz` section / `default_length`
/// field is absent. Returns `Err` when the file exists but cannot be read
/// (IO error), is structurally invalid YAML, or carries an out-of-range
/// `default_length` — callers SHALL fall back to defaults on `Err` after
/// printing a stderr warning, mirroring the `pii.*` loader contract.
pub fn load_quiz_config(path: &Path) -> Result<QuizConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(QuizConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile =
        serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = QuizConfig::default();
    if let Some(quiz) = file.quiz {
        if let Some(len) = quiz.default_length {
            cfg.default_length = len;
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// Scenario: Missing key resolves to default — file missing → 5.
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_quiz_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert_eq!(cfg, QuizConfig::default());
        assert_eq!(cfg.default_length, 5);
    }

    /// Scenario: Missing key resolves to default — quiz section absent → 5.
    #[test]
    fn default_when_quiz_section_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  scanner: regex_basic\n");
        let cfg = load_quiz_config(&p).unwrap();
        assert_eq!(cfg.default_length, 5);
    }

    /// Scenario: Missing key resolves to default — field absent → 5.
    #[test]
    fn default_when_default_length_field_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "quiz:\n  future_field: hi\n");
        let cfg = load_quiz_config(&p).unwrap();
        assert_eq!(cfg.default_length, 5);
    }

    /// Task 1.1: legal value read back verbatim.
    #[test]
    fn valid_value_round_trips() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "quiz:\n  default_length: 8\n");
        let cfg = load_quiz_config(&p).unwrap();
        assert_eq!(cfg.default_length, 8);
    }

    /// Task 1.1: inclusive boundaries 3 and 10 are valid.
    #[test]
    fn boundary_values_are_valid() {
        let tmp = TempDir::new().unwrap();
        for v in [3u8, 10u8] {
            let p = write_yaml(tmp.path(), &format!("quiz:\n  default_length: {v}\n"));
            let cfg = load_quiz_config(&p).unwrap();
            assert_eq!(cfg.default_length, v);
        }
    }

    /// Task 1.1: out-of-range low is rejected (caller warn-and-defaults).
    #[test]
    fn out_of_range_low_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "quiz:\n  default_length: 2\n");
        let result = load_quiz_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Task 1.1: out-of-range high is rejected.
    #[test]
    fn out_of_range_high_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "quiz:\n  default_length: 11\n");
        let result = load_quiz_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Unknown subkey under `quiz` is silently ignored (forward-compat).
    #[test]
    fn unknown_quiz_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "quiz:\n  future_field: hello\n  default_length: 7\n",
        );
        let cfg = load_quiz_config(&p).unwrap();
        assert_eq!(cfg.default_length, 7);
    }

    /// Invalid YAML returns Err so caller can warn-and-default.
    #[test]
    fn invalid_yaml_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "quiz:\n  : :: not yaml\n");
        let result = load_quiz_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }
}
