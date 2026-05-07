//! Lint feedback loop — drive the LLM to fix issues that `lint_wiki`
//! reports until the vault is clean (or a hard iteration cap is reached).
//!
//! Public surface:
//!
//! - [`lint_and_fix`] — the single function shared by both `--goal`'s
//!   auto-fix step and the standalone `--fix` CLI mode.
//! - [`FixReport`] — terminal state of the loop.
//!
//! Submodules:
//!
//! - [`prompt`] — pure prompt construction (no IO).
//! - [`memory`] — `git diff wiki/` summary used as "fake memory" between
//!   iterations.
//!
//! Why a sibling of `wiki/lint/` rather than nested under it: lint is a
//! pure-read invariant, fix writes. Mixing the two would erode the
//! plugin-architecture-refactor "one invariant per module" rule.

pub mod memory;
pub mod prompt;

use crate::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
use crate::render::{Banner, EventRenderer};
use crate::wiki::lint::lint_wiki;
use crate::wiki::types::LintIssue;
use futures_util::StreamExt;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// Terminal state of one [`lint_and_fix`] run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixReport {
    /// Loop ended because the most recent `lint_wiki` returned zero issues.
    /// `iterations` counts how many fix iterations actually invoked the
    /// LLM (zero when the vault was already clean).
    Clean { iterations: u32 },
    /// Loop ended because `iterations` reached the configured
    /// `max_iterations` cap with `remaining_issues` still present.
    MaxIter {
        iterations: u32,
        remaining_issues: Vec<LintIssue>,
    },
}

/// Run the lint→fix→re-lint feedback loop against `vault_root` until the
/// vault is clean or `max_iterations` is hit. `provider` is invoked at
/// most `max_iterations` times.
///
/// `renderer` receives a [`Banner::FixIterStart`] before each LLM invoke
/// and a matching [`Banner::FixIterDone`] after the iteration's re-lint —
/// this lets the CLI's stage-banner UX expose what the loop is doing.
/// Pass a renderer that no-ops `render_banner` (e.g. `NullSink`-style)
/// when the caller doesn't care.
///
/// The function snapshots the vault's nested git HEAD before the first LLM
/// invocation. From the second iteration onward, the prompt embeds
/// `git diff <snapshot> -- wiki/` as "previous-attempt memory" so the
/// agent doesn't retry approaches it has already tried in this run.
pub async fn lint_and_fix(
    vault_root: &Path,
    provider: &dyn LlmProvider,
    max_iterations: u32,
    renderer: &mut dyn EventRenderer,
) -> io::Result<FixReport> {
    // Spec: "Skip the loop entirely when initial lint reports zero issues".
    let initial = lint_wiki(vault_root);
    if initial.issues.is_empty() {
        return Ok(FixReport::Clean { iterations: 0 });
    }

    // Snapshot HEAD once. If the vault has no commit yet (e.g. fresh
    // `--fix` against an uninitialized vault), `base_sha` is empty and
    // `git_diff_summary` will return an empty string on each call —
    // matching the "no observable previous attempt" semantics.
    let base_sha = head_sha(vault_root).unwrap_or_default();

    let mut iter: u32 = 0;
    let mut current_issues = initial.issues;

    while iter < max_iterations {
        let prior_diff: Option<String> = if iter == 0 {
            None
        } else {
            Some(memory::git_diff_summary(vault_root, &base_sha)?)
        };
        let prompt = prompt::build_fix_prompt(&current_issues, prior_diff.as_deref());

        let pre_iter_count = current_issues.len();
        let iter_one_based = iter + 1;
        renderer.render_banner(&Banner::FixIterStart {
            i: iter_one_based,
            max: max_iterations,
        });
        let iter_started_at = Instant::now();

        eprintln!(
            "lint fix iteration {}/{}: {} issues",
            iter_one_based, max_iterations, pre_iter_count
        );

        let opts = InvokeOptions {
            system_prompt: String::new(),
            user_message: prompt,
            mode: LlmMode::Ingest,
            cwd: vault_root.to_path_buf(),
            vault_root: vault_root.to_path_buf(),
            // The fix loop falls back to Claude CLI defaults for model /
            // effort. Plumbing the user-configured values through
            // `lint_and_fix` would require extending its signature; the
            // `llm-claude-cli-params` change explicitly leaves that for a
            // future follow-up so the surface here stays narrow.
            model: None,
            effort: None,
        };
        let mut stream = provider
            .invoke(opts)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        while let Some(_event) = stream.next().await {
            // Drain stream — the fix loop relies on disk diff, not stream
            // event content, for cross-iteration memory.
        }

        iter += 1;
        let after = lint_wiki(vault_root);
        let post_iter_count = after.issues.len();
        // `fixed` reports drop in issue count vs the iteration's own pre-lint
        // count. saturating_sub guards against the (unusual) case where the
        // agent introduced new issues this iteration.
        let fixed = pre_iter_count.saturating_sub(post_iter_count);
        let elapsed_ms = elapsed_ms_saturating(iter_started_at);
        renderer.render_banner(&Banner::FixIterDone {
            i: iter_one_based,
            fixed,
            remaining: post_iter_count,
            elapsed_ms,
        });

        if after.issues.is_empty() {
            return Ok(FixReport::Clean { iterations: iter });
        }
        current_issues = after.issues;
    }

    Ok(FixReport::MaxIter {
        iterations: iter,
        remaining_issues: current_issues,
    })
}

