//! Standalone `--fix` mode: run the lint feedback loop against an
//! existing vault. No raw_sync, no goal-style ingest, no source enrichment.
//!
//! The mode targets two scenarios:
//!
//! 1. A wiki built by an earlier `--goal` run that drifted (lint shows
//!    issues that auto-fix didn't catch, or auto-fix was disabled).
//! 2. A hand-written Obsidian vault placed under `<repo>/.codebus/wiki/`
//!    that the user wants codebus to tidy up.
//!
//! `--fix` requires an existing vault — there's no init-on-demand here,
//! because doing so would blur the line between init / fix / goal.

use codebus_core::git::nested_repo::auto_commit;
use codebus_core::llm::provider::LlmProvider;
use codebus_core::log::{LogSink, RunLog, TokenUsage};
use codebus_core::render::EventRenderer;
use codebus_core::vault::layout::vault_paths;
use codebus_core::vault::lock::{acquire_lock, release_lock};
use codebus_core::wiki::fix::lint_and_fix;
use codebus_core::wiki::lint::lint_wiki;
use codebus_core::wiki::types::LintResult;
use std::io;
use std::path::Path;

pub struct RunFixOptions<'a> {
    pub repo_root: &'a Path,
    pub provider: &'a dyn LlmProvider,
    pub fix_max_iterations: u32,
}

#[derive(Debug)]
pub struct RunFixResult {
    /// Pre-fix lint result captured for renderer use; may be empty if the
    /// vault was already clean.
    pub pre_lint: LintResult,
    /// Post-fix lint result for renderer summary at run end.
    pub post_lint: LintResult,
}

