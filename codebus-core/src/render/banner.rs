//! Run-lifecycle banners with codebus brand identity (the bus / boarding
//! metaphor). Per the `Banner Output for Verb Commands` requirement of the
//! `cli` capability, these are the structural milestones a verb invocation
//! emits at lifecycle transitions.
//!
//! Design: plain enum + free functions, no trait. v3 has exactly one
//! terminal target — the discuss session resolved that an `EventRenderer`
//! trait + factory (v2's pattern) is single-impl speculative abstraction
//! and stays out per `feedback_dont_speculative_abstract`.
//!
//! Each variant maps to one stdout line. The emoji-leading form is used
//! when [`RenderOptions::use_emoji`] is true; otherwise an ASCII symbol
//! fallback is used so non-TTY consumers (pipes, log files) get clean
//! grep-friendly output.

use crate::render::options::RenderOptions;
use std::path::Path;

/// All run-lifecycle banner variants. Borrows `'a` to avoid String
/// allocation when callers pass through references they already hold.
#[derive(Debug)]
pub enum Banner<'a> {
    /// Verb command invocation announcement. Emitted first.
    Start { repo_path: &'a Path },
    /// Goal text echo (only for `codebus goal`).
    Goal { goal_text: &'a str },
    /// Raw mirror sync starting.
    SyncStart,
    /// Raw mirror sync done. `mib` is `bytes / 1024 / 1024` rounded to 1 decimal.
    SyncDone {
        files: usize,
        mib: f64,
        elapsed_ms: u64,
    },
    /// PII scan summary line. `action` is one of `"warn"` / `"skip"` / `"mask"`.
    PiiSummary {
        scanner: &'a str,
        scanned: usize,
        hits: usize,
        action: &'a str,
    },
    /// Lint phase starting.
    LintStart,
    /// Lint phase done.
    LintDone {
        errors: usize,
        warns: usize,
        elapsed_ms: u64,
    },
    /// Vault auto-commit produced a non-empty commit. `sha7` is the 7-char short SHA.
    CommitDone { sha7: &'a str },
    /// Verb completion. Emitted last on the success path.
    Done { wiki_path: &'a Path },
    /// Optional Obsidian usage hint. Emitted after `Done` when Obsidian is
    /// installed and the vault is registered.
    Hint { wiki_path: &'a Path },
}

/// Choose between `emoji` and `symbol` based on `opts.use_emoji`. Inline
/// helper so each variant arm reads as `lead("🚌", "▶", opts)`.
fn lead(emoji: &'static str, symbol: &'static str, opts: &RenderOptions) -> &'static str {
    if opts.use_emoji { emoji } else { symbol }
}

/// Cross-platform path normalization for display: forward-slashes only.
/// Banner output is human-readable; display path style SHALL be uniform
/// across Windows / Unix even when `Path` holds backslashes.
fn norm(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Format a banner to its single-line string representation. Pure function
/// for test assertion; production code typically goes through [`print_banner`].
pub fn format_banner(banner: Banner<'_>, opts: &RenderOptions) -> String {
    match banner {
        Banner::Start { repo_path } => format!(
            "{} 來囉來囉~ CodeBus 駛入 {}...",
            lead("🚌", "▶", opts),
            norm(repo_path)
        ),
        Banner::Goal { goal_text } => format!("{} 任務目標：{goal_text}", lead("🎯", "◎", opts),),
        Banner::SyncStart => format!("{} 同步 source → raw/code...", lead("🔄", "~", opts),),
        Banner::SyncDone {
            files,
            mib,
            elapsed_ms,
        } => format!(
            "{} 同步完成 ({files} 檔, {mib:.1} MiB, {elapsed_ms} ms)",
            lead("✓", "ok", opts),
        ),
        Banner::PiiSummary {
            scanner,
            scanned,
            hits,
            action,
        } => format!(
            "{} PII：{scanner}, scanned {scanned}, hits {hits}, action {action}",
            lead("🛡", "!", opts),
        ),
        Banner::LintStart => format!("{} lint 中...", lead("🔍", "~", opts),),
        Banner::LintDone {
            errors,
            warns,
            elapsed_ms,
        } => format!(
            "{} lint：{errors} errors, {warns} warnings ({elapsed_ms} ms)",
            lead("✓", "ok", opts),
        ),
        Banner::CommitDone { sha7 } => format!("{} commit {sha7}", lead("📌", ".", opts),),
        Banner::Done { wiki_path } => format!(
            "{} 掰掰~下車囉！wiki 已生成於 {}",
            lead("🎉", "✓", opts),
            norm(wiki_path)
        ),
        Banner::Hint { wiki_path } => format!(
            "{} 請用 Obsidian 開 {}",
            lead("💡", "i", opts),
            norm(wiki_path)
        ),
    }
}

/// Print a banner to stdout. Convenience wrapper over [`format_banner`].
pub fn print_banner(banner: Banner<'_>, opts: &RenderOptions) {
    println!("{}", format_banner(banner, opts));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn emoji_on() -> RenderOptions {
        RenderOptions::explicit(true, false, false, None)
    }

    fn emoji_off() -> RenderOptions {
        RenderOptions::no_styling()
    }

    /// Spec scenario: "Start banner appears at verb invocation"
    #[test]
    fn format_start_emoji_on() {
        let p = PathBuf::from("/tmp/repo");
        let s = format_banner(Banner::Start { repo_path: &p }, &emoji_on());
        assert!(s.starts_with("🚌"), "got: {s:?}");
        assert!(s.contains("/tmp/repo"));
        assert!(s.contains("駛入"));
    }

    #[test]
    fn format_start_emoji_off() {
        let p = PathBuf::from("/tmp/repo");
        let s = format_banner(Banner::Start { repo_path: &p }, &emoji_off());
        assert!(s.starts_with("▶"), "got: {s:?}");
        assert!(s.contains("/tmp/repo"));
    }

    #[test]
    fn format_goal_emoji_on() {
        let s = format_banner(
            Banner::Goal {
                goal_text: "describe auth",
            },
            &emoji_on(),
        );
        assert!(s.starts_with("🎯"));
        assert!(s.contains("describe auth"));
    }

    #[test]
    fn format_goal_emoji_off() {
        let s = format_banner(
            Banner::Goal {
                goal_text: "describe auth",
            },
            &emoji_off(),
        );
        assert!(s.starts_with("◎"));
    }

    /// Spec scenario: "SyncDone banner reports counts and elapsed time"
    #[test]
    fn format_sync_done_includes_counts_and_ms() {
        let s = format_banner(
            Banner::SyncDone {
                files: 12,
                mib: 0.5,
                elapsed_ms: 230,
            },
            &emoji_on(),
        );
        assert!(s.starts_with("✓"));
        assert!(s.contains("12 檔"));
        assert!(s.contains("0.5 MiB"));
        assert!(s.contains("230 ms"));
    }

    #[test]
    fn format_sync_done_emoji_off_uses_ok_lead() {
        let s = format_banner(
            Banner::SyncDone {
                files: 1,
                mib: 0.0,
                elapsed_ms: 5,
            },
            &emoji_off(),
        );
        assert!(s.starts_with("ok"), "got: {s:?}");
    }

    #[test]
    fn format_pii_summary_includes_all_fields() {
        let s = format_banner(
            Banner::PiiSummary {
                scanner: "regex_basic",
                scanned: 12,
                hits: 3,
                action: "mask",
            },
            &emoji_on(),
        );
        assert!(s.starts_with("🛡"));
        assert!(s.contains("regex_basic"));
        assert!(s.contains("scanned 12"));
        assert!(s.contains("hits 3"));
        assert!(s.contains("action mask"));
    }

    #[test]
    fn format_lint_start_emoji_off_uses_tilde() {
        let s = format_banner(Banner::LintStart, &emoji_off());
        assert!(s.starts_with("~"), "got: {s:?}");
    }

    #[test]
    fn format_lint_done_includes_counts() {
        let s = format_banner(
            Banner::LintDone {
                errors: 1,
                warns: 2,
                elapsed_ms: 45,
            },
            &emoji_on(),
        );
        assert!(s.contains("1 errors"));
        assert!(s.contains("2 warnings"));
        assert!(s.contains("45 ms"));
    }

    /// Spec scenario: "CommitDone banner reports short SHA"
    #[test]
    fn format_commit_done_includes_sha7() {
        let s = format_banner(Banner::CommitDone { sha7: "abc1234" }, &emoji_on());
        assert!(s.starts_with("📌"));
        assert!(s.contains("abc1234"));
    }

    /// Spec scenario: "Done banner appears at successful completion"
    #[test]
    fn format_done_emoji_on() {
        let p = PathBuf::from("/v/.codebus/wiki");
        let s = format_banner(Banner::Done { wiki_path: &p }, &emoji_on());
        assert!(s.starts_with("🎉"));
        assert!(s.contains("掰掰"));
        assert!(s.contains("/v/.codebus/wiki"));
    }

    #[test]
    fn format_done_emoji_off_uses_check() {
        let p = PathBuf::from("/v/.codebus/wiki");
        let s = format_banner(Banner::Done { wiki_path: &p }, &emoji_off());
        assert!(s.starts_with("✓"), "got: {s:?}");
    }

    #[test]
    fn format_hint_includes_wiki_path() {
        let p = PathBuf::from("/v/.codebus/wiki");
        let s = format_banner(Banner::Hint { wiki_path: &p }, &emoji_on());
        assert!(s.starts_with("💡"));
        assert!(s.contains("/v/.codebus/wiki"));
    }

    /// Path normalization: backslashes → forward slashes for display.
    #[test]
    fn format_normalizes_windows_paths_to_forward_slash() {
        let p = PathBuf::from(r"C:\Users\harry\repo");
        let s = format_banner(Banner::Start { repo_path: &p }, &emoji_on());
        assert!(s.contains("C:/Users/harry/repo"), "got: {s:?}");
        assert!(!s.contains(r"\Users"), "backslash leaked: {s:?}");
    }

    /// Each variant produces exactly one line (no embedded newline).
    #[test]
    fn each_variant_renders_single_line() {
        let p = PathBuf::from("/x");
        let opts = emoji_on();
        let lines = [
            format_banner(Banner::Start { repo_path: &p }, &opts),
            format_banner(Banner::Goal { goal_text: "g" }, &opts),
            format_banner(Banner::SyncStart, &opts),
            format_banner(
                Banner::SyncDone {
                    files: 1,
                    mib: 0.0,
                    elapsed_ms: 1,
                },
                &opts,
            ),
            format_banner(
                Banner::PiiSummary {
                    scanner: "s",
                    scanned: 0,
                    hits: 0,
                    action: "warn",
                },
                &opts,
            ),
            format_banner(Banner::LintStart, &opts),
            format_banner(
                Banner::LintDone {
                    errors: 0,
                    warns: 0,
                    elapsed_ms: 1,
                },
                &opts,
            ),
            format_banner(Banner::CommitDone { sha7: "abc1234" }, &opts),
            format_banner(Banner::Done { wiki_path: &p }, &opts),
            format_banner(Banner::Hint { wiki_path: &p }, &opts),
        ];
        for line in &lines {
            assert!(!line.contains('\n'), "embedded newline in: {line:?}");
        }
    }
}
