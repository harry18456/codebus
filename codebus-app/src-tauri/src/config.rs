//! `app.*` namespace inside `~/.codebus/config.yaml`.
//!
//! After v3-app-quiz the `app.*` namespace carries ONLY
//! `app.quiz.pass_threshold` (50–100). The default quiz length moved OUT of
//! `app.*` into the shared top-level `quiz.default_length` key, read by both
//! the CLI and the app — see [`codebus_core::config::quiz`] and the
//! superseded `AppConfig Namespace Isolation` requirement (v3-app-quiz).
//!
//! The CLI never touches `app.*`. [`read_app_config`] validates
//! `pass_threshold` eagerly so the Settings UI cannot persist an
//! out-of-range value. [`resolve_quiz_default_length`] resolves the shared
//! length, performing a one-time migration that still honours a legacy
//! `app.quiz.default_length` left over from a pre-v3-app-quiz config.

use codebus_core::config::quiz::{DEFAULT_QUIZ_LENGTH, QUIZ_LENGTH_MAX, QUIZ_LENGTH_MIN};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_QUIZ_PASS_THRESHOLD: u8 = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)] // missing fields fall back to `AppQuizConfig::default()`
pub struct AppQuizConfig {
    pub pass_threshold: u8,
}

impl Default for AppQuizConfig {
    fn default() -> Self {
        Self {
            pass_threshold: DEFAULT_QUIZ_PASS_THRESHOLD,
        }
    }
}

impl AppQuizConfig {
    /// Validate field ranges. Returns the first violation as
    /// `AppError::Invalid { field, message }` matching the spec's
    /// "Invalid threshold returns invalid with field name" scenario.
    ///
    /// `default_length` is no longer an `app.*` field after v3-app-quiz —
    /// its range is enforced by [`resolve_quiz_default_length`] against the
    /// shared `quiz.default_length` key instead.
    pub fn validate(&self) -> Result<(), AppError> {
        if !(50..=100).contains(&self.pass_threshold) {
            return Err(AppError::Invalid {
                field: "app.quiz.pass_threshold".into(),
                message: format!(
                    "must be between 50 and 100 (inclusive); got {}",
                    self.pass_threshold
                ),
            });
        }
        Ok(())
    }
}

/// User-selected locale override for the UI. `Zh` / `En` pin the active
/// language; absence (None) means "auto-detect from `navigator.language`".
/// Backs spec *Settings Language Override*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocaleOverride {
    Zh,
    En,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub quiz: AppQuizConfig,
    /// `null` (or absent) means auto-detect; `"zh"` / `"en"` pin the locale.
    /// Round-tripped via serde — invalid string values surface as
    /// `AppError::ConfigParse` through [`read_app_config`]. Serialized
    /// explicitly (no `skip_serializing_if`) so the saved YAML always
    /// states the user's choice, including the explicit `null` for "Auto".
    #[serde(default)]
    pub locale_override: Option<LocaleOverride>,
}

/// Read the `app.*` namespace from a JSON-shaped global config payload.
/// Missing `app` or `app.quiz` keys collapse to defaults (forward-compat
/// rule: app must boot cleanly on a config.yaml with no `app:` section).
///
/// A legacy `app.quiz.default_length` left over from a pre-v3-app-quiz
/// config is an unknown field here and is silently ignored (serde default
/// behaviour) — it is consumed instead by [`resolve_quiz_default_length`]
/// as the migration source.
pub fn read_app_config(payload: &serde_json::Value) -> Result<AppConfig, AppError> {
    let app_node = payload
        .get("app")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let cfg: AppConfig = if app_node.is_null() {
        AppConfig::default()
    } else {
        serde_json::from_value(app_node).map_err(|e| AppError::ConfigParse {
            message: format!("app namespace: {e}"),
        })?
    };
    cfg.quiz.validate()?;
    Ok(cfg)
}

