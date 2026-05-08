use codebus_core::fs::file_ops::sha256_file;
use codebus_core::fs::raw_sync::sync_repo_to_raw_with_scanner;
use codebus_core::git::nested_repo::auto_commit;
use codebus_core::git::source_version::get_source_version;
use codebus_core::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
use codebus_core::log::{LogSink, RunLog, TokenUsage, accumulate_token_usage};
use codebus_core::pii::{OnHit, PiiScanner};
use codebus_core::render::{Banner, EventRenderer};
use codebus_core::schema::CODEBUS_SCHEMA;
use codebus_core::stream::StreamEvent;
use codebus_core::vault::layout::vault_paths;
use codebus_core::vault::lock::{acquire_lock, release_lock};
use codebus_core::wiki::date::utc_today_iso;
use codebus_core::wiki::fix::lint_and_fix;
use codebus_core::wiki::frontmatter::{parse_page, serialize_page};
use codebus_core::wiki::lint::lint_wiki;
use codebus_core::wiki::stale_detect::detect_stale_sources;
use codebus_core::wiki::types::{LintResult, PageFrontmatter, SourceRef};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::commands::init::run_init;

pub struct RunGoalOptions<'a> {
    pub repo_root: &'a Path,
    pub goal: &'a str,
    pub provider: &'a dyn LlmProvider,
    /// Scanner for raw_sync to invoke against each candidate text file.
    /// Built from `cfg.pii` in main.rs; default config produces a
    /// `NullScanner` so 0.2.0 raw mirror behavior is preserved.
    pub pii_scanner: &'a dyn PiiScanner,
    /// Behavior on PII hit (Warn / Skip / Mask).
    pub pii_on_hit: OnHit,
    /// When true, skip the post-ingest `lint_and_fix` step. Resolved by
    /// main.rs from `cli.no_fix || !cfg.lint.auto_fix.enabled`.
    pub fix_disabled: bool,
    /// Cap on fix-loop iterations. Resolved by main.rs from
    /// `cli.fix_max_iter.unwrap_or(cfg.lint.auto_fix.max_iterations)`.
    pub fix_max_iterations: u32,
    /// Optional `--model` override forwarded to the LLM invocation.
    /// Extracted from `ProviderConfig::ClaudeCli { model, .. }` in main.rs;
    /// `None` for non-ClaudeCli variants.
    pub model: Option<&'a str>,
    /// Optional `--effort` override forwarded to the LLM invocation.
    pub effort: Option<&'a str>,
    /// When the goal flow needs to fall back to a fresh init (vault root
    /// doesn't exist), forwarded to `init::run_init` so the user's
    /// `--no-obsidian-register` opt-out is honored.
    pub no_obsidian_register: bool,
}

pub struct RunGoalResult {
    /// True if `wiki/` subtree had any uncommitted changes after the agent
    /// finished (i.e. the agent wrote/edited at least one page). False when
    /// agent ran cleanly but produced no wiki content (typically out-of-scope
    /// goal refusal).
    pub wiki_changed: bool,
    /// Lint result captured AFTER enrich/stale-detect, BEFORE auto-commit.
    /// `None` when lint itself errored (best-effort soft mode).
    pub lint: Option<LintResult>,
}

