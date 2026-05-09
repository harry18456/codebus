//! Write `<vault_root>/.claude/settings.json` containing the PreToolUse
//! Bash hook for the fix sandbox.
//!
//! Per v3-fix-trust-agent `Fix Bash Hook Installation` requirement:
//! init writes a vault-internal settings.json with a hook that routes
//! every Bash tool invocation through `codebus hook check-bash`. The
//! hook is the actual hard gate (the `--allowedTools` Bash specifier
//! is auto-approval scope only — see design.md §"PreToolUse Bash hook").
//!
//! Write-if-missing semantics: an existing settings.json is preserved
//! byte-identical so user customizations survive re-init.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOutcome {
    Written,
    AlreadyPresent,
}

/// `<vault_root>/.claude/settings.json` path (deterministic helper for
/// callers / tests).
pub fn settings_json_path(vault_root: &Path) -> PathBuf {
    vault_root.join(".claude").join("settings.json")
}

/// Default content for a fresh settings.json — registers a single
/// PreToolUse hook on Bash that delegates to `codebus hook check-bash`.
pub const DEFAULT_SETTINGS_JSON: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "codebus hook check-bash"
          }
        ]
      }
    ]
  }
}
"#;

/// Write `<vault_root>/.claude/settings.json` containing the default
/// PreToolUse Bash hook config when the file does not already exist.
/// Returns `AlreadyPresent` if the file exists (no overwrite).
pub fn write_settings_if_missing(vault_root: &Path) -> io::Result<SettingsOutcome> {
    let path = settings_json_path(vault_root);
    if path.exists() {
        return Ok(SettingsOutcome::AlreadyPresent);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, DEFAULT_SETTINGS_JSON)?;
    Ok(SettingsOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_settings_json_on_fresh_vault() {
        let tmp = TempDir::new().unwrap();
        let outcome = write_settings_if_missing(tmp.path()).unwrap();
        assert_eq!(outcome, SettingsOutcome::Written);
        let p = settings_json_path(tmp.path());
        assert!(p.exists());
    }

    #[test]
    fn settings_json_parses_as_valid_json_with_pretooluse_bash_hook() {
        let tmp = TempDir::new().unwrap();
        write_settings_if_missing(tmp.path()).unwrap();
        let body = fs::read_to_string(settings_json_path(tmp.path())).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let hooks = &parsed["hooks"]["PreToolUse"];
        assert!(hooks.is_array(), "hooks.PreToolUse must be an array");
        let entries = hooks.as_array().unwrap();
        assert!(!entries.is_empty(), "PreToolUse must have at least one entry");
        // First entry matches Bash and invokes codebus hook check-bash.
        assert_eq!(entries[0]["matcher"], "Bash");
        let nested = entries[0]["hooks"].as_array().unwrap();
        assert_eq!(nested[0]["type"], "command");
        assert_eq!(nested[0]["command"], "codebus hook check-bash");
    }

    #[test]
    fn does_not_overwrite_existing_settings_json() {
        let tmp = TempDir::new().unwrap();
        let custom = "{\"hooks\":{\"PreToolUse\":[]},\"my_custom\":\"value\"}";
        let p = settings_json_path(tmp.path());
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, custom).unwrap();
        let outcome = write_settings_if_missing(tmp.path()).unwrap();
        assert_eq!(outcome, SettingsOutcome::AlreadyPresent);
        // Byte-identical to original.
        assert_eq!(fs::read_to_string(&p).unwrap(), custom);
    }

    #[test]
    fn settings_json_path_resolves_under_vault_dot_claude() {
        let p = settings_json_path(Path::new("/some/repo/.codebus"));
        let s = p.to_string_lossy();
        assert!(s.contains(".claude"));
        assert!(s.ends_with("settings.json"));
    }
}
