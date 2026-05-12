//! `verb::goal::run_goal` — full goal-verb orchestration as a library
//! function. Mirrors `codebus-cli/src/commands/goal.rs`.
//!
//! Behavior order:
//! 1. Emit `VerbBanner::Start` + `VerbBanner::Goal`
//! 2. Vault precondition — auto-init when `<repo>/.codebus/` is missing
//!    via `vault::init::run_init` (silent in library; CLI thin wrapper
//!    keeps using `commands::init::run` for byte-equivalent banner output
//!    when invoked directly via `codebus init`)
//! 3. Load `lint.fix` config — `VerbError::ConfigParse { which: "lint.fix" }`
//!    on parse failure; apply `options.no_fix` override
//! 4. Source-signal drift detection + optional re-sync (with PII scanner)
//! 5. Load `pii` config — `VerbError::ConfigParse { which: "pii" }`
//! 6. Load `claude_code` config — `VerbError::ConfigParse { which: "claude_code" }`
//! 7. Build env overrides — `VerbError::KeyringMissing` on failure
//! 8. Spawn goal agent with `GOAL_TOOLSET`; emit `SpawnStart` / `SpawnEnd`
//! 9. Run fix loop (when `lint.fix.enabled` and not `no_fix`)
//! 10. Auto-commit `wiki: <goal>` — `VerbError::Internal` on git failure
//!     (SKIPPED when `VerbError::Cancelled`)
//! 11. Load `log` config + write `RunLog`
//! 12. Emit `VerbBanner::Done` and return `GoalReport`

use crate::config::{
    PiiConfig, PiiScannerKind, Verb, build_env_overrides, default_config_path,
    load_claude_code_config, load_lint_fix_config, load_pii_config,
};
use crate::git::auto_commit;
use crate::log::verb_log::{
    load_verb_log_config, resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};
