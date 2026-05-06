use codebus_core::fs::file_ops::sha256_file;
use codebus_core::fs::raw_sync::sync_repo_to_raw_with_scanner;
use codebus_core::git::nested_repo::auto_commit;
use codebus_core::git::source_version::get_source_version;
use codebus_core::llm::provider::{InvokeOptions, LlmMode, LlmProvider};
use codebus_core::log::LogSink;
use codebus_core::pii::{OnHit, PiiScanner};
use codebus_core::render::EventRenderer;
use codebus_core::schema::CODEBUS_SCHEMA;
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
    // Plumbing only: per the change Non-Goal "不啟用 LogSink 寫檔", the sink
    // is accepted but not wired to receive run summaries in this change.
    // A follow-up token-tracking change will populate `RunLog` and call
    // `log_sink.write_run(&run_log)` here.
    let _ = log_sink;
    let p = vault_paths(opts.repo_root);

    if !p.root.exists() {
        run_init(opts.repo_root)?;
    }

    let mut lock = acquire_lock(&p.lock)
        .map_err(|e| io::Error::new(io::ErrorKind::AlreadyExists, e.to_string()))?;
    let mut wiki_changed = false;
    let mut lint: Option<LintResult> = None;

    let result: io::Result<()> = (async {
        sync_repo_to_raw_with_scanner(
            opts.repo_root,
            &p.raw_code,
            opts.pii_scanner,
            opts.pii_on_hit,
        )?;

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
        };

        let mut stream = opts
            .provider
            .invoke(invoke)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        while let Some(event) = stream.next().await {
            renderer.render(&event);
        }

        enrich_source_metadata(&p.wiki_page_folders, &p.raw_code, ver.commit.as_deref())?;
        flag_stale_pages(&p.wiki_page_folders, &p.raw_code)?;

        // Soft lint — never block commit. Run before any fix attempt so
        // the pre-fix issue list is observable for downstream debugging.
        let pre_fix = lint_wiki(&p.root);
        let mut final_lint = pre_fix;

        if !opts.fix_disabled {
            // Spec: "Goal flow auto-runs lint_and_fix after ingest completes".
            // Best-effort — fix-loop errors are surfaced via stderr but do
            // not block the surrounding goal flow's auto_commit.
            match lint_and_fix(&p.root, opts.provider, opts.fix_max_iterations).await {
                Ok(_report) => {}
                Err(e) => {
                    eprintln!("warning: lint fix loop errored ({e}); continuing");
                }
            }
            // Re-lint so RunGoalResult.lint reflects the user-visible
            // post-fix state.
            final_lint = lint_wiki(&p.root);
        }

        lint = Some(final_lint);

        wiki_changed = has_wiki_changes(&p.root)?;

        // Spec: "Auto-commit happens once after fix loop terminates" —
        // single commit captures both goal-ingest writes and fix-loop edits.
        let _ = auto_commit(&p.root, &format!("wiki: {}", opts.goal))
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(())
    })
    .await;

    let _ = release_lock(&mut lock);
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
    }

    impl EventRenderer for CollectingRenderer {
        fn render(&mut self, e: &StreamEvent) {
            self.events.push(e.clone());
        }
        fn render_banner(&mut self, _: &Banner<'_>) {}
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

        let mut renderer = CollectingRenderer { events: Vec::new() };
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
        let mut renderer = CollectingRenderer { events: Vec::new() };
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
        let mut renderer = CollectingRenderer { events: Vec::new() };
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
}
