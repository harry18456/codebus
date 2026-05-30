//! `lifecycle.*` config loader (run-outcome-lifecycle-integrity).
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! lifecycle:
//!   run_timeout_secs: 1800   # per-run wall-clock limit in seconds
//! ```
//!
//! `run_timeout_secs` is the optional per-run wall-clock safety net injected
//! into `agent::invoke` by the caller (CLI / app). Default is `None` = no
//! limit = current unbounded behavior; a load error MUST also resolve to
//! `None` (never silently fabricate or shorten a limit).
//!
//! **Zero-value safety (audit):** `run_timeout_secs: 0` is normalized to
//! `None` (no limit), NOT a zero-second timeout. A literal 0s limit would
//! terminate every run the instant it starts — a foot-gun for a confused
//! user who writes `0` expecting "disable". Treating `0` as "no limit" keeps
//! the absent / zero / disabled cases all safe and identical.
//!
//! Forward-compat tolerance mirrors `config::goal`: missing file, missing
//! `lifecycle` section, and missing `run_timeout_secs` field all fall
//! through to `None` without stderr output. Unknown keys inside `lifecycle`
//! are silently ignored. Structurally invalid / wrong-typed YAML is surfaced
//! as `ConfigLoadError::YamlParse` so the caller applies the standard
//! conservative warn-and-default (to `None`) fallback.
//!
//! The verb library never calls this loader — `run_timeout_secs` is always
//! caller-injected (the library never reads config itself, matching the verb
//! convention used by `config::goal`).

use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Effective `lifecycle.*` configuration after merging file + defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleConfig {
    /// Per-run wall-clock limit in seconds, or `None` for no limit.
    /// `Some(0)` from the file is normalized to `None` (see module docs).
    pub run_timeout_secs: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    lifecycle: Option<LifecycleSection>,
}

#[derive(Debug, Default, Deserialize)]
struct LifecycleSection {
    /// Absent → `None` (forward-compat). A wrong-typed value surfaces as a
    /// `serde` error → `YamlParse` → caller conservatively defaults.
    #[serde(default)]
    run_timeout_secs: Option<u64>,
}

/// Load `lifecycle.*` config from `path`. Returns the default
/// (`run_timeout_secs = None`) when the file does not exist OR the
/// `lifecycle` section / `run_timeout_secs` field is absent OR the field is
/// `0` (normalized to "no limit"). Returns `Err` when the file exists but
/// cannot be read (IO error) or is structurally invalid / wrong-typed YAML —
/// callers SHALL fall back to the (`None`) default on `Err`, mirroring the
/// `goal.*` / `quiz.*` loader contract.
pub fn load_lifecycle_config(path: &Path) -> Result<LifecycleConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LifecycleConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile =
        serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = LifecycleConfig::default();
    if let Some(section) = file.lifecycle {
        // Normalize Some(0) → None: a zero-second limit would kill every run
        // instantly; treat it as "no limit" (audit: zero-value safety).
        cfg.run_timeout_secs = match section.run_timeout_secs {
            Some(0) | None => None,
            Some(n) => Some(n),
        };
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

    /// Missing file / missing section / missing field → None (no limit).
    #[test]
    fn run_timeout_defaults_none_when_absent() {
        let tmp = TempDir::new().unwrap();
        // file missing
        let c1 = load_lifecycle_config(&tmp.path().join("none.yaml")).unwrap();
        assert_eq!(c1.run_timeout_secs, None);
        assert_eq!(c1, LifecycleConfig::default());
        // lifecycle section absent
        let p2 = write_yaml(tmp.path(), "pii:\n  scanner: regex_basic\n");
        assert_eq!(load_lifecycle_config(&p2).unwrap().run_timeout_secs, None);
        // lifecycle section present, field absent
        let p3 = write_yaml(tmp.path(), "lifecycle:\n  future_field: hi\n");
        assert_eq!(load_lifecycle_config(&p3).unwrap().run_timeout_secs, None);
    }

    /// Explicit positive value is honored.
    #[test]
    fn run_timeout_parses_positive_value() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lifecycle:\n  run_timeout_secs: 1800\n");
        assert_eq!(
            load_lifecycle_config(&p).unwrap().run_timeout_secs,
            Some(1800)
        );
    }

    /// Zero is normalized to None (no instant-kill foot-gun).
    #[test]
    fn run_timeout_zero_normalizes_to_none() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lifecycle:\n  run_timeout_secs: 0\n");
        assert_eq!(load_lifecycle_config(&p).unwrap().run_timeout_secs, None);
    }

    /// Wrong-typed value is rejected as `YamlParse` (no panic) so the caller
    /// conservatively defaults to None.
    #[test]
    fn wrong_typed_run_timeout_returns_err_no_panic() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lifecycle:\n  run_timeout_secs: not-a-number\n");
        assert!(matches!(
            load_lifecycle_config(&p),
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Structurally invalid YAML returns Err (caller warn-and-defaults).
    #[test]
    fn invalid_yaml_returns_err_no_panic() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lifecycle:\n  : :: not yaml\n");
        assert!(matches!(
            load_lifecycle_config(&p),
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Unknown subkey is silently ignored (forward-compat).
    #[test]
    fn unknown_lifecycle_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "lifecycle:\n  future_field: hello\n  run_timeout_secs: 600\n",
        );
        assert_eq!(
            load_lifecycle_config(&p).unwrap().run_timeout_secs,
            Some(600)
        );
    }

    /// Coexists with other namespaces (quiz/pii/goal) in the same file.
    #[test]
    fn coexists_with_other_namespaces() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "goal:\n  content_verify: true\nlifecycle:\n  run_timeout_secs: 900\npii:\n  scanner: regex_basic\n",
        );
        assert_eq!(
            load_lifecycle_config(&p).unwrap().run_timeout_secs,
            Some(900)
        );
    }
}
