//! `goal.*` shared config loader (goal-content-verify design D5).
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! goal:
//!   content_verify: false   # bool, default false
//! ```
//!
//! Shared `goal.*` namespace: both the `codebus goal` CLI and the
//! codebus-app read `goal.content_verify` via this one loader. It
//! deliberately lives outside the `app.*` namespace (app-only,
//! CLI-ignored). Default is `false` so existing users do not silently
//! pay extra verify/repair spawns (audit: secure/cheap default; a load
//! error must also resolve to `false`, never silently enable spawns).
//!
//! Forward-compat tolerance mirrors `config::quiz`: missing file,
//! missing `goal` section, and missing `content_verify` field all fall
//! through to the default of `false` without stderr output. Unknown
//! keys inside `goal` are silently ignored. Structurally invalid YAML is
//! surfaced as `ConfigLoadError::YamlParse` so the caller applies the
//! standard conservative warn-and-default fallback.
//!
//! The `run_goal` library function never calls this loader —
//! `content_verify` is always caller-injected (the library never reads
//! config itself, matching the verb convention).

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Effective shared goal configuration after merging file + defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct GoalConfig {
    /// goal-content-verify (design D5): gate for the optional
    /// model-based content verification + repair stage. Default `false`.
    pub content_verify: bool,
}


#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    goal: Option<GoalSection>,
}

#[derive(Debug, Default, Deserialize)]
struct GoalSection {
    /// Absent → false (forward-compat). A wrong-typed value surfaces as
    /// a `serde` error → `YamlParse` → caller conservatively defaults.
    #[serde(default)]
    content_verify: bool,
}

/// Load `goal.*` config from `path`. Returns the default
/// (`content_verify = false`) when the file does not exist OR the `goal`
/// section / `content_verify` field is absent. Returns `Err` when the
/// file exists but cannot be read (IO error) or is structurally invalid
/// / wrong-typed YAML — callers SHALL fall back to the (false) default
/// on `Err`, mirroring the `quiz.*` / `pii.*` loader contract.
pub fn load_goal_config(path: &Path) -> Result<GoalConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(GoalConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile =
        serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = GoalConfig::default();
    if let Some(goal) = file.goal {
        cfg.content_verify = goal.content_verify;
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

    /// Scenario (cli / Goal Content Verify CLI Behavior — default off):
    /// missing file / missing section / missing field → false.
    #[test]
    fn content_verify_defaults_false_when_absent() {
        let tmp = TempDir::new().unwrap();
        // file missing
        let c1 = load_goal_config(&tmp.path().join("none.yaml")).unwrap();
        assert!(!c1.content_verify);
        assert_eq!(c1, GoalConfig::default());
        // goal section absent
        let p2 = write_yaml(tmp.path(), "pii:\n  scanner: regex_basic\n");
        assert!(!load_goal_config(&p2).unwrap().content_verify);
        // goal section present, field absent
        let p3 = write_yaml(tmp.path(), "goal:\n  future_field: hi\n");
        assert!(!load_goal_config(&p3).unwrap().content_verify);
    }

    #[test]
    fn content_verify_parses_true_and_false() {
        let tmp = TempDir::new().unwrap();
        let pt = write_yaml(tmp.path(), "goal:\n  content_verify: true\n");
        assert!(load_goal_config(&pt).unwrap().content_verify);
        let pf = write_yaml(tmp.path(), "goal:\n  content_verify: false\n");
        assert!(!load_goal_config(&pf).unwrap().content_verify);
    }

    #[test]
    fn unknown_goal_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "goal:\n  future_field: hello\n  content_verify: true\n",
        );
        assert!(load_goal_config(&p).unwrap().content_verify);
    }

    /// Wrong-typed value is rejected as `YamlParse` (no panic) so the
    /// caller conservatively defaults to false.
    #[test]
    fn wrong_typed_content_verify_returns_err_no_panic() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "goal:\n  content_verify: not-a-bool\n");
        let result = load_goal_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Structurally invalid YAML returns Err (caller warn-and-defaults).
    #[test]
    fn invalid_yaml_returns_err_no_panic() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "goal:\n  : :: not yaml\n");
        assert!(matches!(
            load_goal_config(&p),
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Coexists with other namespaces (quiz/pii) in the same file.
    #[test]
    fn coexists_with_other_namespaces() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "quiz:\n  content_verify: true\ngoal:\n  content_verify: true\npii:\n  scanner: regex_basic\n",
        );
        assert!(load_goal_config(&p).unwrap().content_verify);
    }
}