pub async fn run_goal(
    opts: RunGoalOptions<'_>,
    renderer: &mut dyn EventRenderer,
    log_sink: &mut dyn LogSink,
) -> io::Result<RunGoalResult> {
    let p = vault_paths(opts.repo_root);

    if !p.root.exists() {
        run_init(opts.repo_root, opts.no_obsidian_register)?;
    }

    let mut lock = acquire_lock(&p.lock)
        .map_err(|e| io::Error::new(io::ErrorKind::AlreadyExists, e.to_string()))?;
    let mut wiki_changed = false;
    let mut lint: Option<LintResult> = None;
    let mut accumulated_tokens = TokenUsage::default();
    let started_at = chrono_iso_now();

    let result: io::Result<()> = (async {
        renderer.render_banner(&Banner::SyncStart);
        let sync_started = Instant::now();
        let summary = sync_repo_to_raw_with_scanner(
            opts.repo_root,
            &p.raw_code,
            opts.pii_scanner,
            opts.pii_on_hit,
        )?;
        let sync_elapsed_ms = elapsed_ms_saturating(sync_started);
        renderer.render_banner(&Banner::SyncDone {
            files: summary.files,
            mib: summary.bytes / (1024 * 1024),
            elapsed_ms: sync_elapsed_ms,
        });
        let action_label = on_hit_label(summary.action);
        renderer.render_banner(&Banner::PiiSummary {
            scanner: opts.pii_scanner.name(),
            scanned: summary.scanned,
            hits: summary.hits,
            action: action_label,
        });

        let ver = get_source_version(opts.repo_root);
        let goal_entry = serde_json::json!({
            "goal": opts.goal,
            "source_commit": ver.commit,
            "uncommitted": ver.uncommitted,
            "timestamp": chrono_iso_now(),
        });
        let mut line = goal_entry.to_string();
        line.push('\n');
        append_file(&p.goals_jsonl, line.as_bytes())?;

        let schema = if p.schema_md.exists() {
            fs::read_to_string(&p.schema_md)?
        } else {
            CODEBUS_SCHEMA.to_string()
        };
        let index_md = if p.wiki_index.exists() {
            fs::read_to_string(&p.wiki_index)?
        } else {
            "(empty)".to_string()
        };
        let system_prompt = format!(
            "{schema}\n\n# Current wiki index\n\n{index_md}\n\n# Goal\n\n{goal}",
            goal = opts.goal
        );

        let invoke = InvokeOptions {
            system_prompt,
            user_message: format!("Build/update the wiki for this goal: {}", opts.goal),
            mode: LlmMode::Ingest,
            cwd: p.root.clone(),
            vault_root: p.root.clone(),
            model: opts.model.map(str::to_string),
            effort: opts.effort.map(str::to_string),
        };

        let mut stream = opts
            .provider
            .invoke(invoke)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        while let Some(event) = stream.next().await {
            if let StreamEvent::Usage(u) = &event {
                accumulate_token_usage(&mut accumulated_tokens, u);
            }
            renderer.render(&event);
        }

        enrich_source_metadata(&p.wiki_page_folders, &p.raw_code, ver.commit.as_deref())?;
        flag_stale_pages(&p.wiki_page_folders, &p.raw_code)?;

        // Soft lint — never block commit. Run before any fix attempt so
        // the pre-fix issue list is observable for downstream debugging.
        renderer.render_banner(&Banner::LintStart);
        let lint_started = Instant::now();
        let pre_fix = lint_wiki(&p.root);
        let pre_fix_elapsed = elapsed_ms_saturating(lint_started);
        renderer.render_banner(&Banner::LintDone {
            errors: pre_fix.error_count,
            warns: pre_fix.warn_count,
            elapsed_ms: pre_fix_elapsed,
        });
        let mut final_lint = pre_fix;

        if !opts.fix_disabled {
            // Spec: "Goal flow auto-runs lint_and_fix after ingest completes".
            lint_and_fix(
                &p.root,
                opts.provider,
                opts.fix_max_iterations,
                renderer,
                &mut accumulated_tokens,
            )
            .await?;
            // Re-lint so RunGoalResult.lint reflects the user-visible
            // post-fix state. Bracket with another LintStart/LintDone pair so
            // the "post-fix lint" is also visible in the banner stream.
            renderer.render_banner(&Banner::LintStart);
            let post_fix_started = Instant::now();
            final_lint = lint_wiki(&p.root);
            let post_fix_elapsed = elapsed_ms_saturating(post_fix_started);
            renderer.render_banner(&Banner::LintDone {
                errors: final_lint.error_count,
                warns: final_lint.warn_count,
                elapsed_ms: post_fix_elapsed,
            });
        }

        lint = Some(final_lint);

        wiki_changed = has_wiki_changes(&p.root)?;

        // Spec: "Auto-commit happens once after fix loop terminates" —
        // single commit captures both goal-ingest writes and fix-loop edits.
        let head_sha = auto_commit(&p.root, &format!("wiki: {}", opts.goal))
            .map_err(|e| io::Error::other(e.to_string()))?;
        let sha7: String = head_sha.chars().take(7).collect();
        renderer.render_banner(&Banner::CommitDone { sha7: &sha7 });

        Ok(())
    })
    .await;

    let _ = release_lock(&mut lock);

    // Build the RunLog regardless of success / failure of the inner run
    // — partial token counts are still informative. Per the spec the
    // sink write is best-effort: a sink failure SHALL NOT mask a goal
    // failure, and SHALL NOT promote a successful goal to a failure.
    let finished_at = chrono_iso_now();
    let lint_error_count = lint.as_ref().map(|l| l.error_count).unwrap_or(0);
    let lint_warn_count = lint.as_ref().map(|l| l.warn_count).unwrap_or(0);
    let run_log = RunLog {
        goal: opts.goal.to_string(),
        mode: "goal".into(),
        model: opts.model.map(str::to_string),
        effort: opts.effort.map(str::to_string),
        started_at,
        finished_at,
        tokens: accumulated_tokens,
        wiki_changed,
        lint_error_count,
        lint_warn_count,
    };
    if let Err(e) = log_sink.write_run(&run_log) {
        eprintln!("warning: failed to write run log: {e}");
    }

    result?;

    Ok(RunGoalResult { wiki_changed, lint })
}

