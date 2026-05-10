//! Lint output formatters: text (human) + JSON (LLM agent).
//!
//! v3-lint Lint Output Formats requirement:
//! - text format: vault-relative paths grouped by file, coverage summary line.
//! - JSON format: absolute paths via `vault_root` join, single JSON object,
//!   no human prose / emoji / ANSI mixed in.

use crate::wiki::types::{LintResult, LintSeverity};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Format a lint result as plain (no-styling) text — backwards-compatible
/// API kept byte-equal with the v3-lint output. New v3-render-polish
/// callers should use [`format_text_with_opts`] to opt into emoji / color /
/// OSC 8 hyperlinks.
pub fn format_text(result: &LintResult) -> String {
    use crate::render::{RenderOptions, format_lint_text};
    use std::path::Path;
    format_lint_text(result, &RenderOptions::no_styling(), Path::new(""))
}

/// Format a lint result with caller-supplied styling. Delegates to
/// `render::lint_text::format_lint_text`; this thin wrapper exists so
/// `wiki::lint` consumers can keep importing from one place even after the
/// styling logic moved to the `render` module.
pub fn format_text_with_opts(
    result: &LintResult,
    opts: &crate::render::RenderOptions,
    wiki_root: &std::path::Path,
) -> String {
    crate::render::format_lint_text(result, opts, wiki_root)
}

/// JSON-serializable view of a lint result with absolute issue paths.
/// Spec: `vault_root`, `pages_scanned`, `nav_files_scanned`, `error_count`,
/// `warn_count`, and `issues[]` with absolute `path`.
#[derive(Debug, Serialize)]
struct JsonOutput<'a> {
    vault_root: String,
    pages_scanned: usize,
    nav_files_scanned: usize,
    error_count: usize,
    warn_count: usize,
    issues: Vec<JsonIssue<'a>>,
}

#[derive(Debug, Serialize)]
struct JsonIssue<'a> {
    path: String,
    severity: &'a LintSeverity,
    rule: &'a str,
    message: &'a str,
}

/// Format a lint result as a single JSON object. Issue paths are absolute
/// — joined with `<vault_root>/wiki/<vault-relative-path>` so agent
/// consumers can pass them straight to Read/Write/Edit tools regardless
/// of cwd.
pub fn format_json(result: &LintResult, vault_root: &Path) -> serde_json::Result<String> {
    let wiki_root = vault_root.join("wiki");
    let issues: Vec<JsonIssue<'_>> = result
        .issues
        .iter()
        .map(|i| JsonIssue {
            path: absolutize(&wiki_root, &i.path),
            severity: &i.severity,
            rule: &i.rule_id,
            message: &i.message,
        })
        .collect();

    let out = JsonOutput {
        vault_root: vault_root.to_string_lossy().into_owned(),
        pages_scanned: result.pages_scanned,
        nav_files_scanned: result.nav_files_scanned,
        error_count: result.error_count,
        warn_count: result.warn_count,
        issues,
    };
    serde_json::to_string(&out)
}

