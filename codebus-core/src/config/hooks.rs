//! `hooks.*` config loader for the `pretooluse-image-block-toggle` change.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! hooks:
//!   read_image_block: true   # default: true (fail-safe to block images)
//! ```
//!
//! The `hooks` namespace was introduced to host the runtime gate for
//! `codebus hook check-read`. Defaults apply when the config file is
//! absent OR the `hooks` section is missing OR an individual field is
//! missing. Unknown keys are silently ignored (forward-compat) so future
//! hook toggles can be added without breaking existing configs.
//!
//! Call site convention (see spec `lint-feedback-loop` Requirement
//! `PII Image Read Hook Installation`): when this loader returns Err
//! (yaml parse failure), the caller SHALL log a warning and fall back
//! to `HooksConfig::default()` — fail-safe to block.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Effective hooks configuration.
///
/// `read_image_block` gates the runtime behavior of
/// `codebus hook check-read`: when false the hook subcommand always
/// allows the Read tool invocation; when true it executes the
/// blocklist + fail-closed logic defined in the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HooksConfig {
    pub read_image_block: bool,
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            read_image_block: true,
        }
    }
}

/// Intermediate YAML shape — top-level `hooks:` mapping. Both the
/// section and the inner field are optional. Unknown keys silently
/// ignored (forward-compat).
#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    hooks: Option<HooksSection>,
}

#[derive(Debug, Default, Deserialize)]
struct HooksSection {
    #[serde(default)]
    read_image_block: Option<bool>,
}

/// Load `hooks` config from `path`. Returns defaults when the file does
/// not exist OR the `hooks` section is absent OR the field is absent.
/// Returns an Err only when the file exists but cannot be read (IO
/// error) or is structurally invalid YAML — callers SHALL fall back to
/// `HooksConfig::default()` on Err to keep the hook subcommand safe
/// (fail-safe to block).
pub fn load_hooks_config(path: &Path) -> Result<HooksConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HooksConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile =
        serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = HooksConfig::default();
    if let Some(hooks) = file.hooks {
        if let Some(v) = hooks.read_image_block {
            cfg.read_image_block = v;
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// Default: `HooksConfig::default()` reflects the design Decision
    /// "預設 true（opt-out），absent → true" — the field is on.
    #[test]
    fn default_is_block_on() {
        let cfg = HooksConfig::default();
        assert!(cfg.read_image_block);
    }

    /// File not found → defaults (block).
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_hooks_config(&tmp.path().join("nonexistent.yaml")).unwrap();
        assert!(cfg.read_image_block);
    }

    /// File exists, no hooks section → defaults (block).
    #[test]
    fn default_when_no_hooks_section() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: true\n");
        let cfg = load_hooks_config(&p).unwrap();
        assert!(cfg.read_image_block);
    }

    /// hooks section present but field absent → defaults (block).
    #[test]
    fn default_when_no_read_image_block_field() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "hooks:\n  some_future_knob: 42\n");
        let cfg = load_hooks_config(&p).unwrap();
        assert!(cfg.read_image_block);
    }

    /// Explicit `false` → block disabled.
    #[test]
    fn user_overrides_to_false() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "hooks:\n  read_image_block: false\n");
        let cfg = load_hooks_config(&p).unwrap();
        assert!(!cfg.read_image_block);
    }

    /// Explicit `true` round-trips through serde.
    #[test]
    fn user_overrides_to_true() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "hooks:\n  read_image_block: true\n");
        let cfg = load_hooks_config(&p).unwrap();
        assert!(cfg.read_image_block);
    }

    /// Forward-compat: unknown subkeys in `hooks:` are silently ignored.
    #[test]
    fn unknown_hooks_subkeys_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "hooks:\n  read_image_block: false\n  future_hook_toggle: true\n",
        );
        let cfg = load_hooks_config(&p).unwrap();
        assert!(!cfg.read_image_block);
    }

    /// Non-boolean `read_image_block` is a structural error — return
    /// Err so the caller can fall back to default (fail-safe to block).
    /// Spec scenario "Malformed config yaml resolves read_image_block
    /// to true (fail-safe block)" — the call-site fallback is what
    /// preserves the fail-safe; this loader returns Err.
    #[test]
    fn non_bool_read_image_block_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "hooks:\n  read_image_block: \"yes\"\n");
        let result = load_hooks_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Structurally invalid yaml returns Err so the caller can fall
    /// back to default. The fail-safe-to-block guarantee lives at the
    /// call site, not in the loader.
    #[test]
    fn malformed_yaml_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "hooks:\n  : :: not yaml\n");
        let result = load_hooks_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }
}