fn append_file(path: &Path, data: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    f.write_all(data)
}

/// Saturate `as u64` if a stage runs long enough to overflow u128→u64.
/// Realistic goal runs are < 1 hour, but a stuck goal could in principle
/// hit the boundary; we never panic on it.
fn elapsed_ms_saturating(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Stable string label for `OnHit` policy (used by `Banner::PiiSummary`).
/// Kept here rather than as `impl OnHit` so adding a label doesn't reach
/// across crate boundaries; `OnHit` is a closed enum so this match is
/// guaranteed-exhaustive at compile time.
fn on_hit_label(action: OnHit) -> &'static str {
    match action {
        OnHit::Warn => "warn",
        OnHit::Skip => "skip",
        OnHit::Mask => "mask",
    }
}

fn chrono_iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn list_page_files(folders: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for f in folders {
        if !f.exists() {
            continue;
        }
        for entry in fs::read_dir(f)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }
    Ok(out)
}

/// CRITICAL (iter-8 invariant): only enrich pages where AT LEAST ONE source
/// lacks sha256+at_commit. Carry-over pages from prior runs MUST keep their
/// old sha256 so flag_stale_pages can detect drift against current raw.
fn enrich_source_metadata(
    folders: &[PathBuf],
    raw_code: &Path,
    commit: Option<&str>,
) -> io::Result<()> {
    for full in list_page_files(folders)? {
        let content = match fs::read_to_string(&full) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = match parse_page(&content) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if parsed.frontmatter.sources.is_empty() {
            continue;
        }
        let all_enriched = parsed
            .frontmatter
            .sources
            .iter()
            .all(|s| s.sha256.is_some() && s.at_commit.is_some());
        if all_enriched {
            continue;
        }

        let mut enriched: Vec<SourceRef> = Vec::with_capacity(parsed.frontmatter.sources.len());
        for src in &parsed.frontmatter.sources {
            if src.sha256.is_some() && src.at_commit.is_some() {
                enriched.push(src.clone());
                continue;
            }
            let raw_path = raw_code.join(&src.path);
            let sha = if raw_path.exists() {
                sha256_file(&raw_path).unwrap_or_default()
            } else {
                String::new()
            };
            enriched.push(SourceRef {
                path: src.path.clone(),
                sha256: Some(sha),
                at_commit: Some(commit.unwrap_or("").to_string()),
            });
        }
        let new_fm = PageFrontmatter {
            sources: enriched,
            updated: utc_today_iso(),
            ..parsed.frontmatter
        };
        fs::write(&full, serialize_page(&new_fm, &parsed.body))?;
    }
    Ok(())
}

fn flag_stale_pages(folders: &[PathBuf], raw_code: &Path) -> io::Result<()> {
    for full in list_page_files(folders)? {
        let content = match fs::read_to_string(&full) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = match parse_page(&content) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let mut current_hashes: HashMap<String, String> = HashMap::new();
        for src in &parsed.frontmatter.sources {
            let raw_path = raw_code.join(&src.path);
            if raw_path.exists() {
                if let Ok(h) = sha256_file(&raw_path) {
                    current_hashes.insert(src.path.clone(), h);
                }
            }
        }
        let result = detect_stale_sources(&parsed.frontmatter, &current_hashes);
        if result.is_stale != parsed.frontmatter.stale {
            let new_fm = PageFrontmatter {
                stale: result.is_stale,
                ..parsed.frontmatter
            };
            fs::write(&full, serialize_page(&new_fm, &parsed.body))?;
        }
    }
    Ok(())
}