/// Resolve the shared `quiz.default_length` with one-time migration.
///
/// Resolution order:
/// 1. top-level `quiz.default_length` (the post-v3-app-quiz shared key)
/// 2. legacy `app.quiz.default_length` (pre-v3-app-quiz; migrated forward)
/// 3. default [`DEFAULT_QUIZ_LENGTH`] (5)
///
/// An out-of-range value (outside `QUIZ_LENGTH_MIN..=QUIZ_LENGTH_MAX`) from
/// either source surfaces as `AppError::Invalid { field:
/// "quiz.default_length" }`, mirroring the eager-validation contract used for
/// `pass_threshold`.
pub fn resolve_quiz_default_length(payload: &serde_json::Value) -> Result<u8, AppError> {
    let from = |root: &str, nested: bool| -> Option<u64> {
        let node = payload.get(root)?;
        let quiz = if nested { node.get("quiz")? } else { node };
        quiz.get("default_length")?.as_u64()
    };
    // Shared key wins; legacy app.quiz.default_length is the migration source.
    let raw = from("quiz", false).or_else(|| from("app", true));
    let Some(val) = raw else {
        return Ok(DEFAULT_QUIZ_LENGTH);
    };
    u8::try_from(val)
        .ok()
        .filter(|v| (QUIZ_LENGTH_MIN..=QUIZ_LENGTH_MAX).contains(v))
        .ok_or_else(|| AppError::Invalid {
            field: "quiz.default_length".into(),
            message: format!(
                "must be between {QUIZ_LENGTH_MIN} and {QUIZ_LENGTH_MAX} (inclusive); got {val}"
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pass_threshold_default_matches_spec() {
        let cfg = AppQuizConfig::default();
        assert_eq!(cfg.pass_threshold, 80);
    }

    #[test]
    fn missing_app_namespace_yields_defaults() {
        let payload = json!({ "pii": {} });
        let cfg = read_app_config(&payload).expect("missing app namespace must succeed");
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn missing_app_quiz_subsection_yields_defaults() {
        let payload = json!({ "app": {} });
        let cfg = read_app_config(&payload).expect("missing app.quiz must succeed");
        assert_eq!(cfg.quiz, AppQuizConfig::default());
    }

    #[test]
    fn legacy_default_length_in_app_quiz_is_ignored_by_read() {
        // A stale pre-v3-app-quiz config still has app.quiz.default_length.
        // read_app_config must not choke on it (unknown field ignored) and
        // pass_threshold still resolves.
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 70, "default_length": 4 } }
        });
        let cfg = read_app_config(&payload).expect("legacy field must be ignored");
        assert_eq!(cfg.quiz.pass_threshold, 70);
    }

    #[test]
    fn threshold_above_100_returns_invalid_with_field() {
        let payload = json!({ "app": { "quiz": { "pass_threshold": 200 } } });
        let err = read_app_config(&payload).expect_err("threshold=200 must fail");
        match err {
            AppError::Invalid { field, .. } => {
                assert_eq!(field, "app.quiz.pass_threshold");
            }
            other => panic!("expected Invalid{{field}}, got {other:?}"),
        }
    }

    #[test]
    fn threshold_below_50_returns_invalid_with_field() {
        let payload = json!({ "app": { "quiz": { "pass_threshold": 10 } } });
        let err = read_app_config(&payload).expect_err("threshold=10 must fail");
        assert!(matches!(
            err,
            AppError::Invalid { ref field, .. } if field == "app.quiz.pass_threshold"
        ));
    }

    #[test]
    fn boundary_values_50_and_100_accepted() {
        for threshold in [50u8, 100u8] {
            let payload = json!({ "app": { "quiz": { "pass_threshold": threshold } } });
            read_app_config(&payload)
                .unwrap_or_else(|e| panic!("threshold={threshold} should be accepted: {e:?}"));
        }
    }

    // --- resolve_quiz_default_length: migration cases (task 1.2) ---

    /// Case "皆無→5": neither shared nor legacy key present.
    #[test]
    fn resolve_neither_yields_default_5() {
        let payload = json!({ "app": { "quiz": { "pass_threshold": 80 } } });
        assert_eq!(resolve_quiz_default_length(&payload).unwrap(), 5);
    }

    /// Case "純新鍵": top-level shared quiz.default_length is read.
    #[test]
    fn resolve_shared_key_is_used() {
        let payload = json!({ "quiz": { "default_length": 8 } });
        assert_eq!(resolve_quiz_default_length(&payload).unwrap(), 8);
    }

    /// Case "僅舊鍵→遷移": no shared key, legacy app.quiz.default_length
    /// migrates forward.
    #[test]
    fn resolve_legacy_app_key_migrates() {
        let payload = json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 7 } } });
        assert_eq!(resolve_quiz_default_length(&payload).unwrap(), 7);
    }

    /// Shared key wins over a stale legacy key when both are present.
    #[test]
    fn resolve_shared_key_wins_over_legacy() {
        let payload = json!({
            "quiz": { "default_length": 9 },
            "app": { "quiz": { "pass_threshold": 80, "default_length": 3 } }
        });
        assert_eq!(resolve_quiz_default_length(&payload).unwrap(), 9);
    }

    /// Out-of-range shared value is rejected with the shared field name.
    #[test]
    fn resolve_out_of_range_returns_invalid() {
        let payload = json!({ "quiz": { "default_length": 99 } });
        let err = resolve_quiz_default_length(&payload).expect_err("99 must fail");
        assert!(matches!(
            err,
            AppError::Invalid { ref field, .. } if field == "quiz.default_length"
        ));
    }

    /// Inclusive boundaries 3 and 10 are accepted.
    #[test]
    fn resolve_boundary_values_accepted() {
        for v in [3u64, 10u64] {
            let payload = json!({ "quiz": { "default_length": v } });
            assert_eq!(resolve_quiz_default_length(&payload).unwrap(), v as u8);
        }
    }
}
