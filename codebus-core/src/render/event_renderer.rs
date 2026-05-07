//! [`EventRenderer`] trait + shared output types.
//!
//! Sync trait by design (decision §"Trait sync/async"): all renderers are
//! either `println!` (terminal), `tokio::sync::mpsc::Sender::try_send`
//! (Tauri webview emit, fire-and-forget), or `BufWriter::write_all`
//! (JsonLines) — none need `await`.
//!
//! Adding a new renderer: implement this trait + register a variant in
//! [`super::factory::RendererKind`].

use crate::stream::StreamEvent;
use crate::wiki::types::LintResult;

/// One of the structural CLI banners. Lifecycle banners (start/goal/done/hint)
/// frame the run; stage banners (sync/pii/lint/fix/commit) surface progress
/// through the otherwise-silent goal pipeline. Strings carried as `&'a str`
/// to avoid intermediate allocation; the renderer is responsible for any
/// required normalization (e.g. backslash → forward-slash).
#[derive(Debug, Clone, Copy)]
pub enum Banner<'a> {
    /// "CodeBus 駛入 <path>" — startup line printed by every command.
    Start { path: &'a str },
    /// "任務目標：<goal>" — goal command echo.
    Goal { goal: &'a str },
    /// "下車囉！wiki 已生成於 <wiki_path>" — successful goal completion.
    Done { wiki_path: &'a str },
    /// "請用 Obsidian 開 <path>" — post-goal next-step hint.
    Hint { path: &'a str },
    /// Sync stage opening banner — emitted before raw_sync starts.
    SyncStart,
    /// Sync stage closing banner — emitted after raw_sync returns.
    SyncDone {
        files: usize,
        mib: u64,
        elapsed_ms: u64,
    },
    /// PII summary banner — emitted once after sync, regardless of scanner.
    PiiSummary {
        scanner: &'a str,
        scanned: usize,
        hits: usize,
        action: &'a str,
    },
    /// Lint stage opening banner.
    LintStart,
    /// Lint stage closing banner.
    LintDone {
        errors: usize,
        warns: usize,
        elapsed_ms: u64,
    },
    /// Fix-loop iteration opening banner. `i` is 1-based.
    FixIterStart { i: u32, max: u32 },
    /// Fix-loop iteration closing banner. `fixed` is the drop in issue count
    /// versus the iteration's pre-lint snapshot; `remaining` is post-lint.
    FixIterDone {
        i: u32,
        fixed: usize,
        remaining: usize,
        elapsed_ms: u64,
    },
    /// Auto-commit closing banner with the short (7-char) HEAD sha.
    CommitDone { sha7: &'a str },
}

/// Object-safe renderer trait. Each method side-effects on the underlying
/// sink (stdout, webview channel, file). Methods take `&mut self` because
/// renderers may buffer output (terminal does not, but file/network ones
/// likely will).
pub trait EventRenderer: Send + Sync {
    /// Render one stream event. May be a no-op (e.g. terminal renderer
    /// silently drops `StreamEvent::Done` and tool-result echoes).
    fn render(&mut self, event: &StreamEvent);

    /// Render a structural banner.
    fn render_banner(&mut self, banner: &Banner<'_>);

    /// Render the full lint report (used by `--check`).
    fn render_lint_report(&mut self, result: &LintResult);

    /// Render the one-line lint summary used by the goal flow's done
    /// banner — nothing if the report is clean.
    fn render_lint_summary(&mut self, result: &LintResult);

    /// Flush any buffered output. Default no-op; renderers with buffered
    /// I/O override this.
    fn flush(&mut self) {}
}