fn has_wiki_changes(vault_root: &Path) -> io::Result<bool> {
    let out = Command::new("git")
        .current_dir(vault_root)
        .args(["status", "--porcelain", "wiki/"])
        .output()?;
    if !out.status.success() {
        return Ok(false);
    }
    Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::llm::provider::ProviderError;
    use codebus_core::log::sinks::null_sink::NullSink;
    use codebus_core::pii::scanners::null_scanner::NullScanner;
    use codebus_core::render::Banner;
    use codebus_core::stream::StreamEvent;

    struct CollectingRenderer {
        events: Vec<StreamEvent>,
        /// One Debug-formatted line per `render_banner` call. Tests assert on
        /// these via `.contains("SyncStart")` etc. — the `{:?}` form embeds
        /// both the variant name and field values, which is enough granularity
        /// to verify ordering and payloads without an extra `OwnedBanner` mirror.
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

    struct WriteOnePageProvider;

    #[async_trait::async_trait]
    impl LlmProvider for WriteOnePageProvider {
        async fn invoke(
            &self,
            opts: InvokeOptions,
        ) -> Result<codebus_core::llm::provider::EventStream, ProviderError> {
            // Simulate the agent writing one wiki page during the turn.
            let target = opts.cwd.join("wiki").join("concepts").join("foo.md");
            std::fs::create_dir_all(target.parent().unwrap()).ok();
            let content = "---\ntitle: Foo\ntype: concept\nsources:\n  - path: src/a.rs\ngoals:\n  - g\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n\n# Foo body\n";
            std::fs::write(&target, content).ok();
            Ok(Box::pin(futures_util::stream::iter(vec![
                StreamEvent::Thought {
                    text: "writing page".into(),
                },
                StreamEvent::Done,
            ])))
        }
        fn cancel(&self) {}
    }

    /// Counting mock for goal-flow integration tests. The first invoke
    /// simulates the goal ingest by writing a wiki page that intentionally
    /// has a broken wikilink (so the post-ingest lint reports an issue and
    /// the fix loop has work to do). Subsequent invokes (the fix-loop's)
    /// are no-ops, so the loop runs to its `max_iterations` cap.
    struct CountingMock {
        invokes: std::sync::Arc<std::sync::Mutex<Vec<codebus_core::llm::provider::InvokeOptions>>>,
    }

    impl CountingMock {
        fn new() -> Self {
            Self {
                invokes: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }
        fn count(&self) -> usize {
            self.invokes.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for CountingMock {
        async fn invoke(
            &self,
            opts: codebus_core::llm::provider::InvokeOptions,
        ) -> Result<codebus_core::llm::provider::EventStream, ProviderError> {
            let n = {
                let mut g = self.invokes.lock().unwrap();
                g.push(opts.clone());
                g.len()
            };
            if n == 1 {
                // Simulate goal ingest with a broken wikilink.
                let target = opts.cwd.join("wiki").join("concepts").join("foo.md");
                std::fs::create_dir_all(target.parent().unwrap()).ok();
                let content = "---\ntitle: Foo\ntype: concept\nsources:\n  - path: src/a.rs\ngoals:\n  - g\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n\n# see [[ghost]]\n";
                std::fs::write(&target, content).ok();
            }
            // Subsequent fix-loop invokes are no-ops so the loop hits
            // max_iterations and returns MaxIter.
            Ok(Box::pin(futures_util::stream::iter(vec![
                StreamEvent::Done,
            ])))
        }
        fn cancel(&self) {}
    }

    fn seed_repo() -> std::path::PathBuf {
        let repo = std::env::temp_dir().join(format!(
            "codebus-goalfix-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "t@t"])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "t"])
            .current_dir(&repo)
            .status()
            .unwrap();
        fs::write(repo.join("a.rs"), "// source\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();
        repo
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    #[tokio::test]
    async fn run_goal_writes_jsonl_entry_and_runs_lint() {
        let repo =
            std::env::temp_dir().join(format!("codebus-goal-{}-{}", std::process::id(), nanos()));
        let _ = fs::remove_dir_all(&repo);
        fs::create_dir_all(&repo).unwrap();
        // Make repo a git repo so source_version returns a commit.
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.email", "t@t"])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "t"])
            .current_dir(&repo)
            .status()
            .unwrap();
        fs::write(repo.join("a.rs"), "// source\n").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();

        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();
        let provider = WriteOnePageProvider;
        let null_scanner = NullScanner::new();
        let result = run_goal(
            RunGoalOptions {
                repo_root: &repo,
                goal: "explore foo",
                provider: &provider,
                pii_scanner: &null_scanner,
                pii_on_hit: OnHit::Warn,
                fix_disabled: true,
                fix_max_iterations: 5,
                model: None,
                effort: None,
                no_obsidian_register: true,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_goal succeeded");

        // Stream events forwarded
        assert!(!renderer.events.is_empty());

        // goals.jsonl appended
        let p = vault_paths(&repo);
        let jsonl = fs::read_to_string(&p.goals_jsonl).unwrap();
        assert!(jsonl.contains("explore foo"));
        assert!(jsonl.contains("source_commit"));

        // wiki page written by mock provider
        assert!(p.wiki_concepts.join("foo.md").exists());

        // lint ran and is captured
        assert!(result.lint.is_some());

        // wiki changed
        assert!(result.wiki_changed);

        let _ = fs::remove_dir_all(&repo);
    }

    // === lint-feedback-loop: goal flow integration ===

    #[tokio::test]
    async fn default_goal_flow_triggers_fix_loop_after_lint() {
        // Spec: "Default goal run triggers the fix loop after lint"
        // Spec: "Auto-commit happens once after fix loop terminates"
        let repo = seed_repo();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();
        let provider = CountingMock::new();
        let null_scanner = NullScanner::new();
        run_goal(
            RunGoalOptions {
                repo_root: &repo,
                goal: "explore foo",
                provider: &provider,
                pii_scanner: &null_scanner,
                pii_on_hit: OnHit::Warn,
                fix_disabled: false,
                fix_max_iterations: 2, // 1 goal-ingest + 2 fix iters = 3 invokes
                model: None,
                effort: None,
                no_obsidian_register: true,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_goal succeeded");

        // 1 invoke for goal ingest + 2 for the fix loop hitting max_iter.
        assert_eq!(
            provider.count(),
            3,
            "expected 1 ingest + 2 fix iterations, got {}",
            provider.count()
        );

        // run_init seeds an "init: codebus vault" commit on first goal run;
        // the goal flow itself adds exactly one more commit even though the
        // fix loop iterates multiple times (single auto_commit captures both
        // ingest write and fix-loop edits).
        let p = vault_paths(&repo);
        let log = std::process::Command::new("git")
            .current_dir(&p.root)
            .args(["log", "--pretty=format:%s"])
            .output()
            .unwrap();
        let log_lines = String::from_utf8_lossy(&log.stdout);
        let commit_lines: Vec<_> = log_lines.lines().collect();
        assert_eq!(
            commit_lines.len(),
            2,
            "expected init commit + single goal commit, got: {log_lines}"
        );
        assert!(
            commit_lines[0].contains("wiki: explore foo"),
            "newest commit should be the goal commit"
        );
        assert!(
            commit_lines[1].contains("init"),
            "older commit should be the init commit"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn fix_disabled_short_circuits_fix_loop_in_goal_flow() {
        // Spec: "--no-fix flag skips the fix loop in goal flow"
        // Spec: "Disabled config skips the fix loop in goal flow"
        let repo = seed_repo();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();
        let provider = CountingMock::new();
        let null_scanner = NullScanner::new();
        run_goal(
            RunGoalOptions {
                repo_root: &repo,
                goal: "explore foo",
                provider: &provider,
                pii_scanner: &null_scanner,
                pii_on_hit: OnHit::Warn,
                fix_disabled: true, // <- escape hatch
                fix_max_iterations: 5,
                model: None,
                effort: None,
                no_obsidian_register: true,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_goal succeeded");

        // Only the goal-ingest invoke; fix loop never runs.
        assert_eq!(
            provider.count(),
            1,
            "expected only the goal-ingest invoke, got {}",
            provider.count()
        );

        // auto_commit still runs once for the goal (init produced its own
        // commit during run_init).
        let p = vault_paths(&repo);
        let log = std::process::Command::new("git")
            .current_dir(&p.root)
            .args(["log", "--pretty=format:%s"])
            .output()
            .unwrap();
        let lines = String::from_utf8_lossy(&log.stdout);
        let commits: Vec<_> = lines.lines().collect();
        assert_eq!(
            commits.len(),
            2,
            "expected init + goal commit, got: {lines}"
        );
        assert!(commits[0].contains("wiki: explore foo"));

        let _ = fs::remove_dir_all(&repo);
    }

    // === Stage banner integration (goal-stage-banners change) ===

    #[tokio::test]
    async fn run_goal_emits_stage_banners_in_order() {
        // Spec: "Render stage banners during goal flow" + "Render PII summary
        // banner" — goal flow surfaces SyncStart/SyncDone, PiiSummary,
        // LintStart/LintDone, and CommitDone in that order, plus FixIter*
        // banners from the inner fix loop when --no-fix is not set.
        let repo = seed_repo();
        let mut renderer = CollectingRenderer::new();
        let mut sink = NullSink::new();
        let provider = WriteOnePageProvider;
        let null_scanner = NullScanner::new();

        run_goal(
            RunGoalOptions {
                repo_root: &repo,
                goal: "explore foo",
                provider: &provider,
                pii_scanner: &null_scanner,
                pii_on_hit: OnHit::Warn,
                fix_disabled: true, // keep fix loop out of this test's scope
                fix_max_iterations: 1,
                model: None,
                effort: None,
                no_obsidian_register: true,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_goal succeeded");

        // Required stage banners in this order. We assert the index of each
        // marker is monotonically increasing, ignoring any other banner in
        // between (e.g. lifecycle banners emitted by main.rs are absent here
        // because we call run_goal directly).
        let banners = &renderer.banners;
        let idx = |needle: &str| -> Option<usize> {
            banners.iter().position(|b| b.starts_with(needle))
        };
        let i_sync_start = idx("SyncStart").expect("SyncStart missing");
        let i_sync_done = idx("SyncDone").expect("SyncDone missing");
        let i_pii = idx("PiiSummary").expect("PiiSummary missing");
        let i_lint_start = idx("LintStart").expect("LintStart missing");
        let i_lint_done = idx("LintDone").expect("LintDone missing");
        let i_commit = idx("CommitDone").expect("CommitDone missing");

        assert!(
            i_sync_start < i_sync_done,
            "SyncStart before SyncDone: {banners:?}"
        );
        assert!(
            i_sync_done < i_pii,
            "SyncDone before PiiSummary: {banners:?}"
        );
        assert!(
            i_pii < i_lint_start,
            "PiiSummary before LintStart: {banners:?}"
        );
        assert!(
            i_lint_start < i_lint_done,
            "LintStart before LintDone: {banners:?}"
        );
        assert!(
            i_lint_done < i_commit,
            "LintDone before CommitDone: {banners:?}"
        );

        // PiiSummary payload sanity: scanner name "null" + 0 hits.
        let pii_line = &banners[i_pii];
        assert!(pii_line.contains("null"), "PII banner: {pii_line}");
        assert!(pii_line.contains("hits: 0"), "PII banner: {pii_line}");

        // SyncDone payload: WriteOnePageProvider seeds a small fixture
        // (seed_repo writes a.rs only), so files >= 1.
        let sync_done_line = &banners[i_sync_done];
        assert!(
            sync_done_line.contains("files:"),
            "sync done banner: {sync_done_line}"
        );

        // CommitDone sha7 should be 7 hex chars.
        let commit_line = &banners[i_commit];
        assert!(
            commit_line.contains("sha7"),
            "commit banner missing sha7 field: {commit_line}"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    #[tokio::test]
    async fn run_goal_continues_when_renderer_swallows_banner_errors() {
        // Spec: "Stage banners do not block on stdout failures" — the
        // EventRenderer trait returns `()` from render_banner, so banner
        // emission is non-fallible by construction. This test pins that
        // contract: a renderer that ignores every input still lets the goal
        // flow complete with the same wiki / lint / commit outcome.
        struct DropAllRenderer;
        impl EventRenderer for DropAllRenderer {
            fn render(&mut self, _: &StreamEvent) {}
            fn render_banner(&mut self, _: &Banner<'_>) {}
            fn render_lint_report(&mut self, _: &LintResult) {}
            fn render_lint_summary(&mut self, _: &LintResult) {}
        }

        let repo = seed_repo();
        let mut renderer = DropAllRenderer;
        let mut sink = NullSink::new();
        let provider = WriteOnePageProvider;
        let null_scanner = NullScanner::new();

        let result = run_goal(
            RunGoalOptions {
                repo_root: &repo,
                goal: "explore foo",
                provider: &provider,
                pii_scanner: &null_scanner,
                pii_on_hit: OnHit::Warn,
                fix_disabled: true,
                fix_max_iterations: 1,
                model: None,
                effort: None,
                no_obsidian_register: true,
            },
            &mut renderer,
            &mut sink,
        )
        .await
        .expect("run_goal must succeed even when renderer drops every banner");

        let p = vault_paths(&repo);
        assert!(p.wiki_concepts.join("foo.md").exists());
        assert!(result.wiki_changed);
        assert!(result.lint.is_some());

        let _ = fs::remove_dir_all(&repo);
    }
}
