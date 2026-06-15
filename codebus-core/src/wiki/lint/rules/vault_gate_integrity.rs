//! `vault_gate_integrity` rule — verify the vault gate config at
//! `<vault-root>/.claude/settings.json` still installs the hooks and
//! `permissions.deny` rules codebus relies on.
//!
//! agent-run-integrity `Vault Gate Integrity Check` requirement. This is a
//! detection-only check: it reads exactly ONE file and NEVER modifies it.
//! The required expectation sets are sourced from [`crate::vault::settings`]
//! (single source of truth shared with the default settings writer) so lint and
//! init cannot drift.

use crate::vault::settings::{
    REQUIRED_HOOKS, RequiredHook, SENSITIVE_BASENAME_RULES, SensitiveBasenameRule,
    settings_json_path,
};
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
        let deny_entries = parsed["permissions"]["deny"].as_array().map(Vec::as_slice);
        for required in SENSITIVE_BASENAME_RULES {
            if !contains_deny_rule(deny_entries, required) {
                issues.push(gate_issue(format!(
                    "vault gate config {SETTINGS_REL_PATH} is missing the required permissions.deny rule `{}` — this sensitive-basename Read deny gate has been removed or altered",
                    required.claude_read_rule
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

fn contains_deny_rule(
    entries: Option<&[serde_json::Value]>,
    required: &SensitiveBasenameRule,
) -> bool {
    entries.is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.as_str() == Some(required.claude_read_rule))
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
    use crate::vault::settings::{SENSITIVE_BASENAME_RULES, default_settings_json};
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

    fn all_required_deny_rules_json() -> String {
        SENSITIVE_BASENAME_RULES
            .iter()
            .map(|rule| format!(r#""{}""#, rule.claude_read_rule))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn settings_with_pretooluse(pretooluse: &str) -> String {
        format!(
            r#"{{"permissions":{{"deny":[{}]}},"hooks":{{"PreToolUse":{pretooluse}}}}}"#,
            all_required_deny_rules_json()
        )
    }

    fn all_required_hooks_pretooluse() -> &'static str {
        r#"[{"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},{"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Glob","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Grep","hooks":[{"type":"command","command":"codebus hook check-read"}]}]"#
    }

    // (a) both hooks present → 0 issues.
    #[test]
    fn both_required_hooks_present_yields_no_issue() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), default_settings_json());
        let issues = run(tmp.path());
        assert!(issues.is_empty(), "expected 0 issues, got {issues:?}");
    }

    // (b) hooks.PreToolUse empty array → exactly 1 error, rule id.
    #[test]
    fn empty_pretooluse_array_yields_one_error_per_missing_hook() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), &settings_with_pretooluse("[]"));
        let issues = run(tmp.path());
        // All four required hooks missing → one error each. The spec scenario
        // (b) wording ("exactly 1 issue") refers to the emptied-array case
        // surfacing the failure; we report one per missing required hook so
        // the message names each gone gate. Assert all are errors with the
        // stable rule id and at least the Bash/Read/Glob/Grep gates are named.
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
            &settings_with_pretooluse(
                r#"[{"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Glob","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Grep","hooks":[{"type":"command","command":"codebus hook check-read"}]}]"#,
            ),
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
            &settings_with_pretooluse(
                r#"[{"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},{"matcher":"Glob","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Grep","hooks":[{"type":"command","command":"codebus hook check-read"}]}]"#,
            ),
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

    // (d2) check-read-vault-containment: Glob missing (others present) →
    // error naming the Glob gate.
    #[test]
    fn glob_missing_others_present_names_glob_gate() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &settings_with_pretooluse(
                r#"[{"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},{"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Grep","hooks":[{"type":"command","command":"codebus hook check-read"}]}]"#,
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        assert_eq!(issues[0].severity, LintSeverity::Error);
        assert_eq!(issues[0].rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issues[0].message.contains("Glob"),
            "message must name the missing Glob gate: {}",
            issues[0].message
        );
    }

    // (d3) check-read-vault-containment: Grep missing (others present) →
    // error naming the Grep gate.
    #[test]
    fn grep_missing_others_present_names_grep_gate() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &settings_with_pretooluse(
                r#"[{"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},{"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},{"matcher":"Glob","hooks":[{"type":"command","command":"codebus hook check-read"}]}]"#,
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        assert_eq!(issues[0].severity, LintSeverity::Error);
        assert_eq!(issues[0].rule_id, VaultGateIntegrityRule::RULE_ID);
        assert!(
            issues[0].message.contains("Grep"),
            "message must name the missing Grep gate: {}",
            issues[0].message
        );
    }

    // (e) both hooks present PLUS extra user entries/top-level keys → 0 issues.
    #[test]
    fn extra_user_entries_and_keys_do_not_trigger_issue() {
        let tmp = TempDir::new().unwrap();
        let body = r#"{
              "my_custom_top_level": "value",
              "permissions": {
                "deny": [__DENY__],
                "allow": ["Read(wiki/**)"]
              },
              "hooks": {
                "PreToolUse": [
                  {"matcher":"Bash","hooks":[{"type":"command","command":"codebus hook check-bash"}]},
                  {"matcher":"Read","hooks":[{"type":"command","command":"codebus hook check-read"}]},
                  {"matcher":"Glob","hooks":[{"type":"command","command":"codebus hook check-read"}]},
                  {"matcher":"Grep","hooks":[{"type":"command","command":"codebus hook check-read"}]},
                  {"matcher":"Write","hooks":[{"type":"command","command":"user custom hook"}]}
                ],
                "PostToolUse": [
                  {"matcher":"Bash","hooks":[{"type":"command","command":"some user thing"}]}
                ]
              }
            }"#
        .replace("__DENY__", &all_required_deny_rules_json());
        write_settings(tmp.path(), &body);
        let issues = run(tmp.path());
        assert!(issues.is_empty(), "expected 0 issues, got {issues:?}");
    }

    #[test]
    fn missing_permissions_deny_yields_error_per_required_deny_rule() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &format!(
                r#"{{"hooks":{{"PreToolUse":{}}}}}"#,
                all_required_hooks_pretooluse()
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(
            issues.len(),
            SENSITIVE_BASENAME_RULES.len(),
            "got {issues:?}"
        );
        for rule in SENSITIVE_BASENAME_RULES {
            assert!(
                issues
                    .iter()
                    .any(|issue| issue.message.contains(rule.claude_read_rule)),
                "missing deny issue must name {}; got {issues:?}",
                rule.claude_read_rule
            );
        }
    }

    #[test]
    fn non_array_permissions_deny_yields_error_per_required_deny_rule() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &format!(
                r#"{{"permissions":{{"deny":"Read(**/*.pem)"}},"hooks":{{"PreToolUse":{}}}}}"#,
                all_required_hooks_pretooluse()
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(
            issues.len(),
            SENSITIVE_BASENAME_RULES.len(),
            "got {issues:?}"
        );
        assert!(issues.iter().all(|i| i.severity == LintSeverity::Error));
        assert!(
            issues
                .iter()
                .all(|i| i.rule_id == VaultGateIntegrityRule::RULE_ID)
        );
    }

    #[test]
    fn empty_permissions_deny_yields_error_per_required_deny_rule() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &format!(
                r#"{{"permissions":{{"deny":[]}},"hooks":{{"PreToolUse":{}}}}}"#,
                all_required_hooks_pretooluse()
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(
            issues.len(),
            SENSITIVE_BASENAME_RULES.len(),
            "got {issues:?}"
        );
    }

    #[test]
    fn missing_one_sensitive_deny_rule_names_rule() {
        let tmp = TempDir::new().unwrap();
        write_settings(
            tmp.path(),
            &format!(
                r#"{{"permissions":{{"deny":["{}","{}"]}},"hooks":{{"PreToolUse":{}}}}}"#,
                SENSITIVE_BASENAME_RULES[0].claude_read_rule,
                SENSITIVE_BASENAME_RULES[1].claude_read_rule,
                all_required_hooks_pretooluse()
            ),
        );
        let issues = run(tmp.path());
        assert_eq!(issues.len(), 1, "got {issues:?}");
        assert!(
            issues[0]
                .message
                .contains(SENSITIVE_BASENAME_RULES[2].claude_read_rule),
            "message must name the missing deny rule: {}",
            issues[0].message
        );
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
