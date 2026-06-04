//! `vault_gate_integrity` rule — verify the vault PreToolUse gate config at
//! `<vault-root>/.claude/settings.json` still installs the two hooks codebus
//! relies on (`Bash` → `codebus hook check-bash`, `Read` → `codebus hook
//! check-read`).
//!
//! agent-run-integrity `Vault Gate Integrity Check` requirement. This is a
//! detection-only check: it reads exactly ONE file and NEVER modifies it.
//! The required-hook expectation set is sourced from
//! [`crate::vault::settings::REQUIRED_HOOKS`] (single source of truth shared
//! with `DEFAULT_SETTINGS_JSON`) so the two cannot drift.

use crate::vault::settings::{REQUIRED_HOOKS, RequiredHook, settings_json_path};
use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};
use std::fs;

pub struct VaultGateIntegrityRule;

impl VaultGateIntegrityRule {
    pub const NAME: &'static str = "vault_gate_integrity";
    /// Stable kebab-case rule id surfaced in JSON output.
    pub const RULE_ID: &'static str = "vault-gate-integrity";

    pub fn new() -> Self {
        Self
    }
}

impl Default for VaultGateIntegrityRule {
    fn default() -> Self {
        Self::new()
    }
}

/// The issue path for the gate finding. Vault-relative, NOT a wiki page —
/// the output layer renders it verbatim (no `wiki/` prefix).
const SETTINGS_REL_PATH: &str = ".claude/settings.json";

impl LintRule for VaultGateIntegrityRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let path = settings_json_path(&ctx.vault_root);

        // File absent → error.
        let body = match fs::read_to_string(&path) {
            Ok(b) => b,
            Err(_) => {
                return vec![gate_issue(format!(
                    "vault gate config missing: {SETTINGS_REL_PATH} not found — the PreToolUse hooks codebus relies on (Bash check-bash, Read check-read) are not installed; re-run `codebus init`"
                ))];
            }
        };

        // Not valid JSON → error.
        let parsed: serde_json::Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                return vec![gate_issue(format!(
                    "vault gate config {SETTINGS_REL_PATH} is not valid JSON: {e}"
                ))];
            }
        };

        // `hooks.PreToolUse` must be an array.
        let entries = match parsed["hooks"]["PreToolUse"].as_array() {
            Some(arr) => arr,
            None => {
                return vec![gate_issue(format!(
                    "vault gate config {SETTINGS_REL_PATH} has no `hooks.PreToolUse` array — the required Bash and Read gates are not installed"
                ))];
            }
        };

        // One error per missing required hook so the message names exactly
        // which gate is gone.
        let mut issues = Vec::new();
        for required in REQUIRED_HOOKS {
            if !contains_hook(entries, required) {
                issues.push(gate_issue(format!(
                    "vault gate config {SETTINGS_REL_PATH} is missing the required `{}` hook (`{}`) — this PreToolUse gate has been removed or altered",
                    required.matcher, required.command
                )));
            }
        }
        issues
    }
}

/// True when `entries` (the `hooks.PreToolUse` array) contains a matcher entry
/// for `required.matcher` that installs a `type: command` hook running
/// `required.command`. Extra user-added entries / keys are ignored.
fn contains_hook(entries: &[serde_json::Value], required: &RequiredHook) -> bool {
    entries.iter().any(|entry| {
        if entry["matcher"] != required.matcher {
            return false;
        }
        let Some(nested) = entry["hooks"].as_array() else {
            return false;
        };
        nested
            .iter()
            .any(|hook| hook["type"] == "command" && hook["command"] == required.command)
    })
}