/// Cap elapsed at `u64::MAX` if the conversion overflows. Realistically a
/// goal run is under an hour so `as_millis()` fits in u64 — this guard
/// exists for the long-running pathological case (goal stuck for years
/// without timeout) so we never panic on overflow.
fn elapsed_ms_saturating(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn head_sha(vault_root: &Path) -> Option<String> {
    let out = Command::new("git")
        .current_dir(vault_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::nested_repo::{auto_commit, init_nested_repo};
    use crate::llm::provider::{EventStream, ProviderError};
    use crate::stream::StreamEvent;
    use crate::wiki::types::{LintIssue, LintSeverity};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn tmp_vault(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codebus-fixloop-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
            fs::create_dir_all(dir.join("wiki").join(f)).unwrap();
        }
        fs::write(dir.join("wiki/index.md"), "# index\n").unwrap();
        fs::write(dir.join("wiki/log.md"), "# log\n").unwrap();
        dir
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }

    fn write_page(root: &Path, rel: &str, body: &str) {
        let full = root.join("wiki").join(rel);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        let frontmatter = "title: P\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n";
        let content = format!("---\n{frontmatter}---\n{body}");
        fs::write(full, content).unwrap();
    }

    type OnInvoke = Arc<dyn Fn(&Path) + Send + Sync>;

    /// Test mock that records every invoke and runs an arbitrary side
    /// effect on the vault for each call. Side effects simulate what the
    /// real agent would do (or wouldn't, in the no-op case).
    struct RecordingMock {
        captured: Arc<Mutex<Vec<InvokeOptions>>>,
        on_invoke: OnInvoke,
    }

    impl RecordingMock {
        fn new(on_invoke: OnInvoke) -> Self {
            Self {
                captured: Arc::new(Mutex::new(Vec::new())),
                on_invoke,
            }
        }

        fn invoke_count(&self) -> usize {
            self.captured.lock().unwrap().len()
        }

        fn captured(&self) -> Vec<InvokeOptions> {
            self.captured.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for RecordingMock {
        async fn invoke(&self, opts: InvokeOptions) -> Result<EventStream, ProviderError> {
            (self.on_invoke)(&opts.cwd);
            self.captured.lock().unwrap().push(opts);
            Ok(Box::pin(futures_util::stream::iter(vec![
                StreamEvent::Done,
            ])))
        }

        fn cancel(&self) {}
    }

    /// Provider that panics if it is ever invoked. Used to assert the
    /// 0-issue short-circuit and the --check read-only contract.
    struct FailOnCallProvider;

    #[async_trait::async_trait]
    impl LlmProvider for FailOnCallProvider {
        async fn invoke(&self, _opts: InvokeOptions) -> Result<EventStream, ProviderError> {
            panic!("LlmProvider::invoke must not be called");
        }
        fn cancel(&self) {}
    }

    /// Renderer that drops every event and banner. Tests that don't care
    /// about banner emission use this to satisfy the new `lint_and_fix`
    /// signature without pulling in `TerminalRenderer`'s stdout side-effect.
    struct NullRenderer;
    impl EventRenderer for NullRenderer {
        fn render(&mut self, _: &StreamEvent) {}
        fn render_banner(&mut self, _: &Banner<'_>) {}
        fn render_lint_report(&mut self, _: &crate::wiki::types::LintResult) {}
        fn render_lint_summary(&mut self, _: &crate::wiki::types::LintResult) {}
    }

    // === Task 3.1: 0-issue short-circuit ===

    #[tokio::test]
    async fn zero_issue_vault_short_circuits_with_clean_zero_iterations() {
        // Spec: "Skip the loop entirely when initial lint reports zero issues"
        // Spec: "Clean vault produces zero LLM invocations"
        let v = tmp_vault("zero");
        // No issues: 5 type folders + index + log all present, no broken
        // links, no oversize pages.
        let provider = FailOnCallProvider;
        let report = lint_and_fix(&v, &provider, 5, &mut NullRenderer).await.unwrap();
        assert_eq!(report, FixReport::Clean { iterations: 0 });
        cleanup(&v);
    }

    // === Task 3.2: max_iter termination ===

    #[tokio::test]
    async fn max_iter_termination_when_provider_does_not_fix_anything() {
        // Spec: "Loop terminates by max iterations cap when issues remain"
        let v = tmp_vault("maxiter");
        init_nested_repo(&v).unwrap();
        // Seed a broken wikilink that the no-op mock never fixes.
        write_page(&v, "concepts/foo.md", "see [[ghost]]");
        auto_commit(&v, "seed").unwrap();

        let no_op: OnInvoke = Arc::new(|_path: &Path| {});
        let mock = RecordingMock::new(no_op);
        let report = lint_and_fix(&v, &mock, 3, &mut NullRenderer).await.unwrap();

        match report {
            FixReport::MaxIter {
                iterations,
                remaining_issues,
            } => {
                assert_eq!(iterations, 3);
                assert!(!remaining_issues.is_empty());
            }
            other => panic!("expected MaxIter, got {other:?}"),
        }
        assert_eq!(mock.invoke_count(), 3, "provider must be called 3 times");
        cleanup(&v);
    }

    // === Task 3.3: clean termination ===

    #[tokio::test]
    async fn clean_termination_when_provider_fixes_on_first_invoke() {
        // Spec: "Loop terminates with clean state when all issues are fixed"
        let v = tmp_vault("cleanterm");
        init_nested_repo(&v).unwrap();
        write_page(&v, "concepts/foo.md", "see [[ghost]]");
        auto_commit(&v, "seed").unwrap();

        // Mock removes the offending page on first invoke.
        let fix_once: OnInvoke = Arc::new(|vault: &Path| {
            let target = vault.join("wiki/concepts/foo.md");
            let _ = fs::remove_file(target);
        });
        let mock = RecordingMock::new(fix_once);
        let report = lint_and_fix(&v, &mock, 5, &mut NullRenderer).await.unwrap();

        assert_eq!(report, FixReport::Clean { iterations: 1 });
        assert_eq!(mock.invoke_count(), 1, "provider called exactly once");
        cleanup(&v);
    }

    // === Task 3.4: prior diff snapshot — base SHA stays stable across iters ===

    #[tokio::test]
    async fn prior_diff_uses_loop_start_snapshot_not_iter_to_iter() {
        // Spec: "Subsequent iterations include diff against the snapshot
        // taken at loop start" — base sha is captured once, every iter's
        // <previous_attempt> block is computed against that same base.
        let v = tmp_vault("snapshot");
        init_nested_repo(&v).unwrap();
        write_page(&v, "concepts/foo.md", "see [[ghost]]");
        auto_commit(&v, "seed").unwrap();

        // Mock that adds a unique marker file on each invoke (does NOT
        // fix the broken wikilink), so the loop runs to max_iter.
        let count = Arc::new(Mutex::new(0usize));
        let count_for_closure = count.clone();
        let add_marker: OnInvoke = Arc::new(move |vault: &Path| {
            let mut n = count_for_closure.lock().unwrap();
            *n += 1;
            let marker = vault.join(format!("wiki/concepts/marker_{}.md", *n));
            // Use raw text so it's still found in git diff regardless of
            // frontmatter parsing.
            fs::write(marker, format!("marker iteration {}\n", *n)).unwrap();
        });
        let mock = RecordingMock::new(add_marker);
        let report = lint_and_fix(&v, &mock, 3, &mut NullRenderer).await.unwrap();

        assert!(matches!(report, FixReport::MaxIter { .. }));
        let captured = mock.captured();
        assert_eq!(captured.len(), 3);

        // iter 1 (first invoke): no <previous_attempt> block.
        assert!(!captured[0].user_message.contains("<previous_attempt>"));

        // iter 2 (second invoke): diff vs base must show marker_1 (added in
        // iter 1).
        let m2 = &captured[1].user_message;
        assert!(m2.contains("<previous_attempt>"));
        assert!(
            m2.contains("marker_1.md"),
            "iter 2 prompt should diff vs loop-start base and show marker_1; got: {m2}"
        );

        // iter 3 (third invoke): diff vs base must show BOTH marker_1 AND
        // marker_2 (cumulative since loop start, NOT iter-to-iter). If we
        // were diffing iter-to-iter we'd only see marker_2 here.
        let m3 = &captured[2].user_message;
        assert!(m3.contains("<previous_attempt>"));
        assert!(
            m3.contains("marker_1.md") && m3.contains("marker_2.md"),
            "iter 3 prompt must include marker_1 AND marker_2 (proves cumulative-from-base, not iter-to-iter); got: {m3}"
        );

        cleanup(&v);
    }

    // === Task 3.5: all 7 lint rules participate in the prompt ===

    #[tokio::test]
    async fn all_seven_lint_rules_forwarded_to_llm() {
        // Spec: "All lint rules participate in the fix loop"
        // Spec: "Duplicate slug issues are forwarded to the LLM"
        // Spec: "Unexpected-file issues are forwarded to the LLM"
        let v = tmp_vault("sevenrules");
        init_nested_repo(&v).unwrap();

        // 1. broken_wikilink
        write_page(&v, "concepts/a.md", "see [[ghost]]");
        // 2. page_size — concepts threshold is 8KB; pad past it
        let big_body = "x".repeat(9000);
        write_page(&v, "concepts/big.md", &big_body);
        // 3. missing_nav — drop log.md to trigger missing-log warn
        fs::remove_file(v.join("wiki/log.md")).unwrap();
        // 4. root_page — page lives in wiki/ root
        fs::write(
            v.join("wiki/stray.md"),
            "---\ntitle: stray\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\nbody",
        )
        .unwrap();
        // 5. frontmatter_integrity — broken yaml
        fs::write(v.join("wiki/concepts/badfm.md"), "---\n: : not yaml\n---\n").unwrap();
        // 6. duplicate_slug — same `cart` slug across two type folders
        write_page(&v, "concepts/cart.md", "# c");
        write_page(&v, "entities/cart.md", "# e");
        // 7. unexpected_file — non-md file under a type folder
        fs::write(v.join("wiki/concepts/notes.txt"), "raw").unwrap();

        auto_commit(&v, "seed").unwrap();

        // No-op mock so the loop reaches max_iter and we can capture the
        // prompt.
        let no_op: OnInvoke = Arc::new(|_| {});
        let mock = RecordingMock::new(no_op);
        let _ = lint_and_fix(&v, &mock, 1, &mut NullRenderer).await.unwrap();

        let captured = mock.captured();
        assert_eq!(captured.len(), 1);
        let prompt = &captured[0].user_message;

        // Each rule should surface at least one issue path/keyword in the
        // prompt body.
        assert!(
            prompt.contains("[[ghost]]"),
            "broken_wikilink missing in prompt"
        );
        assert!(
            prompt.contains("oversize"),
            "page_size warning missing in prompt"
        );
        assert!(
            prompt.contains("log.md"),
            "missing-nav (log.md) missing in prompt"
        );
        assert!(
            prompt.contains("stray.md"),
            "root_page (stray.md) missing in prompt"
        );
        assert!(
            prompt.contains("badfm.md") || prompt.contains("frontmatter"),
            "frontmatter_integrity missing in prompt"
        );
        assert!(
            prompt.contains("duplicate slug"),
            "duplicate_slug missing in prompt"
        );
        assert!(
            prompt.contains("notes.txt") || prompt.contains("non-.md file"),
            "unexpected_file missing in prompt"
        );

        cleanup(&v);
    }

    // === Task 2.5/2.6 retained: FixReport variants ===

    #[test]
    fn clean_variant_carries_iteration_count() {
        let r = FixReport::Clean { iterations: 0 };
        assert_eq!(r, FixReport::Clean { iterations: 0 });
        assert_ne!(r, FixReport::Clean { iterations: 1 });
    }

    #[test]
    fn max_iter_variant_carries_remaining_issues() {
        let issue = LintIssue {
            path: "concepts/foo.md".into(),
            severity: LintSeverity::Warn,
            message: "still bad".into(),
        };
        let r = FixReport::MaxIter {
            iterations: 5,
            remaining_issues: vec![issue.clone()],
        };
        match r {
            FixReport::MaxIter {
                iterations,
                remaining_issues,
            } => {
                assert_eq!(iterations, 5);
                assert_eq!(remaining_issues, vec![issue]);
            }
            _ => panic!("expected MaxIter variant"),
        }
    }

    #[test]
    fn debug_format_distinguishes_variants() {
        let clean = format!("{:?}", FixReport::Clean { iterations: 3 });
        let maxiter = format!(
            "{:?}",
            FixReport::MaxIter {
                iterations: 5,
                remaining_issues: vec![],
            }
        );
        assert!(clean.contains("Clean"));
        assert!(maxiter.contains("MaxIter"));
    }
}
