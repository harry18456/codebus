//! `lint.fix.*` config loader for v3-fix-trust-agent Fix Loop Configuration.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! lint:
//!   fix:
//!     enabled: true            # default: true
//! ```
//!
//! Defaults apply when the config file is absent OR the `lint.fix` section
//! is missing OR an individual field is missing. Unknown keys are silently
//! ignored (forward compat) — this includes the legacy `outer_ping_max` key
//! left over from v3-lint configs, which v3-fix-trust-agent has retired.
//!
//! CLI flag `--no-fix` is merged on top of this at the call site (see
//! `LintFixConfig::merge_cli_overrides`). The previously-recognized
//! `--fix-max-iter` flag was removed in v3-fix-trust-agent and is no longer
//! part of any flow.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Effective fix-loop configuration after merging file + CLI overrides.
///
/// v3-fix-trust-agent intentionally has only one knob: `enabled`. The
/// previously-tunable `outer_ping_max` was removed when the multi-spawn
/// outer ping mechanism was replaced by single-shot `Fix Single-Shot
/// Verification` (no caps to set when there is no loop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintFixConfig {
    pub enabled: bool,
}

impl Default for LintFixConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl LintFixConfig {
    /// Apply CLI flag overrides. `--no-fix` (when present) forces `enabled`
    /// to `false`.
    pub fn merge_cli_overrides(mut self, no_fix: bool) -> Self {
        if no_fix {
            self.enabled = false;
        }
        self
    }
}

/// Intermediate YAML shapes for parsing — top-level `lint:` mapping with a
/// nested `fix:` mapping. Both are optional. Unknown keys silently ignored.
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

/// Per Fix Loop Configuration spec scenario "Legacy outer_ping_max key is
/// silently ignored": this struct accepts arbitrary extra keys (including
/// the retired `outer_ping_max`) without erroring. Serde drops unknown
/// fields by default.
#[derive(Debug, Default, Deserialize)]
struct LintFixSection {
    enabled: Option<bool>,
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

    /// Spec: "Default config enables fix"
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_lint_fix_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert!(cfg.enabled);
    }

    #[test]
    fn default_when_no_lint_section() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "emoji: on\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
    }

    #[test]
    fn default_when_no_fix_subsection() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  disabled_rules: []\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
    }

    #[test]
    fn user_overrides_enabled_to_false() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: false\n");
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(!cfg.enabled);
    }

    /// Spec: "Legacy outer_ping_max key is silently ignored"
    #[test]
    fn legacy_outer_ping_max_key_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "lint:\n  fix:\n    enabled: true\n    outer_ping_max: 10\n",
        );
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
        // No public field for outer_ping_max — its value has no observable effect.
    }

    #[test]
    fn unknown_keys_silently_ignored_forward_compat() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "lint:\n  fix:\n    enabled: true\n    future_field: hello\n",
        );
        let cfg = load_lint_fix_config(&p).unwrap();
        assert!(cfg.enabled);
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
        let cfg = LintFixConfig { enabled: true };
        let merged = cfg.merge_cli_overrides(true);
        assert!(!merged.enabled);
    }

    #[test]
    fn merge_with_no_overrides_returns_input_unchanged() {
        let cfg = LintFixConfig { enabled: false };
        let merged = cfg.merge_cli_overrides(false);
        assert_eq!(merged, cfg);
    }
}