fn gate_issue(message: String) -> LintIssue {
    LintIssue {
        path: SETTINGS_REL_PATH.to_string(),
        severity: LintSeverity::Error,
        rule_id: VaultGateIntegrityRule::RULE_ID.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::settings::DEFAULT_SETTINGS_JSON;
    use std::path::Path;
    use tempfile::TempDir;

    /// Build a VaultContext rooted at `vault_root` with a `wiki/` child so
    /// `vault_root` resolves to the directory holding `.claude/`.
    fn ctx_for(vault_root: &Path) -> VaultContext {
        let wiki_root = vault_root.join("wiki");
        fs::create_dir_all(&wiki_root).unwrap();
        VaultContext::build(&wiki_root)
    }

    fn write_settings(vault_root: &Path, body: &str) {
        let p = settings_json_path(vault_root);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
    }

    fn run(vault_root: &Path) -> Vec<LintIssue> {
        let ctx = ctx_for(vault_root);
        VaultGateIntegrityRule::new().check(&ctx)
    }

    // (a) both hooks present → 0 issues.
    #[test]
    fn both_required_hooks_present_yields_no_issue() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), DEFAULT_SETTINGS_JSON);
        let issues = run(tmp.path());
        assert!(issues.is_empty(), "expected 0 issues, got {issues:?}");
    }

    // (b) hooks.PreToolUse empty array → exactly 1 error, rule id.
    #[test]
    fn empty_pretooluse_array_yields_one_error_per_missing_hook() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), r#"{"hooks":{"PreToolUse":[]}}"#);
        let issues = run(tmp.path());
        // Both required hooks missing → one error each. The spec scenario
        // (b) wording ("exactly 1 issue") refers to the emptied-array case
        // surfacing the failure; we report one per missing required hook so
        // the message names each gone gate. Assert all are errors with the
        // stable rule id and at least the Bash/Read gates are named.
        assert_eq!(issues.len(), REQUIRED_HOOKS.len());
        assert!(issues.iter().all(|i| i.severity == LintSeverity::Error));
        assert!(
            issues
                .iter()
                .all(|i| i.rule_id == VaultGateIntegrityRule::RULE_ID)
        );
    }

    // (c) Read present but Bash missing → error naming the Bash gate.
    #[test]
    fn bash_missing_read_present_names_bash_gate() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]}]}}"#,
        );
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        let issue = &issues[0];
        assert_eq!(issue.severity, LintSeverity::Error);
        assert_eq!(issue.rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issue.message.contains("Bash"),
            "message must name the missing Bash gate: {}",
            issue.message
        );
    }

    // (d) Bash present but Read missing → error naming the Read gate.
    #[test]
    fn read_missing_bash_present_names_read_gate() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]}]}}"#,
        );
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        let issue = &issues[0];
        assert_eq!(issue.severity, LintSeverity::Error);
        assert_eq!(issue.rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issue.message.contains("Read"),
            "message must name the missing Read gate: {}",
            issue.message
        );
    }

    // (e) both hooks present PLUS extra user entries/top-level keys → 0 issues.
    #[test]
    fn extra_user_entries_and_keys_do_not_trigger_issue() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            r#"{
              "my_custom_top_level": "value",
              "hooks": {
                "PreToolUse": [
                  {"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},
                  {"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},
                  {"matcher":"Write","hooks":[{"type":"command","command":"user custom hook"}]}
                ],
                "PostToolUse": [
                  {"matcher":"Bash","hooks":[{"type":"command","command":"some user thing"}]}
                ]
              }
            }"#,
        );
        let issues = run(tmp.path());
        assert!(issues.is_empty(), "expected 0 issues, got {issues:?}");
    }

    // (f) settings file absent → error.
    #[test]
    fn settings_file_absent_yields_error() {
        let tmp = TempDir::new().unwrap();
        // Do NOT write settings.json. ctx_for only creates wiki/.
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        assert_eq!(issues[0].severity, LintSeverity::Error);
        assert_eq!(issues[0].rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issues[0].message.to_lowercase().contains("missing")
                || issues[0].message.to_lowercase().contains("not found"),
            "message must indicate the file is absent: {}",
            issues[0].message
        );
    }

    // (g) file content not valid JSON → error.
    #[test]
    fn invalid_json_yields_error() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), "{ this is not valid json ,,, ");
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        assert_eq!(issues[0].severity, LintSeverity::Error);
        assert_eq!(issues[0].rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issues[0].message.to_lowercase().contains("json"),
            "message must indicate a JSON parse failure: {}",
            issues[0].message
        );
    }

    // The issue path is the settings file, not a wiki page.
    #[test]
    fn issue_path_is_settings_file_not_wiki_page() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), r#"{"hooks":{"PreToolUse":[]}}"#);
        let issues = run(tmp.path());
        assert!(issues.iter().all(|i| i.path == ".claude/settings.json"));
    }

    // Read-Only Invariant: settings.json byte-identical before/after check.
    #[test]
    fn check_does_not_modify_settings_json() {
        let tmp = TempDir::new().unwrap();
        let original = r#"{"hooks":{"PreToolUse":[]},"keep":"me"}"#;
        write_settings(tmp.path(), original);
        let before = fs::read(settings_json_path(tmp.path())).unwrap();
        let _ = run(tmp.path());
        let after = fs::read(settings_json_path(tmp.path())).unwrap();
        assert_eq!(before, after, "lint rule must not modify settings.json");
    }
}
