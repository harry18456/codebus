//! Terminal renderer — `println!` to stdout. Mirrors the legacy
//! `codebus-cli/src/ui.rs` byte-equal at `--check` paths used by Phase C
//! conformance fixtures.
//!
//! Color formatting is intentionally deferred (`use_color` is honored by
//! reserving the field but no chalk-like escape sequences are emitted in
//! Phase 1). Tests run with `use_color: false` so captured stdout matches
//! the fixture byte-for-byte.

use crate::render::event_renderer::{Banner, EventRenderer};
use crate::stream::StreamEvent;
use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const INDENT: &str = "    ";

/// Renderer-specific options. Field of [`TerminalRenderer`]; not exposed via
/// the trait because other renderers (`Tauri`, `JsonLines`) don't need it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptions {
    #[serde(default)]
    pub use_emoji: bool,
    #[serde(default)]
    pub use_color: bool,
}

pub struct TerminalRenderer {
    opts: RenderOptions,
}

impl TerminalRenderer {
    pub fn new(opts: RenderOptions) -> Self {
        Self { opts }
    }
}

impl EventRenderer for TerminalRenderer {
    fn render(&mut self, event: &StreamEvent) {
        let line = format_event(event, self.opts);
        if !line.is_empty() {
            println!("{line}");
        }
    }

    fn render_banner(&mut self, banner: &Banner<'_>) {
        println!("{}", format_banner(*banner, self.opts));
    }

    fn render_lint_report(&mut self, result: &LintResult) {
        print!("{}", format_lint_report(result, self.opts));
    }

    fn render_lint_summary(&mut self, result: &LintResult) {
        let s = format_lint_summary(result, self.opts);
        if !s.is_empty() {
            println!("{s}");
        }
    }
}

fn lead(emoji: &'static str, symbol: &'static str, use_emoji: bool) -> &'static str {
    if use_emoji { emoji } else { symbol }
}

fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}

pub fn format_event(event: &StreamEvent, opts: RenderOptions) -> String {
    match event {
        StreamEvent::Thought { text } => {
            let label = format!("{} [Agent 思考]", lead("🤔", "◆", opts.use_emoji));
            if text.contains('\n') {
                format!("{label}\n{}", indent(text))
            } else {
                format!("{label} {text}")
            }
        }
        StreamEvent::ToolUse { name, input } => {
            if name == "Write" || name == "Edit" {
                let fp = input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .map(normalize_path)
                    .unwrap_or_else(|| "(unknown)".into());
                format!(
                    "{} [正在生成]\n{INDENT}{fp}",
                    lead("✍️ ", "+", opts.use_emoji)
                )
            } else {
                let args = format_tool_args(name, input);
                format!(
                    "{} [呼叫工具]\n{INDENT}{name}({args})",
                    lead("🛠️ ", "→", opts.use_emoji)
                )
            }
        }
        StreamEvent::ToolResult { output, is_error } => {
            if is_write_success_echo(output) {
                return String::new();
            }
            let body = if let Some(n) = read_line_count(output) {
                format!("({n} lines)")
            } else if output.len() > 200 {
                let mut t = output[..200].to_string();
                t.push('…');
                t
            } else {
                output.clone()
            };
            let _ = is_error;
            format!(
                "{} [觀察結果]\n{}",
                lead("👀", "←", opts.use_emoji),
                indent(&body)
            )
        }
        StreamEvent::Usage(_) => {
            // Token usage is consumed by run_goal / run_query for RunLog
            // accumulation; not surfaced to the terminal directly. Future
            // banner-level UX could surface a per-run cost summary if
            // desired.
            String::new()
        }
        StreamEvent::Done => String::new(),
    }
}

fn format_tool_args(name: &str, input: &serde_json::Value) -> String {
    if !input.is_object() {
        return String::new();
    }
    match name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => {
            let p = input
                .get("file_path")
                .or_else(|| input.get("notebook_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            normalize_path(p)
        }
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path_part = input
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| format!(", {}", normalize_path(p)))
                .unwrap_or_default();
            format!("{pattern}{path_part}")
        }
        _ => {
            let json = serde_json::to_string(input).unwrap_or_default();
            if json.len() > 80 {
                let mut t = json[..80].to_string();
                t.push('…');
                t
            } else {
                json
            }
        }
    }
}