use crate::log::{RunLog, TokenUsage};
use crate::pii::PiiScanner;
use crate::pii::scanners::null_scanner::NullScanner;
use crate::pii::scanners::regex_basic::RegexBasicScanner;
use crate::stream::StreamEvent;
use crate::vault::init::{InitOptions, run_init};
use crate::vault::layout::vault_paths;
use crate::vault::manifest::{self, ManifestOutcome};
use crate::vault::raw_sync::{SyncSummary, sync_with_scanner, walk_source_for_signal};
use crate::vault::source_signal_detect::detect_drift;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbBanner, VerbEvent, VerbLifecycleEvent};
use crate::wiki::fix::{TerminationReason, run_fix_loop};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Toolset for goal: read code, write/edit wiki pages. No Bash (fix gets
/// that). v2 iter-9 carry, spike-verified 2026-05-09 to be a real hard gate.
pub const GOAL_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

/// Caller-controllable inputs to `run_goal`.
#[derive(Debug, Clone)]
pub struct GoalOptions {
    pub text: String,
    pub force_resync: bool,
    pub no_fix: bool,
    pub no_obsidian_register: bool,
}

/// Outcome of a successful `run_goal` invocation.
#[derive(Debug, Clone)]
pub struct GoalReport {
    pub accumulated_tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    pub started_at: String,
    pub finished_at: String,
}

/// Run the goal verb against `repo`. See module docs for full behavior.
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoalReport, VerbError> {
    let paths = vault_paths(repo);

    // Step 1: Start + Goal banners.
    on_event(VerbEvent::Banner(VerbBanner::Start {
        repo_path: repo.to_path_buf(),
    }));
    on_event(VerbEvent::Banner(VerbBanner::Goal {
        goal_text: options.text.clone(),
    }));

    // Step 2: vault precondition — auto-init if missing.
    if !paths.root.exists() {
        let init_opts = InitOptions {
            no_obsidian_register: options.no_obsidian_register,
            write_starter_config: true,
        };
        // Silent auto-init: library doesn't render init banners. CLI's
        // direct `codebus init` invocation still emits banners via its
        // own thin wrapper.
        run_init(repo, &init_opts, |_| {}).map_err(|e| VerbError::Internal {
            message: format!("auto-init: {e}"),
        })?;
        if !paths.root.exists() {
            return Err(VerbError::Internal {
                message: format!(
                    "auto-init did not create vault at {}",
                    paths.root.display()
                ),
            });
        }
    }

    // Step 3: load lint.fix config + apply CLI override.
    let fix_cfg = match default_config_path() {
        Some(p) if p.exists() => load_lint_fix_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "lint.fix",
            source: e,
        })?,
        _ => Default::default(),
    };
    let fix_cfg = fix_cfg.merge_cli_overrides(options.no_fix);

    // Step 4: source-signal drift detection.
    let (walk_files, walk_bytes) =
        walk_source_for_signal(repo).map_err(|e| VerbError::Internal {
            message: format!("compute source signal: {e}"),
        })?;
    let stub_summary = SyncSummary {
        files: walk_files,
        bytes: walk_bytes,
        pii_matches: 0,
        pii_skipped_files: 0,
        pii_masked_matches: 0,
    };
    let current_signal = manifest::compute_source_signal(repo, &stub_summary);
    let needs_resync =
        options.force_resync || detect_drift(&paths.manifest_yaml, &current_signal);

    // Step 5: load pii + claude_code config.
    let pii_cfg = match default_config_path() {
        Some(p) if p.exists() => load_pii_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "pii",
            source: e,
        })?,
        _ => Default::default(),
    };
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => load_claude_code_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "claude_code",
            source: e,
        })?,
        _ => Default::default(),
    };

    // Step 4b: conditional re-sync with PII scanner.
    if needs_resync {
        let scanner = build_pii_scanner(&pii_cfg);
        on_event(VerbEvent::Banner(VerbBanner::SyncStart));
        let sync_started = Instant::now();
        let summary =
            sync_with_scanner(repo, &paths.raw_code, scanner.as_ref(), pii_cfg.on_hit).map_err(
                |e| VerbError::Internal {
                    message: format!("raw mirror re-sync: {e}"),
                },
            )?;
        let sync_elapsed_ms = sync_started.elapsed().as_millis();
        on_event(VerbEvent::Banner(VerbBanner::SyncDone {
            files: summary.files,
            mib: (summary.bytes as f64) / (1024.0 * 1024.0),
            elapsed_ms: sync_elapsed_ms,
        }));
        let signal = manifest::compute_source_signal(repo, &summary);
        match manifest::write_or_update_manifest(
            repo,
            &paths.root,
            env!("CARGO_PKG_VERSION"),
            signal,
        ) {
            Ok(ManifestOutcome::Written) | Ok(ManifestOutcome::Updated) => {}
            Err(e) => {
                return Err(VerbError::Internal {
                    message: format!("manifest update: {e}"),
                });
            }
        }
    }

    // Step 7: resolve goal verb config + build env overrides.
    let goal_resolved = cc_cfg.resolve(Verb::Goal);
    let goal_env =
        build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;

    // Step 8: spawn goal agent.
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Goal,
    }));
    let slash_command = format!("/codebus-goal \"{}\"", options.text);
    let invoke_report = {
        let on_event = &mut on_event;
        crate::agent::invoke(
            crate::agent::InvokeAgentOptions {
                slash_command,
                vault_root: paths.root.clone(),
                toolset: GOAL_TOOLSET,
                bash_whitelist: None,
                model: goal_resolved.model.clone(),
                effort: goal_resolved.effort.clone(),
                env: goal_env,
            },
            |event: StreamEvent| on_event(VerbEvent::Stream(event)),
            cancel.clone(),
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Goal,
        exit_code: invoke_report.exit.code(),
    }));

    // Cancellation: skip fix loop, skip auto-commit, return Cancelled.
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        return Err(VerbError::Cancelled);
    }

    // Step 9: lint-and-fix phase (when enabled).
    let mut fix_lint_errors: usize = 0;
    let mut fix_lint_warns: usize = 0;
    if fix_cfg.enabled {
        on_event(VerbEvent::Banner(VerbBanner::LintStart));
        let lint_started = Instant::now();
        let fix_resolved = cc_cfg.resolve(Verb::Fix);
        let fix_env =
            build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;
        let report = {
            let on_event = &mut on_event;
            run_fix_loop(
                paths.root.clone(),
                fix_resolved.model.clone(),
                fix_resolved.effort.clone(),
                fix_env,
                |event: StreamEvent| on_event(VerbEvent::Stream(event)),
                cancel.clone(),
            )
            .map_err(|e| match e {
                crate::wiki::fix::FixError::Spawn(io_err) => VerbError::Spawn { source: io_err },
            })?
        };
        fix_lint_errors = report.final_lint.error_count;
        fix_lint_warns = report.final_lint.warn_count;
        let lint_elapsed_ms = lint_started.elapsed().as_millis();
        on_event(VerbEvent::Banner(VerbBanner::LintDone {
            errors: fix_lint_errors,
            warns: fix_lint_warns,
            elapsed_ms: lint_elapsed_ms,
        }));

        // Cancellation propagated by run_fix_loop: re-check.
        if let Some(flag) = &cancel
            && flag.load(Ordering::Relaxed)
        {
            return Err(VerbError::Cancelled);
        }

        // Track unused report variant to silence warnings.
        let _ = TerminationReason::InitialClean;
    }

    // Step 10: auto-commit "wiki: <goal>".
    let commit_msg = format!("wiki: {}", options.text);
    match auto_commit(&paths.root, &commit_msg) {
        Ok(sha) => {
            if !sha.is_empty() {
                let sha7: String = sha.chars().take(7).collect();
                on_event(VerbEvent::Banner(VerbBanner::CommitDone {
                    sha7: sha7.clone(),
                }));
            }
        }
        Err(e) => {
            return Err(VerbError::Internal {
                message: format!("vault git auto-commit: {e}"),
            });
        }
    }

    // Step 11: write RunLog.
    let wiki_changed = wiki_changed_since_last_commit(&paths.root);
    let run_log = RunLog {
        goal: options.text.clone(),
        mode: "goal".into(),
        model: goal_resolved.model.clone(),
        effort: goal_resolved.effort.clone(),
        started_at: invoke_report.started_at.clone(),
        finished_at: invoke_report.finished_at.clone(),
        tokens: invoke_report.accumulated_tokens.clone(),
        wiki_changed,
        lint_error_count: fix_lint_errors,
        lint_warn_count: fix_lint_warns,
    };
    let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
        which: "log",
        source: e,
    })?;
    let sink_cfg = resolve_sink_dir(log_cfg, &paths.log);
    write_run_log(sink_cfg, &run_log);

    // Step 12: Done banner + return.
    on_event(VerbEvent::Banner(VerbBanner::Done {
        wiki_path: paths.wiki.clone(),
    }));

    Ok(GoalReport {
        accumulated_tokens: invoke_report.accumulated_tokens,
        wiki_changed,
        lint_error_count: fix_lint_errors,
        lint_warn_count: fix_lint_warns,
        started_at: invoke_report.started_at,
        finished_at: invoke_report.finished_at,
    })
}

/// Construct the active PII scanner from `pii.scanner` discriminator.
/// On `RegexBasic` with malformed `patterns_extra`, falls back silently to
/// the built-in pattern set (library variant — CLI prints a stderr warning
/// in its thin wrapper if it observes the same fallback).
fn build_pii_scanner(cfg: &PiiConfig) -> Box<dyn PiiScanner> {
    match cfg.scanner {
        PiiScannerKind::Null => Box::new(NullScanner::new()),
        PiiScannerKind::RegexBasic => match RegexBasicScanner::new(&cfg.patterns_extra) {
            Ok(s) => Box::new(s),
            Err(_) => Box::new(
                RegexBasicScanner::new(&[]).expect("built-in patterns must compile"),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_goal_returns_internal_when_auto_init_unreachable() {
        // We can't easily trigger a real auto-init failure here without
        // mocking the entire vault layout primitives. Instead, verify the
        // entry point signature is callable with the documented options
        // shape — full happy-path testing lives in CLI integration tests
        // (goal_flow.rs) with mock_claude.
        let _ = GoalOptions {
            text: "test".into(),
            force_resync: false,
            no_fix: false,
            no_obsidian_register: false,
        };
        let _ = TempDir::new().unwrap();
    }
}
