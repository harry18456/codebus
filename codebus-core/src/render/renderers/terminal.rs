//! Terminal renderer — `println!` to stdout. Mirrors the legacy
//! `codebus-cli/src/ui.rs` byte-equal at `--check` paths used by Phase C
//! conformance fixtures.
//!
//! When `use_color` is `false`, output stays byte-equal with fixtures —
//! no ANSI escapes, no OSC 8 hyperlinks. When `use_color` is `true`,
//! [`StreamEvent::Thought`] text picks up lightweight markdown styling
//! ([`crate::render::markdown_style::style_thought_text`]) and, if the
//! caller supplied `vault_id` + `slug_index` and the terminal advertises
//! hyperlink support via the `supports-hyperlinks` crate, each
//! resolvable `[[slug]]` is wrapped in an OSC 8 escape pointing at
//! `obsidian://open?vault=<vault_id>&file=<rel_path>`. Slugs missing
//! from the index, an absent `vault_id`/`slug_index`, an unsupported
//! terminal, or `hyperlinks: false` each independently downgrade the
//! output to "styling only" — none of them are an error.

use crate::render::event_renderer::{Banner, EventRenderer};
use crate::render::markdown_style;
use crate::stream::StreamEvent;
use crate::wiki::slug_index::SlugIndex;
use crate::wiki::types::{LintIssue, LintResult, LintSeverity};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

const INDENT: &str = "    ";

/// Renderer-specific options. Field of [`TerminalRenderer`]; not exposed via
/// the trait because other renderers (`Tauri`, `JsonLines`) don't need it.
///
/// `RenderOptions` is constructed in code, not loaded from YAML — see
/// [`RenderOptionsConfig`] for the serializable subset that round-trips
/// `~/.codebus/config.yaml`. `slug_index` carries an `Arc<SlugIndex>`
/// (built once per run) and `vault_id` is resolved from the Obsidian
/// registry, neither of which makes sense on disk.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub use_emoji: bool,
    pub use_color: bool,
    /// Effective Obsidian vault id used in the OSC 8 URI's `vault=`
    /// parameter. `None` disables hyperlink emission entirely (caller
    /// signals "Obsidian not registered for this vault").
    pub vault_id: Option<String>,
    /// Slug-to-path index built once per run (typically by
    /// [`crate::wiki::slug_index::build`]). `None` disables hyperlink
    /// emission.
    pub slug_index: Option<Arc<SlugIndex>>,
    /// Explicit hyperlink kill-switch. Default `true`. When `false`, no
    /// OSC 8 escape is emitted regardless of terminal capability
    /// detection. Useful for tests, fixture comparisons, and a future
    /// `--no-hyperlinks` opt-out.
    pub hyperlinks: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            use_emoji: false,
            use_color: false,
            vault_id: None,
            slug_index: None,
            hyperlinks: true,
        }
    }
}

/// Serializable subset of [`RenderOptions`]. This is what
/// `~/.codebus/config.yaml` carries under `render.options`; runtime
/// fields (`vault_id`, `slug_index`, `hyperlinks`) are populated by the
/// goal/query/fix flow before the renderer is constructed.
///
/// The two-struct split exists because `Arc<SlugIndex>` doesn't
/// trivially round-trip through YAML and the hyperlink kill-switch
/// belongs on the CLI surface, not the on-disk config schema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOptionsConfig {
    #[serde(default)]
    pub use_emoji: bool,
    #[serde(default)]
    pub use_color: bool,
}