fn is_write_success_echo(text: &str) -> bool {
    text.starts_with("File created successfully at:")
        || text.starts_with("File updated successfully at:")
        || text.starts_with("File edited successfully at:")
        || (text.starts_with("The file ")
            && (text.ends_with(" successfully.") || text.ends_with(" successfully")))
}

fn read_line_count(text: &str) -> Option<usize> {
    let lines: Vec<&str> = text.split('\n').collect();
    let numbered = lines
        .iter()
        .filter(|l| line_starts_with_number_and_space(l))
        .count();
    if numbered >= 3 && (numbered as f64) / (lines.len() as f64) > 0.5 {
        Some(numbered)
    } else {
        None
    }
}

fn line_starts_with_number_and_space(l: &str) -> bool {
    let bytes = l.as_bytes();
    let start = match bytes.iter().position(|b| !b.is_ascii_whitespace()) {
        Some(s) => s,
        None => return false,
    };
    let mut i = start;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i > start && i < bytes.len() && bytes[i].is_ascii_whitespace()
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("{INDENT}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_banner(b: Banner<'_>, opts: RenderOptions) -> String {
    match b {
        Banner::Start { path } => format!(
            "{} 來囉來囉~ CodeBus 駛入 {} ...",
            lead("🚌", "▶", opts.use_emoji),
            normalize_path(path)
        ),
        Banner::Goal { goal } => format!("{} 任務目標：{goal}", lead("🎯", "◎", opts.use_emoji)),
        Banner::Done { wiki_path } => format!(
            "{} 掰掰~下車囉！wiki 已生成於 {}",
            lead("🎉", "✓", opts.use_emoji),
            normalize_path(wiki_path)
        ),
        Banner::Hint { path } => format!(
            "{} 請用 Obsidian 開 {}",
            lead("💡", "i", opts.use_emoji),
            normalize_path(path)
        ),
        Banner::SyncStart => format!(
            "{} 同步 source → raw/code...",
            lead("🔄", "~", opts.use_emoji)
        ),
        Banner::SyncDone {
            files,
            mib,
            elapsed_ms,
        } => format!(
            "{} 同步完成 ({files} 檔, {mib} MiB, {elapsed_ms} ms)",
            lead("✓", "ok", opts.use_emoji)
        ),
        Banner::PiiSummary {
            scanner,
            scanned,
            hits,
            action,
        } => format!(
            "{} PII：{scanner}, scanned {scanned}, hits {hits}, action {action}",
            lead("🛡", "!", opts.use_emoji)
        ),
        Banner::LintStart => format!("{} lint 中...", lead("🔍", "~", opts.use_emoji)),
        Banner::LintDone {
            errors,
            warns,
            elapsed_ms,
        } => format!(
            "{} lint：{errors} errors, {warns} warnings ({elapsed_ms} ms)",
            lead("✓", "ok", opts.use_emoji)
        ),
        Banner::FixIterStart { i, max } => format!(
            "{} fix iter {i}/{max}...",
            lead("🔧", "~", opts.use_emoji)
        ),
        Banner::FixIterDone {
            i,
            fixed,
            remaining,
            elapsed_ms,
        } => format!(
            "{} fix iter {i}: {fixed} fixed, {remaining} remaining ({elapsed_ms} ms)",
            lead("✓", "ok", opts.use_emoji)
        ),
        Banner::CommitDone { sha7 } => {
            format!("{} commit {sha7}", lead("📌", ".", opts.use_emoji))
        }
    }
}

/// Full lint report (`--check` stdout). Output format matches TS
/// `printLintReport` byte-for-byte under `use_color = false`.
pub fn format_lint_report(result: &LintResult, opts: RenderOptions) -> String {
    let coverage = format_coverage(result);

    if result.issues.is_empty() {
        let lead_glyph = if opts.use_emoji { "✅" } else { "ok" };
        return format!("{lead_glyph} {coverage}, no issues\n");
    }

    let head = if opts.use_emoji { "🔍" } else { "#" };
    let mut out = String::new();
    out.push_str(&format!(
        "{head} {coverage}, {} error(s), {} warning(s)\n",
        result.error_count, result.warn_count
    ));
    out.push('\n');

    let mut order: Vec<String> = Vec::new();
    let mut by_path: BTreeMap<String, Vec<&LintIssue>> = BTreeMap::new();
    for i in &result.issues {
        if !by_path.contains_key(&i.path) {
            order.push(i.path.clone());
        }
        by_path.entry(i.path.clone()).or_default().push(i);
    }

    let warn_mark = if opts.use_emoji { "⚠" } else { "!" };
    let error_mark = if opts.use_emoji { "✗" } else { "x" };

    for path in &order {
        let list = by_path.get(path).unwrap();
        let has_error = list.iter().any(|i| i.severity == LintSeverity::Error);
        let mark = if has_error { error_mark } else { warn_mark };
        out.push_str(&format!("{mark} wiki/{path}\n"));
        for issue in list {
            let tag = if issue.severity == LintSeverity::Error {
                "error:"
            } else {
                "warn: "
            };
            out.push_str(&format!("   {tag} {}\n", issue.message));
        }
    }
    out
}

/// One-line summary for the goal flow's banner sequence. Empty string when
/// there are no issues.
pub fn format_lint_summary(result: &LintResult, opts: RenderOptions) -> String {
    if result.issues.is_empty() {
        return String::new();
    }
    let mark = if opts.use_emoji { "⚠" } else { "!" };
    let mut parts: Vec<String> = Vec::new();
    if result.error_count > 0 {
        let s = if result.error_count > 1 { "s" } else { "" };
        parts.push(format!("{} error{s}", result.error_count));
    }
    if result.warn_count > 0 {
        let s = if result.warn_count > 1 { "s" } else { "" };
        parts.push(format!("{} warning{s}", result.warn_count));
    }
    format!("{mark} lint: {} — codebus --check 看詳情", parts.join(", "))
}

fn format_coverage(r: &LintResult) -> String {
    let pages_s = if r.pages_scanned == 1 { "" } else { "s" };
    let nav_s = if r.nav_files_scanned == 1 { "" } else { "s" };
    format!(
        "{} page{pages_s} + {} nav file{nav_s} scanned",
        r.pages_scanned, r.nav_files_scanned
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::types::{LintIssue, LintSeverity};

    fn no_color() -> RenderOptions {
        RenderOptions {
            use_emoji: false,
            use_color: false,
        }
    }

    #[test]
    fn empty_lint_result_prints_no_issues_line() {
        let r = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![],
            error_count: 0,
            warn_count: 0,
        };
        let out = format_lint_report(&r, no_color());
        assert_eq!(out, "ok 0 pages + 0 nav files scanned, no issues\n");
    }

    #[test]
    fn coverage_pluralization() {
        let r = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 1,
            issues: vec![],
            error_count: 0,
            warn_count: 0,
        };
        let out = format_lint_report(&r, no_color());
        assert!(out.contains("1 page + 1 nav file scanned"));
    }

    #[test]
    fn issue_block_format_matches_ts() {
        let r = LintResult {
            pages_scanned: 14,
            nav_files_scanned: 2,
            issues: vec![
                LintIssue {
                    path: "overview.md".into(),
                    severity: LintSeverity::Warn,
                    message: "page lives in wiki/ root".into(),
                },
                LintIssue {
                    path: "index.md".into(),
                    severity: LintSeverity::Warn,
                    message: "broken wikilink in body: [[ghost]]".into(),
                },
            ],
            error_count: 0,
            warn_count: 2,
        };
        let out = format_lint_report(&r, no_color());
        let expected = "# 14 pages + 2 nav files scanned, 0 error(s), 2 warning(s)\n\n! wiki/overview.md\n   warn:  page lives in wiki/ root\n! wiki/index.md\n   warn:  broken wikilink in body: [[ghost]]\n";
        assert_eq!(out, expected);
    }

    fn with_emoji() -> RenderOptions {
        RenderOptions {
            use_emoji: true,
            use_color: false,
        }
    }

    // === Stage banners (goal-stage-banners change) ===

    #[test]
    fn sync_start_banner_renders_in_both_modes() {
        let s = format_banner(Banner::SyncStart, no_color());
        assert!(
            s.contains("同步") || s.contains("sync"),
            "no-emoji line should mention sync, got: {s}"
        );
        let e = format_banner(Banner::SyncStart, with_emoji());
        assert!(
            e.starts_with("🔄"),
            "emoji line should lead with 🔄, got: {e}"
        );
    }

    #[test]
    fn sync_done_banner_carries_files_mib_elapsed() {
        let b = Banner::SyncDone {
            files: 1289,
            mib: 26,
            elapsed_ms: 6234,
        };
        let s = format_banner(b, no_color());
        assert!(s.contains("1289"), "files count missing: {s}");
        assert!(s.contains("26"), "mib missing: {s}");
        assert!(s.contains("6234"), "elapsed_ms missing: {s}");
    }

    #[test]
    fn pii_summary_banner_carries_scanner_counts_and_action() {
        let b = Banner::PiiSummary {
            scanner: "null",
            scanned: 1289,
            hits: 0,
            action: "warn",
        };
        let s = format_banner(b, no_color());
        assert!(s.contains("null"), "scanner name missing: {s}");
        assert!(s.contains("1289"), "scanned count missing: {s}");
        assert!(s.contains("warn"), "action missing: {s}");

        let b2 = Banner::PiiSummary {
            scanner: "regex_basic",
            scanned: 1289,
            hits: 3,
            action: "skip",
        };
        let s2 = format_banner(b2, no_color());
        assert!(s2.contains("regex_basic"));
        assert!(s2.contains("3"), "hits missing: {s2}");
        assert!(s2.contains("skip"));
    }

    #[test]
    fn lint_start_and_done_banners() {
        let s = format_banner(Banner::LintStart, no_color());
        assert!(s.contains("lint"), "lint start line: {s}");

        let d = format_banner(
            Banner::LintDone {
                errors: 0,
                warns: 2,
                elapsed_ms: 312,
            },
            no_color(),
        );
        assert!(d.contains("0"), "errors missing: {d}");
        assert!(d.contains("2"), "warns missing: {d}");
        assert!(d.contains("312"), "elapsed_ms missing: {d}");
    }

    #[test]
    fn fix_iter_start_and_done_banners() {
        let s = format_banner(Banner::FixIterStart { i: 1, max: 3 }, no_color());
        assert!(s.contains("1"), "iter index missing: {s}");
        assert!(s.contains("3"), "max missing: {s}");

        let d = format_banner(
            Banner::FixIterDone {
                i: 1,
                fixed: 2,
                remaining: 1,
                elapsed_ms: 8123,
            },
            no_color(),
        );
        assert!(d.contains("1"), "iter index missing: {d}");
        assert!(d.contains("2"), "fixed missing: {d}");
        assert!(d.contains("8123"), "elapsed_ms missing: {d}");
    }

    #[test]
    fn commit_done_banner_carries_sha7() {
        let b = Banner::CommitDone { sha7: "abc1234" };
        let s = format_banner(b, no_color());
        assert!(s.contains("abc1234"), "sha7 missing: {s}");
    }

    #[test]
    fn stage_banners_use_emoji_glyph_when_enabled() {
        // Spec: "Stage banners follow existing emoji mode" — emoji flag flips
        // the prefix; the same glyph family as lifecycle banners.
        let s = format_banner(
            Banner::SyncDone {
                files: 0,
                mib: 0,
                elapsed_ms: 0,
            },
            with_emoji(),
        );
        // Check emoji-version differs from no-emoji version.
        let s_no = format_banner(
            Banner::SyncDone {
                files: 0,
                mib: 0,
                elapsed_ms: 0,
            },
            no_color(),
        );
        assert_ne!(s, s_no, "emoji vs non-emoji output should differ");
    }

    #[test]
    fn format_lint_summary_pluralizes() {
        let one = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![LintIssue {
                path: "x".into(),
                severity: LintSeverity::Error,
                message: "x".into(),
            }],
            error_count: 1,
            warn_count: 0,
        };
        let s = format_lint_summary(&one, no_color());
        assert!(s.contains("1 error"));
        assert!(!s.contains("1 errors"));

        let many = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![
                LintIssue {
                    path: "x".into(),
                    severity: LintSeverity::Warn,
                    message: "x".into(),
                },
                LintIssue {
                    path: "y".into(),
                    severity: LintSeverity::Warn,
                    message: "y".into(),
                },
            ],
            error_count: 0,
            warn_count: 2,
        };
        let s = format_lint_summary(&many, no_color());
        assert!(s.contains("2 warnings"));
    }
}