fn absolutize(wiki_root: &Path, vault_relative: &str) -> String {
    let path: PathBuf = wiki_root.join(vault_relative);
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::types::{LintIssue, LintSeverity};

    fn issue(path: &str, sev: LintSeverity, rule: &str, msg: &str) -> LintIssue {
        LintIssue {
            path: path.into(),
            severity: sev,
            rule_id: rule.into(),
            message: msg.into(),
        }
    }

    fn empty_result() -> LintResult {
        LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: Vec::new(),
            error_count: 0,
            warn_count: 0,
        }
    }

    #[test]
    fn text_format_emits_vault_relative_paths() {
        let result = LintResult {
            pages_scanned: 5,
            nav_files_scanned: 2,
            issues: vec![issue(
                "concepts/foo.md",
                LintSeverity::Error,
                "frontmatter-parse",
                "frontmatter parse failed",
            )],
            error_count: 1,
            warn_count: 0,
        };
        let text = format_text(&result);
        assert!(text.contains("wiki/concepts/foo.md"));
        // Must not contain absolute path leakage (no leading drive letter or slash before `wiki/`).
        // Text format is vault-relative only.
        assert!(!text.contains("/wiki/concepts"), "text leaked abs path: {text}");
        // v3-render-polish post-ship UX tweak: rule_id no longer leaks into
        // text format (lives only in JSON `rule` field). Human-readable
        // message + severity tag is sufficient for the terminal user; agents
        // consuming `--format json` still get the structured rule identifier.
        assert!(!text.contains("[frontmatter-parse]"));
        assert!(text.contains("error:"));
    }

    #[test]
    fn text_format_clean_result_says_no_issues() {
        let text = format_text(&empty_result());
        assert!(text.contains("no issues"));
        assert!(text.contains("0 page"));
    }

    #[test]
    fn text_format_groups_issues_by_file() {
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![
                issue("concepts/foo.md", LintSeverity::Error, "r1", "e1"),
                issue("concepts/foo.md", LintSeverity::Warn, "r2", "w1"),
            ],
            error_count: 1,
            warn_count: 1,
        };
        let text = format_text(&result);
        // Path appears once as a section header
        assert_eq!(text.matches("wiki/concepts/foo.md\n").count(), 1);
        assert!(text.contains("e1"));
        assert!(text.contains("w1"));
    }

    #[test]
    fn json_format_emits_absolute_paths() {
        let vault = Path::new("/abs/path/.codebus");
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![issue(
                "concepts/foo.md",
                LintSeverity::Error,
                "frontmatter-parse",
                "x",
            )],
            error_count: 1,
            warn_count: 0,
        };
        let json = format_json(&result, vault).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let abs = parsed["issues"][0]["path"].as_str().unwrap();
        // Path must be the joined absolute form — must contain the vault_root
        // prefix AND the vault-relative tail. Separator style is OS-dependent
        // (Windows mixes `\` from Path::join with `/` retained from input
        // strings), so normalize to `/` before comparing tails.
        let normalized = abs.replace('\\', "/");
        assert!(
            normalized.contains("wiki/concepts/foo.md"),
            "abs path missing tail: {abs}"
        );
        assert!(normalized.contains(".codebus"), "abs path missing root: {abs}");
    }

    #[test]
    fn json_format_includes_vault_root_field() {
        let vault = Path::new("/some/vault/.codebus");
        let json = format_json(&empty_result(), vault).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["vault_root"].as_str().unwrap().replace('\\', "/"),
            "/some/vault/.codebus"
        );
    }

    #[test]
    fn json_format_emits_single_valid_json_object_no_human_prose() {
        let vault = Path::new("/v/.codebus");
        let result = LintResult {
            pages_scanned: 3,
            nav_files_scanned: 1,
            issues: vec![issue("index.md", LintSeverity::Warn, "nav-missing", "log.md missing")],
            error_count: 0,
            warn_count: 1,
        };
        let json = format_json(&result, vault).unwrap();
        // Whole stdout should parse as a single JSON value — no prefix, no suffix.
        let _: serde_json::Value =
            serde_json::from_str(&json).expect("JSON output must parse cleanly");
        // No emoji, no ANSI, no leading "ok " text.
        assert!(!json.contains('✓'));
        assert!(!json.contains('\x1b'));
        assert!(json.starts_with('{'));
        assert!(json.trim_end().ends_with('}'));
    }

    #[test]
    fn json_uses_rule_field_name_per_spec() {
        let vault = Path::new("/v/.codebus");
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![issue("a.md", LintSeverity::Warn, "broken-wikilink-body", "x")],
            error_count: 0,
            warn_count: 1,
        };
        let json = format_json(&result, vault).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Per spec: JSON field name is `rule` (not `rule_id`).
        assert!(parsed["issues"][0]["rule"].is_string());
        assert_eq!(parsed["issues"][0]["rule"], "broken-wikilink-body");
    }

    #[test]
    fn coverage_line_pluralizes_correctly() {
        let one = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 1,
            ..empty_result()
        };
        let multi = LintResult {
            pages_scanned: 5,
            nav_files_scanned: 2,
            ..empty_result()
        };
        let one_text = format_text(&one);
        let multi_text = format_text(&multi);
        assert!(one_text.contains("1 page + 1 nav file"));
        assert!(multi_text.contains("5 pages + 2 nav files"));
    }
}