pub async fn run_fix(
    opts: RunFixOptions<'_>,
    renderer: &mut dyn EventRenderer,
    log_sink: &mut dyn LogSink,
) -> io::Result<RunFixResult> {
    let p = vault_paths(opts.repo_root);

    if !p.root.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "No codebus vault at {} — run `codebus --repo {}` first to init, or `codebus --repo {} --goal \"...\"` to ingest",
                p.root.display(),
                opts.repo_root.display(),
                opts.repo_root.display()
            ),
        ));
    }

    let mut lock = acquire_lock(&p.lock)
        .map_err(|e| io::Error::new(io::ErrorKind::AlreadyExists, e.to_string()))?;

    let started_at = chrono::Utc::now().to_rfc3339();
    let mut accumulated_tokens = TokenUsage::default();

    let result: io::Result<RunFixResult> = (async {
        let pre_lint = lint_wiki(&p.root);
        lint_and_fix(
            &p.root,
            opts.provider,
            opts.fix_max_iterations,
            renderer,
            &mut accumulated_tokens,
        )
        .await?;
        let post_lint = lint_wiki(&p.root);

        // Spec: "--fix mode commits its results to the nested vault git
        // repo" — distinct commit message so users can spot fix-loop runs
        // in `git log`.
        let _ = auto_commit(&p.root, "wiki: lint fix loop")
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(RunFixResult {
            pre_lint,
            post_lint,
        })
    })
    .await;

    let _ = release_lock(&mut lock);

    // Build a RunLog with mode="fix" so jsonl analysis can distinguish
    // standalone fix-loop runs from goal/query. This isn't covered by
    // the token-tracking spec (which scopes runs to goal+query) but
    // omitting fix-mode logging would silently lose token data the user
    // expects to see, so we record it under a third mode value.
    let post_lint_for_log = result.as_ref().map(|r| &r.post_lint);
    let finished_at = chrono::Utc::now().to_rfc3339();
    let run_log = RunLog {
        goal: "wiki: lint fix loop".into(),
        mode: "fix".into(),
        model: None,
        effort: None,
        started_at,
        finished_at,
        tokens: accumulated_tokens,
        wiki_changed: result.is_ok(),
        lint_error_count: post_lint_for_log.map(|l| l.error_count).unwrap_or(0),
        lint_warn_count: post_lint_for_log.map(|l| l.warn_count).unwrap_or(0),
    };
    if let Err(e) = log_sink.write_run(&run_log) {
        eprintln!("warning: failed to write run log: {e}");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::git::nested_repo::init_nested_repo;
    use codebus_core::llm::provider::{EventStream, InvokeOptions, LlmProvider, ProviderError};
    use codebus_core::log::sinks::null_sink::NullSink;
    use codebus_core::render::Banner;
    use codebus_core::stream::StreamEvent;
    use codebus_core::wiki::types::LintResult;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    struct CollectingRenderer {
        events: Vec<StreamEvent>,
        /// Same Debug-string capture pattern as goal.rs's CollectingRenderer
        /// (one entry per `render_banner` call). Tests assert ordering and
        /// payload via `.contains("FixIterStart")` etc.
        banners: Vec<String>,
    }

    impl CollectingRenderer {
        fn new() -> Self {
            Self {
                events: Vec::new(),
                banners: Vec::new(),
            }
        }
    }

    impl EventRenderer for CollectingRenderer {
        fn render(&mut self, e: &StreamEvent) {
            self.events.push(e.clone());
        }
        fn render_banner(&mut self, b: &Banner<'_>) {
            self.banners.push(format!("{b:?}"));
        }
        fn render_lint_report(&mut self, _: &LintResult) {}
        fn render_lint_summary(&mut self, _: &LintResult) {}
    }

    /// Mock that records each invoke and asserts the user_message looks
    /// like a fix-loop prompt (not goal-style with schema + index + goal).
    struct FixModeMock {
        invokes: Arc<Mutex<Vec<InvokeOptions>>>,
    }

    impl FixModeMock {
        fn new() -> Self {
            Self {
                invokes: Arc::new(Mutex::new(Vec::new())),
            }
        }
        fn count(&self) -> usize {
            self.invokes.lock().unwrap().len()
        }
        fn captured(&self) -> Vec<InvokeOptions> {
            self.invokes.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for FixModeMock {
        async fn invoke(&self, opts: InvokeOptions) -> Result<EventStream, ProviderError> {
            self.invokes.lock().unwrap().push(opts);
            Ok(Box::pin(futures_util::stream::iter(vec![
                StreamEvent::Done,
            ])))
        }
        fn cancel(&self) {}
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn tmp_repo(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-runfix-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn seed_vault(repo: &Path) {
        let codebus = repo.join(".codebus");
        for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
            fs::create_dir_all(codebus.join("wiki").join(f)).unwrap();
        }
        fs::write(codebus.join("wiki/index.md"), "# index\n").unwrap();
        fs::write(codebus.join("wiki/log.md"), "# log\n").unwrap();
        // A page with a broken wikilink so lint reports an issue.
        fs::write(
            codebus.join("wiki/concepts/foo.md"),
            "---\ntitle: Foo\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\nsee [[ghost]]\n",
        )
        .unwrap();
        init_nested_repo(&codebus).unwrap();
        codebus_core::git::nested_repo::auto_commit(&codebus, "seed").unwrap();
    }

    #[tokio::test]
    async fn run_fix_invokes_lint_and_fix_and_auto_commits() {
        // Spec: "--fix mode commits its results to the nested vault git
        //        repo"
        let repo = tmp_repo("happy");
        seed_vault(&repo);

        let provider = FixModeMock::new();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();

        let result = run_fix(
            RunFixOptions {
                repo_root: &repo,
                provider: &provider,
                fix_max_iterations: 2,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_fix should succeed when vault exists");

        // Loop runs to max_iterations (mock fixes nothing).
        assert_eq!(provider.count(), 2);
        assert!(!result.pre_lint.issues.is_empty());

        // Latest commit message is the fix-loop marker.
        let codebus = repo.join(".codebus");
        let log = std::process::Command::new("git")
            .current_dir(&codebus)
            .args(["log", "--pretty=format:%s", "-1"])
            .output()
            .unwrap();
        let msg = String::from_utf8_lossy(&log.stdout);
        assert_eq!(msg.trim(), "wiki: lint fix loop");

        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn run_fix_errors_when_vault_missing() {
        // Spec: "--fix mode requires an existing vault"
        let repo = tmp_repo("novault");

        struct FailMock;
        #[async_trait::async_trait]
        impl LlmProvider for FailMock {
            async fn invoke(&self, _: InvokeOptions) -> Result<EventStream, ProviderError> {
                panic!("must not invoke when vault is absent");
            }
            fn cancel(&self) {}
        }

        let provider = FailMock;
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();

        let err = run_fix(
            RunFixOptions {
                repo_root: &repo,
                provider: &provider,
                fix_max_iterations: 5,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect_err("missing vault must error");

        let msg = err.to_string();
        assert!(
            msg.contains("codebus --repo")
                || msg.contains("codebus init")
                || msg.contains("--goal"),
            "error message must guide the user; got: {msg}"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn run_fix_does_not_run_ingest() {
        // Spec: "--fix mode skips ingest"
        // - No goal-style prompt (system_prompt should be empty for
        //   fix-loop invokes).
        // - raw_sync must not run (sentinel file under raw/ stays put).
        let repo = tmp_repo("noingest");
        seed_vault(&repo);
        let codebus = repo.join(".codebus");
        // Place a sentinel under raw/code that sync_repo_to_raw would wipe.
        fs::create_dir_all(codebus.join("raw/code")).unwrap();
        let sentinel = codebus.join("raw/code/SENTINEL.txt");
        fs::write(&sentinel, "do not delete").unwrap();

        let provider = FixModeMock::new();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();

        run_fix(
            RunFixOptions {
                repo_root: &repo,
                provider: &provider,
                fix_max_iterations: 1,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_fix succeeded");

        assert!(
            sentinel.exists(),
            "raw/ sentinel must survive — run_fix must not invoke raw_sync"
        );

        let captured = provider.captured();
        assert!(!captured.is_empty(), "fix loop should have run");
        // Fix-loop invocations carry an empty system_prompt (the prompt
        // body lives in user_message). Goal-style invocations stuff the
        // schema + index + goal into system_prompt.
        for opts in &captured {
            assert!(
                opts.system_prompt.is_empty(),
                "fix-loop invokes must have empty system_prompt; got: {:?}",
                opts.system_prompt
            );
            assert!(
                !opts.user_message.contains("# Goal"),
                "fix-loop user_message must not look like a goal prompt"
            );
        }

        let _ = fs::remove_dir_all(&repo);
    }

    // === Stage banner integration (goal-stage-banners change) ===

    #[tokio::test]
    async fn run_fix_emits_fix_iter_start_and_done_banners_per_iteration() {
        // Spec: "Fix iteration start banner" + "Fix iteration done banner"
        // — for each iteration of the fix loop, run_fix must surface a
        // FixIterStart before the LLM invoke and a FixIterDone after.
        // Mock fixes nothing → loop runs to max_iterations cap.
        let repo = tmp_repo("fixiterbanners");
        seed_vault(&repo);

        let provider = FixModeMock::new();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();

        let max_iters = 3u32;
        run_fix(
            RunFixOptions {
                repo_root: &repo,
                provider: &provider,
                fix_max_iterations: max_iters,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_fix succeeded");

        // Provider invoked once per iteration.
        assert_eq!(provider.count(), max_iters as usize);

        // Filter to FixIter banners and verify exact count + ordering.
        let fix_iter_starts: Vec<&String> = renderer
            .banners
            .iter()
            .filter(|b| b.starts_with("FixIterStart"))
            .collect();
        let fix_iter_dones: Vec<&String> = renderer
            .banners
            .iter()
            .filter(|b| b.starts_with("FixIterDone"))
            .collect();
        assert_eq!(
            fix_iter_starts.len(),
            max_iters as usize,
            "expected {max_iters} FixIterStart banners, got {}: {:?}",
            fix_iter_starts.len(),
            renderer.banners
        );
        assert_eq!(
            fix_iter_dones.len(),
            max_iters as usize,
            "expected {max_iters} FixIterDone banners, got {}: {:?}",
            fix_iter_dones.len(),
            renderer.banners
        );

        // Banner ordering within the captured list must alternate Start/Done
        // (we don't assert payload values in detail — that's covered by the
        // format_banner unit tests).
        let interleave: Vec<&String> = renderer
            .banners
            .iter()
            .filter(|b| b.starts_with("FixIterStart") || b.starts_with("FixIterDone"))
            .collect();
        for (i, line) in interleave.iter().enumerate() {
            if i % 2 == 0 {
                assert!(
                    line.starts_with("FixIterStart"),
                    "expected FixIterStart at idx {i}, got: {line}"
                );
            } else {
                assert!(
                    line.starts_with("FixIterDone"),
                    "expected FixIterDone at idx {i}, got: {line}"
                );
            }
        }

        // Each FixIterStart carries `i` (1-based) and `max`.
        // Verify the first start has i: 1, max: 3 by looking at the Debug string.
        assert!(
            fix_iter_starts[0].contains("i: 1"),
            "first iter index should be 1: {}",
            fix_iter_starts[0]
        );
        assert!(
            fix_iter_starts[0].contains("max: 3"),
            "max should be 3: {}",
            fix_iter_starts[0]
        );

        let _ = fs::remove_dir_all(&repo);
    }
}
