//! Lint text-format renderer with emoji ↔ ASCII fallback, ANSI color, and
//! OSC 8 hyperlink wrapping per the `Lint Output Formats` requirement of
//! the `lint-feedback-loop` capability.
//!
//! Output shape mirrors the legacy plain-text format (vault-relative paths,
//! per-file grouping, coverage summary line) and adds three styling layers
//! controlled by [`RenderOptions`]:
//!
//! 1. **Emoji header / lead glyphs** — `✅`/`ok` for clean, `🔍`/`#` for
//!    issue header, `✗`/`x` for files-with-errors, `⚠`/`!` for files-with-
//!    only-warnings. ASCII forms are byte-equal to the legacy `format_text`
//!    output; this preserves the v3-lint Lint Output Formats text contract
//!    when emoji are off (non-TTY default).
//! 2. **ANSI color on severity tags** — `error:` wrapped in red (`\x1b[31m`)
//!    and `warn: ` wrapped in yellow (`\x1b[33m`). Body text and rule id
//!    stay uncolored. Suppressed when `use_color` is false.
//! 3. **OSC 8 hyperlink on file path lead** — `wiki/<rel-path>` becomes a
//!    clickable link to `obsidian://open?vault=<id>&file=<rel>` when
//!    `use_hyperlinks` is true AND `vault_id` is `Some`. Both `vault_id`
//!    and `<rel>` are percent-encoded for URL safety.
//!
//! JSON format is NOT rendered here — see `wiki::lint::output::format_json`
//! which intentionally keeps machine-readable output free of every styling
//! layer above (per the `JSON format suppresses human text and styling
//! escapes` scenario).

use crate::render::options::RenderOptions;
use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use std::collections::BTreeMap;
use std::path::Path;

const ANSI_RED: &str = "\x1b[31m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_RESET: &str = "\x1b[0m";

const OSC8_OPEN: &str = "\x1b]8;;";
const OSC8_DELIM: &str = "\x1b\\";

/// Format a lint result as styled text per `RenderOptions`. `wiki_root` is
/// the absolute path to `<vault>/wiki/` and is used only for OSC 8 URL
/// composition (the `file=` query parameter is the issue path relative to
/// `wiki_root`, which equals `LintIssue::path` since lint already stores
/// vault-relative paths). Pass an empty `Path::new("")` when emitting in a
/// context that has no wiki context — the OSC 8 wrap is suppressed when
/// `vault_id` is None regardless.
pub fn format_lint_text(result: &LintResult, opts: &RenderOptions, wiki_root: &Path) -> String {
    let _ = wiki_root; // currently encoded inside LintIssue.path; reserved for future use
    let coverage = format_coverage(result);

    if result.issues.is_empty() {
        let lead_glyph = if opts.use_emoji { "✅" } else { "ok" };
        return format!("{lead_glyph} {coverage}, no issues\n");
    }

    let head = if opts.use_emoji { "🔍" } else { "#" };
    let mut out = String::new();
    out.push_str(&format!(
        "{head} {coverage}, {} error(s), {} warning(s)\n\n",
        result.error_count, result.warn_count
    ));

    let mut by_path: BTreeMap<&str, Vec<&LintIssue>> = BTreeMap::new();
    for issue in &result.issues {
        by_path.entry(issue.path.as_str()).or_default().push(issue);
    }

    let error_lead = if opts.use_emoji { "✗" } else { "x" };
    let warn_lead = if opts.use_emoji { "⚠" } else { "!" };

    for (path, issues) in by_path {
        let has_error = issues.iter().any(|i| i.severity == LintSeverity::Error);
        let lead = if has_error { error_lead } else { warn_lead };
        // Wiki-subtree issues render with a `wiki/` prefix and may be wrapped
        // in an Obsidian OSC 8 hyperlink. Vault-internal issues (e.g. the
        // `.claude/settings.json` gate finding) render verbatim and are never
        // hyperlinked — they are not vault pages.
        let path_line = if crate::wiki::lint::is_wiki_relative_path(path) {
            let label = format!("wiki/{path}");
            wrap_osc8(&label, path, opts)
        } else {
            path.to_string()
        };
        out.push_str(&format!("{lead} {path_line}\n"));
        for issue in issues {
            let raw_tag = match issue.severity {
                LintSeverity::Error => "error:",
                LintSeverity::Warn => "warn: ",
            };
            let tag = if opts.use_color {
                let color = match issue.severity {
                    LintSeverity::Error => ANSI_RED,
                    LintSeverity::Warn => ANSI_YELLOW,
                };
                format!("{color}{raw_tag}{ANSI_RESET}")
            } else {
                raw_tag.to_string()
            };
            out.push_str(&format!("   {tag} {}\n", issue.message));
        }
    }

    out
}

/// Wrap `label` as a clickable OSC 8 hyperlink targeting Obsidian's vault
/// opener with `rel_path` (relative to the wiki root). Falls back to plain
/// `label` when `use_hyperlinks` is false OR `vault_id` is None — both are
/// progressive-enhancement signals; missing either degrades cleanly to the
/// non-hyperlink form.
fn wrap_osc8(label: &str, rel_path: &str, opts: &RenderOptions) -> String {
    if !opts.use_hyperlinks {
        return label.to_string();
    }
    let Some(vault_id) = opts.vault_id.as_deref() else {
        return label.to_string();
    };
    let url = format!(
        "obsidian://open?vault={}&file={}",
        percent_encode(vault_id),
        percent_encode(rel_path),
    );
    format!("{OSC8_OPEN}{url}{OSC8_DELIM}{label}{OSC8_OPEN}{OSC8_DELIM}")
}

/// Minimal RFC 3986 unreserved-set percent encoder. Encodes everything
/// except `[A-Za-z0-9-._~]` and the path separator `/` (which is legal
/// inside the `file=` parameter and would otherwise fragment the URL into
/// noise). Spaces become `%20`, non-ASCII bytes become `%XX` per UTF-8
/// byte sequence.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char);
            }
            other => {
                use std::fmt::Write as _;
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}

