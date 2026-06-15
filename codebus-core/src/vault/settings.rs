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
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsOutcome {
    Written,
    AlreadyPresent,
}

/// One required PreToolUse hook the vault gate relies on: a `matcher` tool
/// name paired with the `command` codebus expects it to route to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiredHook {
    /// PreToolUse matcher (the agent tool name), e.g. `Bash` / `Read`.
    pub matcher: &'static str,
    /// The `type: command` hook command the matcher must install.
    pub command: &'static str,
}

/// Single source of truth for the hooks `DEFAULT_SETTINGS_JSON` installs and
/// the lint `vault-gate-integrity` rule verifies. Both the default content
/// (its intent) and the lint rule reference this set so they cannot drift.
///
/// - `Bash` → `codebus hook check-bash` (Fix Bash Hook Installation)
/// - `Read` → `codebus hook check-read` (PII Image Read Hook Installation)
/// - `Glob` → `codebus hook check-read` (check-read-vault-containment)
/// - `Grep` → `codebus hook check-read` (check-read-vault-containment)
pub const REQUIRED_HOOKS: &[RequiredHook] = &[
    RequiredHook {
        matcher: "Bash",
        command: "codebus hook check-bash",
    },
    RequiredHook {
        matcher: "Read",
        command: "codebus hook check-read",
    },
    // check-read-vault-containment: route Glob/Grep through check-read so the
    // vault-root containment boundary covers the search tools, not just Read.
    RequiredHook {
        matcher: "Glob",
        command: "codebus hook check-read",
    },
    RequiredHook {
        matcher: "Grep",
        command: "codebus hook check-read",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveBasenameMatcher {
    SuffixAsciiCaseInsensitive(&'static str),
    ContainsAsciiCaseInsensitive(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensitiveBasenameRule {
    pub matcher: SensitiveBasenameMatcher,
    pub claude_read_rule: &'static str,
}

impl SensitiveBasenameRule {
    pub fn matches(&self, basename: &str) -> bool {
        let lower = basename.to_ascii_lowercase();
        match self.matcher {
            SensitiveBasenameMatcher::SuffixAsciiCaseInsensitive(suffix) => lower.ends_with(suffix),
            SensitiveBasenameMatcher::ContainsAsciiCaseInsensitive(needle) => {
                lower.contains(needle)
            }
        }
    }
}

/// Single source of truth for sensitive key-file basename patterns enforced
/// by Claude Code `permissions.deny`, verified by `vault-gate-integrity`, and
/// retained as a Read-hook defense-in-depth backstop.
pub const SENSITIVE_BASENAME_RULES: &[SensitiveBasenameRule] = &[
    SensitiveBasenameRule {
        matcher: SensitiveBasenameMatcher::SuffixAsciiCaseInsensitive(".pem"),
        claude_read_rule: "Read(**/*.[pP][eE][mM])",
    },
    SensitiveBasenameRule {
        matcher: SensitiveBasenameMatcher::SuffixAsciiCaseInsensitive(".key"),
        claude_read_rule: "Read(**/*.[kK][eE][yY])",
    },
    SensitiveBasenameRule {
        matcher: SensitiveBasenameMatcher::ContainsAsciiCaseInsensitive("id_rsa"),
        claude_read_rule: "Read(**/*[iI][dD]_[rR][sS][aA]*)",
    },
];

pub fn matches_sensitive_basename(basename: &str) -> bool {
    SENSITIVE_BASENAME_RULES
        .iter()
        .any(|rule| rule.matches(basename))
}

/// `<vault_root>/.claude/settings.json` path (deterministic helper for
/// callers / tests).
pub fn settings_json_path(vault_root: &Path) -> PathBuf {
    vault_root.join(".claude").join("settings.json")
}

/// Default content for a fresh settings.json — registers four PreToolUse
/// hooks: Bash (delegates to `codebus hook check-bash` per
/// `Fix Bash Hook Installation`) and Read / Glob / Grep (each delegates to
/// `codebus hook check-read`, enforcing the vault-root containment boundary
/// per `check-read-vault-containment` AND the image / sensitive denylist per
/// `PII Image Read Hook Installation`).
pub static DEFAULT_SETTINGS_JSON: LazyLock<String> = LazyLock::new(build_default_settings_json);

pub fn default_settings_json() -> &'static str {
    DEFAULT_SETTINGS_JSON.as_str()
}

fn build_default_settings_json() -> String {
    let pretooluse: Vec<serde_json::Value> = REQUIRED_HOOKS
        .iter()
        .map(|required| {
            serde_json::json!({
                "matcher": required.matcher,
                "hooks": [
                    {
                        "type": "command",
                        "command": required.command
                    }
                ]
            })
        })
        .collect();
    let deny: Vec<&str> = SENSITIVE_BASENAME_RULES
        .iter()
        .map(|rule| rule.claude_read_rule)
        .collect();
    let value = serde_json::json!({
        "hooks": {
            "PreToolUse": pretooluse
        },
        "permissions": {
            "deny": deny
        }
    });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&value).expect("default settings JSON must serialize")
    )
}

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
    fs::write(&path, default_settings_json())?;
    Ok(SettingsOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const REQUIRED_SENSITIVE_DENY_RULES: &[&str] = &[
        "Read(**/*.[pP][eE][mM])",
        "Read(**/*.[kK][eE][yY])",
        "Read(**/*[iI][dD]_[rR][sS][aA]*)",
    ];

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
        assert!(
            !entries.is_empty(),
            "PreToolUse must have at least one entry"
        );
        // First entry matches Bash and invokes codebus hook check-bash.
        assert_eq!(entries[0]["matcher"], "Bash");
        let nested = entries[0]["hooks"].as_array().unwrap();
        assert_eq!(nested[0]["type"], "command");
        assert_eq!(nested[0]["command"], "codebus hook check-bash");
    }

    // --- pretooluse-image-block task 2.1 — settings.json carries BOTH
    // the Bash matcher entry (from Fix Bash Hook Installation) AND the
    // Read matcher entry (from PII Image Read Hook Installation) on a
    // fresh vault.

    #[test]
    fn settings_json_contains_both_bash_and_read_matcher_entries() {
        let tmp = TempDir::new().unwrap();
        write_settings_if_missing(tmp.path()).unwrap();
        let body = fs::read_to_string(settings_json_path(tmp.path())).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        let entries = parsed["hooks"]["PreToolUse"]
            .as_array()
            .expect("hooks.PreToolUse must be an array");
        assert!(
            entries.len() >= 4,
            "PreToolUse must carry at least four matcher entries (Bash + Read + Glob + Grep), got {}",
            entries.len()
        );

        let find_entry = |matcher: &str, command: &str| -> bool {
            entries.iter().any(|entry| {
                if entry["matcher"] != matcher {
                    return false;
                }
                let nested = match entry["hooks"].as_array() {
                    Some(arr) => arr,
                    None => return false,
                };
                nested
                    .iter()
                    .any(|hook| hook["type"] == "command" && hook["command"] == command)
            })
        };

        assert!(
            find_entry("Bash", "codebus hook check-bash"),
            "PreToolUse must contain Bash matcher entry invoking `codebus hook check-bash`"
        );
        assert!(
            find_entry("Read", "codebus hook check-read"),
            "PreToolUse must contain Read matcher entry invoking `codebus hook check-read`"
        );
        assert!(
            find_entry("Glob", "codebus hook check-read"),
            "PreToolUse must contain Glob matcher entry invoking `codebus hook check-read`"
        );
        assert!(
            find_entry("Grep", "codebus hook check-read"),
            "PreToolUse must contain Grep matcher entry invoking `codebus hook check-read`"
        );
    }

    #[test]
    fn sensitive_basename_rules_match_ascii_case_insensitively() {
        for basename in [
            "server.pem",
            "server.PEM",
            "private.key",
            "private.KEY",
            "id_rsa",
            "ID_RSA",
            "backup-ID_RSA.txt",
        ] {
            assert!(
                matches_sensitive_basename(basename),
                "expected sensitive basename hit for {basename}"
            );
        }
        assert!(
            !matches_sensitive_basename("readme.md"),
            "non-sensitive basename must not match"
        );
    }

    #[test]
    fn sensitive_basename_rule_set_exposes_required_claude_read_rules() {
        let got: Vec<&str> = SENSITIVE_BASENAME_RULES
            .iter()
            .map(|rule| rule.claude_read_rule)
            .collect();
        assert_eq!(got, REQUIRED_SENSITIVE_DENY_RULES);
    }

    #[test]
    fn default_settings_json_contains_sensitive_basename_permissions_deny() {
        let parsed: serde_json::Value = serde_json::from_str(default_settings_json()).unwrap();
        let deny = parsed["permissions"]["deny"]
            .as_array()
            .expect("permissions.deny must be an array");
        let got: Vec<&str> = deny
            .iter()
            .map(|v| v.as_str().expect("deny rule must be a string"))
            .collect();
        assert_eq!(got, REQUIRED_SENSITIVE_DENY_RULES);
        for rule in got {
            assert!(
                rule.contains('/'),
                "required sensitive deny rule must use forward slash: {rule}"
            );
            assert!(
                !rule.contains('\\'),
                "required sensitive deny rule must not use backslash: {rule}"
            );
            assert!(
                rule.contains('['),
                "required sensitive deny rule must use bracket classes: {rule}"
            );
        }
    }

    // --- agent-run-integrity task 2.1 — drift guard: DEFAULT_SETTINGS_JSON
    // must be consistent with the REQUIRED_HOOKS single source of truth. It
    // must parse and contain EXACTLY the required hooks (no more, no fewer),
    // so the lint `vault-gate-integrity` rule and the default content cannot
    // silently diverge.

    fn pretooluse_pairs(json: &str) -> Vec<(String, String)> {
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
        let entries = parsed["hooks"]["PreToolUse"]
            .as_array()
            .expect("hooks.PreToolUse must be an array");
        let mut pairs = Vec::new();
        for entry in entries {
            let matcher = entry["matcher"].as_str().unwrap_or_default().to_string();
            let nested = entry["hooks"].as_array().cloned().unwrap_or_default();
            for hook in nested {
                if hook["type"] == "command" {
                    if let Some(cmd) = hook["command"].as_str() {
                        pairs.push((matcher.clone(), cmd.to_string()));
                    }
                }
            }
        }
        pairs
    }

    #[test]
    fn default_settings_json_matches_required_hooks_exactly() {
        let pairs = pretooluse_pairs(default_settings_json());
        let expected: Vec<(String, String)> = REQUIRED_HOOKS
            .iter()
            .map(|h| (h.matcher.to_string(), h.command.to_string()))
            .collect();
        // Same length AND each required hook present (order-independent).
        assert_eq!(
            pairs.len(),
            expected.len(),
            "DEFAULT_SETTINGS_JSON PreToolUse hook count drifted from REQUIRED_HOOKS"
        );
        for req in &expected {
            assert!(
                pairs.contains(req),
                "DEFAULT_SETTINGS_JSON missing required hook {req:?}"
            );
        }
        for got in &pairs {
            assert!(
                expected.contains(got),
                "DEFAULT_SETTINGS_JSON has unexpected hook {got:?} not in REQUIRED_HOOKS"
            );
        }
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
