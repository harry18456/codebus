//! `lint.fix.*` config loader for v3-lint Fix Loop Configuration.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! lint:
//!   fix:
//!     enabled: true            # default: true
//!     outer_ping_max: 2        # default: 2 (positive integer)
//! ```
//!
//! Defaults apply when the config file is absent OR the `lint.fix` section
//! is missing OR an individual field is missing. Unknown keys are silently
//! ignored (forward compat).
//!
//! CLI flags `--no-fix` and `--fix-max-iter <N>` are merged on top of this
//! at the call site (see `LintFixConfig::merge_cli_overrides`).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Effective fix-loop configuration after merging file + CLI overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintFixConfig {
    pub enabled: bool,
    pub outer_ping_max: u32,
}

impl Default for LintFixConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            outer_ping_max: 2,
        }
    }
}

impl LintFixConfig {
    /// Apply CLI flag overrides. `--no-fix` (when present) takes precedence
    /// over `--fix-max-iter` per Fix Loop Configuration spec scenario
    /// "--no-fix wins when both override flags are present".
    pub fn merge_cli_overrides(mut self, no_fix: bool, fix_max_iter: Option<u32>) -> Self {
        if no_fix {
            self.enabled = false;
            // `--fix-max-iter` has no observable effect when --no-fix wins.
            return self;
        }
        if let Some(n) = fix_max_iter {
            self.outer_ping_max = n;
        }
        self
    }
}

/// Intermediate YAML shapes for parsing — top-level `lint:` mapping with a
/// nested `fix:` mapping. Both are optional.
#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    lint: Option<LintSection>,
}

#[derive(Debug, Default, Deserialize)]
struct LintSection {
    #[serde(default)]
    fix: Option<LintFixSection>,
}

#[derive(Debug, Default, Deserialize)]
struct LintFixSection {
    enabled: Option<bool>,
    outer_ping_max: Option<u32>,
}

/// Default config path: `~/.codebus/config.yaml`. Returns `None` if the
/// home directory cannot be resolved.
pub fn default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codebus").join("config.yaml"))
}

/// Load `lint.fix` config from `path`. Returns defaults when the file does
/// not exist OR the `lint.fix` section is absent. Returns an Err only when
/// the file exists but cannot be read (IO error) or is structurally
/// invalid YAML — callers SHALL fall back to defaults on Err to keep the
/// CLI resilient against config-file mistakes.
pub fn load_lint_fix_config(path: &Path) -> Result<LintFixConfig, ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LintFixConfig::default());
        }
        Err(err) => return Err(ConfigLoadError::Io(err)),
    };
    let file: ConfigFile = serde_yaml::from_str(&body).map_err(ConfigLoadError::YamlParse)?;
    let mut cfg = LintFixConfig::default();
    if let Some(lint) = file.lint {
        if let Some(fix) = lint.fix {
            if let Some(e) = fix.enabled {
                cfg.enabled = e;
            }
            if let Some(n) = fix.outer_ping_max {
                cfg.outer_ping_max = n;
            }
        }
    }
    Ok(cfg)
}

#[derive(Debug)]
pub enum ConfigLoadError {
    Io(std::io::Error),
    YamlParse(serde_yaml::Error),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::Io(e) => write!(f, "config file io: {e}"),
            ConfigLoadError::YamlParse(e) => write!(f, "config file yaml parse: {e}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// Spec: "Default config enables fix with outer_ping_max two"
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_lint_fix_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.outer_ping_max, 2);
    }

    #[test]
    fn default_when_no_lint_section() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "emoji: on\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.outer_ping_max, 2);
    }

    #[test]
    fn default_when_no_fix_subsection() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  disabled_rules: []\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.outer_ping_max, 2);
    }

    #[test]
    fn user_overrides_enabled_to_false() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: false\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(!cfg.enabled);
        // outer_ping_max stays at default
        assert_eq!(cfg.outer_ping_max, 2);
    }

    #[test]
    fn user_overrides_outer_ping_max() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    outer_ping_max: 5\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.outer_ping_max, 5);
    }

    #[test]
    fn unknown_keys_silently_ignored_forward_compat() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "lint:\n  fix:\n    enabled: true\n    outer_ping_max: 3\n    future_field: hello\n",
        );
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.outer_ping_max, 3);
    }

    #[test]
    fn invalid_yaml_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    : :: not yaml\n");
        let result = load_lint_fix_config(&p);
        assert!(matches!(result, Err(ConfigLoadError::YamlParse(_))));
    }

    /// Spec: "--no-fix flag disables fix even when config enables it"
    #[test]
    fn no_fix_cli_override_disables_even_when_config_enables() {
        let cfg = LintFixConfig {
            enabled: true,
            outer_ping_max: 2,
        };
        let merged = cfg.merge_cli_overrides(true, None);
        assert!(!merged.enabled);
    }

    /// Spec: "--fix-max-iter overrides config outer_ping_max"
    #[test]
    fn fix_max_iter_cli_override_replaces_config_value() {
        let cfg = LintFixConfig {
            enabled: true,
            outer_ping_max: 2,
        };
        let merged = cfg.merge_cli_overrides(false, Some(5));
        assert!(merged.enabled);
        assert_eq!(merged.outer_ping_max, 5);
    }

    /// Spec: "--no-fix wins when both override flags are present"
    #[test]
    fn no_fix_wins_over_fix_max_iter_when_both_present() {
        let cfg = LintFixConfig {
            enabled: true,
            outer_ping_max: 2,
        };
        let merged = cfg.merge_cli_overrides(true, Some(5));
        assert!(!merged.enabled);
        // outer_ping_max may or may not change, but `enabled = false`
        // means the value has no observable effect.
    }

    #[test]
    fn merge_with_no_overrides_returns_input_unchanged() {
        let cfg = LintFixConfig {
            enabled: false,
            outer_ping_max: 7,
        };
        let merged = cfg.merge_cli_overrides(false, None);
        assert_eq!(merged, cfg);
    }
}