fn format_coverage(result: &LintResult) -> String {
    let pages = result.pages_scanned;
    let navs = result.nav_files_scanned;
    let p_plural = if pages == 1 { "" } else { "s" };
    let n_plural = if navs == 1 { "" } else { "s" };
    format!("{pages} page{p_plural} + {navs} nav file{n_plural} scanned")
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

    fn no_styling() -> RenderOptions {
        RenderOptions::no_styling()
    }

    fn emoji_only() -> RenderOptions {
        RenderOptions::explicit(true, false, false, None)
    }

    fn emoji_color() -> RenderOptions {
        RenderOptions::explicit(true, true, false, None)
    }

    fn emoji_color_hyperlinks(vault_id: &str) -> RenderOptions {
        RenderOptions::explicit(true, true, true, Some(vault_id.to_string()))
    }

    /// Spec scenario: "Text format ASCII fallback when emoji disabled"
    #[test]
    fn clean_emoji_off_uses_ok_lead() {
        let s = format_lint_text(&empty_result(), &no_styling(), Path::new(""));
        assert!(s.starts_with("ok "), "got: {s:?}");
        assert!(s.contains("no issues"));
    }

    /// Spec scenario: "Text format emoji header on clean vault when emoji enabled"
    #[test]
    fn clean_emoji_on_uses_check_emoji() {
        let s = format_lint_text(&empty_result(), &emoji_only(), Path::new(""));
        assert!(s.starts_with("✅ "), "got: {s:?}");
    }

    fn one_error_result() -> LintResult {
        LintResult {
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
        }
    }

    /// Spec scenario: "Text format ANSI red error tag when color enabled"
    #[test]
    fn error_with_color_wraps_red() {
        let s = format_lint_text(&one_error_result(), &emoji_color(), Path::new(""));
        assert!(s.contains("\x1b[31merror:\x1b[0m"), "got: {s:?}");
    }

    /// Spec scenario: "Text format suppresses ANSI when NO_COLOR set"
    #[test]
    fn no_color_omits_ansi() {
        let s = format_lint_text(&one_error_result(), &emoji_only(), Path::new(""));
        assert!(!s.contains("\x1b["), "ANSI escape leaked: {s:?}");
        assert!(s.contains("error:"));
    }

    fn one_warn_result() -> LintResult {
        LintResult {
            pages_scanned: 3,
            nav_files_scanned: 1,
            issues: vec![issue(
                "index.md",
                LintSeverity::Warn,
                "nav-missing",
                "log.md missing",
            )],
            error_count: 0,
            warn_count: 1,
        }
    }

    /// Spec scenario: "Text format ANSI yellow warn tag when color enabled"
    #[test]
    fn warn_with_color_wraps_yellow() {
        let s = format_lint_text(&one_warn_result(), &emoji_color(), Path::new(""));
        assert!(s.contains("\x1b[33mwarn: \x1b[0m"), "got: {s:?}");
    }

    /// Spec scenario: "Text format wraps wiki path in OSC 8 hyperlink when vault id present"
    #[test]
    fn osc8_wraps_path_when_vault_id_some() {
        let s = format_lint_text(
            &one_error_result(),
            &emoji_color_hyperlinks("abcdef"),
            Path::new(""),
        );
        let expected = "\x1b]8;;obsidian://open?vault=abcdef&file=concepts/foo.md\x1b\\wiki/concepts/foo.md\x1b]8;;\x1b\\";
        assert!(s.contains(expected), "OSC 8 wrap missing; got: {s:?}");
    }

    /// Spec scenario: "Text format omits OSC 8 when vault id absent"
    #[test]
    fn osc8_omitted_when_vault_id_none() {
        // hyperlinks=true but vault_id=None → fall back to plain text.
        let opts = RenderOptions::explicit(true, true, true, None);
        let s = format_lint_text(&one_error_result(), &opts, Path::new(""));
        assert!(s.contains("wiki/concepts/foo.md"));
        assert!(!s.contains("\x1b]8;"), "OSC 8 leaked: {s:?}");
    }

    /// hyperlinks=false short-circuits even with vault_id set.
    #[test]
    fn osc8_omitted_when_use_hyperlinks_false() {
        let opts = RenderOptions::explicit(true, true, false, Some("vid".into()));
        let s = format_lint_text(&one_error_result(), &opts, Path::new(""));
        assert!(!s.contains("\x1b]8;"));
    }

    /// Spec scenario: "Text format URL-encodes vault id and file path"
    #[test]
    fn url_encodes_spaces_in_vault_id_and_path() {
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![issue(
                "processes/auth flow.md",
                LintSeverity::Error,
                "frontmatter-parse",
                "x",
            )],
            error_count: 1,
            warn_count: 0,
        };
        let s = format_lint_text(&result, &emoji_color_hyperlinks("my vault"), Path::new(""));
        assert!(
            s.contains("vault=my%20vault&file=processes/auth%20flow.md"),
            "encoded URL missing; got: {s:?}"
        );
    }

    /// Per-file grouping: same path appears once as a header.
    #[test]
    fn groups_issues_by_file() {
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
        let s = format_lint_text(&result, &no_styling(), Path::new(""));
        // Path appears once as a section header.
        assert_eq!(s.matches("wiki/concepts/foo.md\n").count(), 1);
        assert!(s.contains("e1"));
        assert!(s.contains("w1"));
    }

    /// Files with only warnings get the warn-lead glyph.
    #[test]
    fn warn_only_file_uses_warn_lead() {
        let s = format_lint_text(&one_warn_result(), &emoji_only(), Path::new(""));
        assert!(s.contains("⚠ wiki/index.md"), "got: {s:?}");
    }

    /// Files with errors get the error-lead glyph (ASCII fallback).
    #[test]
    fn error_file_ascii_fallback_uses_x() {
        let s = format_lint_text(&one_error_result(), &no_styling(), Path::new(""));
        assert!(s.contains("x wiki/concepts/foo.md"), "got: {s:?}");
    }

    /// Coverage line pluralization (1 vs N).
    #[test]
    fn coverage_pluralizes() {
        let one = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 1,
            ..empty_result()
        };
        let s = format_lint_text(&one, &no_styling(), Path::new(""));
        assert!(s.contains("1 page + 1 nav file"));
    }

    /// agent-run-integrity task 2.5 — the vault-gate-integrity issue path
    /// `.claude/settings.json` is NOT a wiki page; text format must render it
    /// VERBATIM (no `wiki/` prefix that wiki-subtree issues get).
    #[test]
    fn gate_issue_path_rendered_verbatim_without_wiki_prefix() {
        let result = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![issue(
                ".claude/settings.json",
                LintSeverity::Error,
                "vault-gate-integrity",
                "missing Bash gate",
            )],
            error_count: 1,
            warn_count: 0,
        };
        let s = format_lint_text(&result, &no_styling(), Path::new(""));
        // Verbatim path, no wiki/ prefix.
        assert!(
            s.contains(".claude/settings.json"),
            "gate path must appear verbatim: {s:?}"
        );
        assert!(
            !s.contains("wiki/.claude/settings.json"),
            "gate path must NOT be prefixed with wiki/: {s:?}"
        );
    }

    /// A non-wiki issue path is never wrapped in an Obsidian OSC 8 hyperlink
    /// (it is not a vault page) even when hyperlinks + vault id are present.
    #[test]
    fn gate_issue_path_not_hyperlinked() {
        let result = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![issue(
                ".claude/settings.json",
                LintSeverity::Error,
                "vault-gate-integrity",
                "missing Bash gate",
            )],
            error_count: 1,
            warn_count: 0,
        };
        let s = format_lint_text(&result, &emoji_color_hyperlinks("vid"), Path::new(""));
        assert!(
            !s.contains("\x1b]8;"),
            "non-wiki gate path must not be OSC 8 hyperlinked: {s:?}"
        );
        assert!(s.contains(".claude/settings.json"));
    }

    /// percent_encode round-trip basics.
    #[test]
    fn percent_encode_keeps_unreserved_and_slash() {
        let s = percent_encode("abc/123-._~XYZ");
        assert_eq!(s, "abc/123-._~XYZ");
    }

    #[test]
    fn percent_encode_replaces_space_and_meta() {
        let s = percent_encode("a b&c=d");
        assert_eq!(s, "a%20b%26c%3Dd");
    }
}