impl From<RenderOptionsConfig> for RenderOptions {
    fn from(c: RenderOptionsConfig) -> Self {
        Self {
            use_emoji: c.use_emoji,
            use_color: c.use_color,
            ..Self::default()
        }
    }
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
        let line = format_event(event, &self.opts);
        if !line.is_empty() {
            println!("{line}");
        }
    }

    fn render_banner(&mut self, banner: &Banner<'_>) {
        println!("{}", format_banner(*banner, &self.opts));
    }

    fn render_lint_report(&mut self, result: &LintResult) {
        print!("{}", format_lint_report(result, &self.opts));
    }

    fn render_lint_summary(&mut self, result: &LintResult) {
        let s = format_lint_summary(result, &self.opts);
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

pub fn format_event(event: &StreamEvent, opts: &RenderOptions) -> String {
    let hyperlinks_supported =
        supports_hyperlinks::on(supports_hyperlinks::Stream::Stdout);
    format_event_inner(event, opts, hyperlinks_supported)
}

/// Test seam over [`format_event`]: callers can drive
/// `hyperlinks_supported` explicitly without depending on the live
/// `supports-hyperlinks` env detection. Public callsites go through
/// [`format_event`] which evaluates the supports check on stdout.
///
/// Public-but-doc-hidden so integration tests in `tests/` can drive the
/// supported branch deterministically across CI environments where stdout
/// may not be a TTY. Production code should use [`format_event`].
#[doc(hidden)]
pub fn format_event_inner(
    event: &StreamEvent,
    opts: &RenderOptions,
    hyperlinks_supported: bool,
) -> String {
    match event {
        StreamEvent::Thought { text } => {
            let label = format!("{} [Agent 思考]", lead("🤔", "◆", opts.use_emoji));
            let body = format_thought(text, opts, hyperlinks_supported);
            if body.contains('\n') {
                format!("{label}\n{}", indent(&body))
            } else {
                format!("{label} {body}")
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

/// Resolve thought-event text: apply markdown styling, then optionally
/// wrap each resolvable `[[slug]]` with an OSC 8 hyperlink.
///
/// Returns the styled-only string when ANY of these is false:
/// - `opts.use_color` (colors disabled → no escapes at all)
/// - `opts.hyperlinks` (explicit kill-switch)
/// - `hyperlinks_supported` (terminal capability detection)
/// - `opts.vault_id.is_some()` (Obsidian vault not registered)
/// - `opts.slug_index.is_some()` (slug index not built)
fn format_thought(text: &str, opts: &RenderOptions, hyperlinks_supported: bool) -> String {
    let styled = markdown_style::style_thought_text(text, opts.use_color);
    if !opts.use_color || !opts.hyperlinks || !hyperlinks_supported {
        return styled;
    }
    let (Some(vault_id), Some(slug_index)) = (opts.vault_id.as_deref(), opts.slug_index.as_ref())
    else {
        return styled;
    };
    wrap_wikilinks_with_osc8(&styled, vault_id, slug_index.as_ref())
}

fn styled_wikilink_re() -> &'static Regex {
    // Matches the exact byte pattern that
    // `markdown_style::style_thought_text` emits for a wikilink:
    //   ESC[36m  ESC[4m  [[<slug>]]  ESC[24m  ESC[39m
    // Capture group 1 is the slug. The regex avoids matching `]` inside
    // the slug — codebus lint already constrains slugs to a strict
    // charset, so `[^\]]` is sufficient.
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\x1b\[36m\x1b\[4m\[\[([^\]]+)\]\]\x1b\[24m\x1b\[39m")
            .expect("styled wikilink regex compiles")
    })
}

/// Find each styled `[[slug]]` in `styled` (the cyan+underline-wrapped
/// payload that `markdown_style::style_thought_text` produced) and wrap
/// it with an OSC 8 hyperlink iff the slug resolves in `slug_index`.
/// Slugs not in the index pass through unmodified, falling back to
/// styling-only output.
///
/// Short-circuits when the input contains no `[[` at all so the regex
/// engine never runs on the common case (Phase-1 thought text rarely
/// contains wikilinks).
fn wrap_wikilinks_with_osc8(styled: &str, vault_id: &str, slug_index: &SlugIndex) -> String {
    if !styled.contains("[[") {
        return styled.to_string();
    }
    styled_wikilink_re()
        .replace_all(styled, |caps: &regex::Captures<'_>| {
            let slug = &caps[1];
            match slug_index.lookup(slug) {
                Some((_loc, rel_path)) => {
                    // SlugIndex stores forward-slashed relative paths
                    // (no `.md`); display() is byte-equal to that on
                    // both Unix and Windows because we never push native
                    // separators in.
                    let uri = format!(
                        "obsidian://open?vault={vault_id}&file={}",
                        rel_path.display()
                    );
                    markdown_style::wrap_osc8(&uri, &caps[0])
                }
                None => caps[0].to_string(),
            }
        })
        .into_owned()
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

pub fn format_banner(b: Banner<'_>, opts: &RenderOptions) -> String {
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
pub fn format_lint_report(result: &LintResult, opts: &RenderOptions) -> String {
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
pub fn format_lint_summary(result: &LintResult, opts: &RenderOptions) -> String {
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
        RenderOptions::default()
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
        let out = format_lint_report(&r, &no_color());
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
        let out = format_lint_report(&r, &no_color());
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
        let out = format_lint_report(&r, &no_color());
        let expected = "# 14 pages + 2 nav files scanned, 0 error(s), 2 warning(s)\n\n! wiki/overview.md\n   warn:  page lives in wiki/ root\n! wiki/index.md\n   warn:  broken wikilink in body: [[ghost]]\n";
        assert_eq!(out, expected);
    }

    fn with_emoji() -> RenderOptions {
        RenderOptions {
            use_emoji: true,
            ..RenderOptions::default()
        }
    }

    // === Stage banners (goal-stage-banners change) ===

    #[test]
    fn sync_start_banner_renders_in_both_modes() {
        let s = format_banner(Banner::SyncStart, &no_color());
        assert!(
            s.contains("同步") || s.contains("sync"),
            "no-emoji line should mention sync, got: {s}"
        );
        let e = format_banner(Banner::SyncStart, &with_emoji());
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
        let s = format_banner(b, &no_color());
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
        let s = format_banner(b, &no_color());
        assert!(s.contains("null"), "scanner name missing: {s}");
        assert!(s.contains("1289"), "scanned count missing: {s}");
        assert!(s.contains("warn"), "action missing: {s}");

        let b2 = Banner::PiiSummary {
            scanner: "regex_basic",
            scanned: 1289,
            hits: 3,
            action: "skip",
        };
        let s2 = format_banner(b2, &no_color());
        assert!(s2.contains("regex_basic"));
        assert!(s2.contains("3"), "hits missing: {s2}");
        assert!(s2.contains("skip"));
    }

    #[test]
    fn lint_start_and_done_banners() {
        let s = format_banner(Banner::LintStart, &no_color());
        assert!(s.contains("lint"), "lint start line: {s}");

        let d = format_banner(
            Banner::LintDone {
                errors: 0,
                warns: 2,
                elapsed_ms: 312,
            },
            &no_color(),
        );
        assert!(d.contains("0"), "errors missing: {d}");
        assert!(d.contains("2"), "warns missing: {d}");
        assert!(d.contains("312"), "elapsed_ms missing: {d}");
    }

    #[test]
    fn fix_iter_start_and_done_banners() {
        let s = format_banner(Banner::FixIterStart { i: 1, max: 3 }, &no_color());
        assert!(s.contains("1"), "iter index missing: {s}");
        assert!(s.contains("3"), "max missing: {s}");

        let d = format_banner(
            Banner::FixIterDone {
                i: 1,
                fixed: 2,
                remaining: 1,
                elapsed_ms: 8123,
            },
            &no_color(),
        );
        assert!(d.contains("1"), "iter index missing: {d}");
        assert!(d.contains("2"), "fixed missing: {d}");
        assert!(d.contains("8123"), "elapsed_ms missing: {d}");
    }

    #[test]
    fn commit_done_banner_carries_sha7() {
        let b = Banner::CommitDone { sha7: "abc1234" };
        let s = format_banner(b, &no_color());
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
            &with_emoji(),
        );
        // Check emoji-version differs from no-emoji version.
        let s_no = format_banner(
            Banner::SyncDone {
                files: 0,
                mib: 0,
                elapsed_ms: 0,
            },
            &no_color(),
        );
        assert_ne!(s, s_no, "emoji vs non-emoji output should differ");
    }

    // === obsidian-clickable-wikilinks: OSC 8 hyperlink emission ===
    //
    // Tests drive the internal `format_event_inner` seam with explicit
    // `hyperlinks_supported` so they don't depend on the live
    // `supports-hyperlinks` env detection (CI may run on a TTY-less
    // dumb terminal). The public `format_event` path evaluates the
    // detection itself; the seam is `pub(crate)` and exists solely to
    // make these specs testable.

    use crate::stream::StreamEvent;
    use crate::wiki::slug_index::{SlugIndex, SlugLocation};
    use crate::wiki::types::PageType;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Cyan + underline ANSI escape pair that wraps a styled `[[slug]]`.
    /// Pinned here so spec assertions explicitly name the bytes the
    /// renderer must emit; mirrors the implementation in
    /// [`markdown_style::style_thought_text`].
    const STYLED_OPEN: &str = "\x1b[36m\x1b[4m";
    const STYLED_CLOSE: &str = "\x1b[24m\x1b[39m";
    const OSC8_OPEN: &str = "\x1b]8;;";

    fn slug_index_with(pairs: &[(&str, SlugLocation, &str)]) -> Arc<SlugIndex> {
        let mut idx = SlugIndex::default();
        for (slug, loc, path) in pairs {
            idx.insert_for_test((*slug).to_string(), loc.clone(), PathBuf::from(*path));
        }
        Arc::new(idx)
    }

    fn opts_with_hyperlinks(
        vault_id: Option<&str>,
        slug_index: Option<Arc<SlugIndex>>,
        hyperlinks: bool,
        use_color: bool,
    ) -> RenderOptions {
        RenderOptions {
            use_emoji: false,
            use_color,
            vault_id: vault_id.map(str::to_string),
            slug_index,
            hyperlinks,
        }
    }

    fn render_thought_event(text: &str, opts: &RenderOptions, supported: bool) -> String {
        let event = StreamEvent::Thought {
            text: text.to_string(),
        };
        format_event_inner(&event, opts, supported)
    }

    #[test]
    fn osc8_hyperlink_with_resolvable_slug_emits_obsidian_uri() {
        // Spec scenario: "Supported terminal with resolvable slug emits
        // OSC 8 hyperlink".
        let idx = slug_index_with(&[(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )]);
        let opts = opts_with_hyperlinks(Some("a38bcac8afd70c5e"), Some(idx), true, true);
        let out = render_thought_event("see [[buddy-cli-commands]]", &opts, true);
        assert!(
            out.contains(
                "obsidian://open?vault=a38bcac8afd70c5e&file=concepts/buddy-cli-commands"
            ),
            "expected obsidian URI in output, got: {out:?}"
        );
        assert!(
            out.contains(OSC8_OPEN),
            "expected OSC 8 escape opener, got: {out:?}"
        );
    }

    #[test]
    fn unsupported_terminal_renders_styling_only() {
        // Spec scenario: "Unsupported terminal renders styling only".
        let idx = slug_index_with(&[(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )]);
        let opts = opts_with_hyperlinks(Some("a38bcac8afd70c5e"), Some(idx), true, true);
        let out = render_thought_event("see [[buddy-cli-commands]]", &opts, false);
        assert!(
            out.contains(STYLED_OPEN) && out.contains(STYLED_CLOSE),
            "expected cyan+underline styling, got: {out:?}"
        );
        assert!(
            !out.contains(OSC8_OPEN),
            "expected NO OSC 8 escape, got: {out:?}"
        );
    }

    #[test]
    fn slug_not_in_index_falls_back_to_styling_only() {
        // Spec scenario: "Slug not in index falls back to styling only".
        let idx = slug_index_with(&[]);
        let opts = opts_with_hyperlinks(Some("a38bcac8afd70c5e"), Some(idx), true, true);
        let out = render_thought_event("see [[unknown-slug]]", &opts, true);
        assert!(
            out.contains(STYLED_OPEN) && out.contains(STYLED_CLOSE),
            "expected styling for unknown slug, got: {out:?}"
        );
        assert!(
            !out.contains(OSC8_OPEN),
            "expected NO OSC 8 for unresolvable slug, got: {out:?}"
        );
    }

    #[test]
    fn use_color_false_suppresses_both_styling_and_hyperlink() {
        // Spec scenario: "use_color false suppresses both styling and
        // hyperlink".
        let idx = slug_index_with(&[(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )]);
        let opts = opts_with_hyperlinks(Some("a38bcac8afd70c5e"), Some(idx), true, false);
        let out = render_thought_event("see [[buddy-cli-commands]]", &opts, true);
        assert!(
            out.contains("[[buddy-cli-commands]]"),
            "expected literal wikilink text, got: {out:?}"
        );
        assert!(
            !out.contains('\x1b'),
            "expected NO ANSI/OSC escapes when use_color=false, got: {out:?}"
        );
    }

    #[test]
    fn vault_id_none_disables_hyperlink_even_when_supported() {
        // Spec scenario: "vault_id None disables hyperlink even when
        // supported".
        let idx = slug_index_with(&[(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )]);
        let opts = opts_with_hyperlinks(None, Some(idx), true, true);
        let out = render_thought_event("see [[buddy-cli-commands]]", &opts, true);
        assert!(
            out.contains(STYLED_OPEN),
            "expected styling without vault, got: {out:?}"
        );
        assert!(
            !out.contains(OSC8_OPEN),
            "expected NO OSC 8 when vault_id is None, got: {out:?}"
        );
    }

    #[test]
    fn hyperlinks_false_overrides_terminal_detection() {
        // Spec scenario: "hyperlinks false overrides terminal
        // detection".
        let idx = slug_index_with(&[(
            "buddy-cli-commands",
            SlugLocation::Type(PageType::Concept),
            "concepts/buddy-cli-commands",
        )]);
        let opts = opts_with_hyperlinks(Some("a38bcac8afd70c5e"), Some(idx), false, true);
        let out = render_thought_event("see [[buddy-cli-commands]]", &opts, true);
        assert!(
            out.contains(STYLED_OPEN),
            "expected styling, got: {out:?}"
        );
        assert!(
            !out.contains(OSC8_OPEN),
            "expected NO OSC 8 when hyperlinks=false, got: {out:?}"
        );
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
        let s = format_lint_summary(&one, &no_color());
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
        let s = format_lint_summary(&many, &no_color());
        assert!(s.contains("2 warnings"));
    }
}
