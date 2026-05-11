//! `app.*` namespace inside `~/.codebus/config.yaml`.
//!
//! The CLI never touches this namespace (spec `AppConfig Namespace
//! Isolation`). The app validates `app.quiz.pass_threshold` (50–100) and
//! `app.quiz.default_length` (3–10) eagerly on both load and save so the
//! Settings UI cannot silently persist an out-of-range value.

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_QUIZ_PASS_THRESHOLD: u8 = 80;
pub const DEFAULT_QUIZ_LENGTH: u8 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)] // missing fields fall back to `AppQuizConfig::default()`
pub struct AppQuizConfig {
    pub pass_threshold: u8,
    pub default_length: u8,
}

impl Default for AppQuizConfig {
    fn default() -> Self {
        Self {
            pass_threshold: DEFAULT_QUIZ_PASS_THRESHOLD,
            default_length: DEFAULT_QUIZ_LENGTH,
        }
    }
}

impl AppQuizConfig {
    /// Validate field ranges. Returns the first violation as
    /// `AppError::Invalid { field, message }` matching the spec's
    /// "Invalid threshold returns invalid with field name" scenario.
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
        if !(3..=10).contains(&self.default_length) {
            return Err(AppError::Invalid {
                field: "app.quiz.default_length".into(),
                message: format!(
                    "must be between 3 and 10 (inclusive); got {}",
                    self.default_length
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub quiz: AppQuizConfig,
}

/// Read the `app.*` namespace from a JSON-shaped global config payload.
/// Missing `app` or `app.quiz` keys collapse to defaults (forward-compat
/// rule: app must boot cleanly on a config.yaml with no `app:` section).
pub fn read_app_config(payload: &serde_json::Value) -> Result<AppConfig, AppError> {
    let app_node = payload.get("app").cloned().unwrap_or(serde_json::Value::Null);
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_match_spec() {
        let cfg = AppQuizConfig::default();
        assert_eq!(cfg.pass_threshold, 80);
        assert_eq!(cfg.default_length, 5);
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
    fn threshold_above_100_returns_invalid_with_field() {
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 200, "default_length": 5 } }
        });
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
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 10, "default_length": 5 } }
        });
        let err = read_app_config(&payload).expect_err("threshold=10 must fail");
        assert!(matches!(
            err,
            AppError::Invalid { ref field, .. } if field == "app.quiz.pass_threshold"
        ));
    }

    #[test]
    fn default_length_out_of_range_returns_invalid() {
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 80, "default_length": 99 } }
        });
        let err = read_app_config(&payload).expect_err("default_length=99 must fail");
        assert!(matches!(
            err,
            AppError::Invalid { ref field, .. } if field == "app.quiz.default_length"
        ));
    }

    #[test]
    fn partial_quiz_missing_default_length_uses_default() {
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 70 } }
        });
        let cfg = read_app_config(&payload).expect("partial quiz must succeed");
        assert_eq!(cfg.quiz.pass_threshold, 70);
        assert_eq!(cfg.quiz.default_length, DEFAULT_QUIZ_LENGTH);
    }

    #[test]
    fn partial_quiz_missing_pass_threshold_uses_default() {
        let payload = json!({
            "app": { "quiz": { "default_length": 7 } }
        });
        let cfg = read_app_config(&payload).expect("partial quiz must succeed");
        assert_eq!(cfg.quiz.pass_threshold, DEFAULT_QUIZ_PASS_THRESHOLD);
        assert_eq!(cfg.quiz.default_length, 7);
    }

    #[test]
    fn boundary_values_50_and_100_accepted() {
        for threshold in [50u8, 100u8] {
            let payload = json!({
                "app": { "quiz": { "pass_threshold": threshold, "default_length": 5 } }
            });
            read_app_config(&payload)
                .unwrap_or_else(|e| panic!("threshold={threshold} should be accepted: {e:?}"));
        }
    }
}
