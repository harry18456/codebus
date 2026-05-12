//! `verb::fix::run_fix` — single-shot fix-verb orchestration as a library
//! function. Mirrors the behavior previously inlined in
//! `codebus-cli/src/commands/fix.rs`.
//!
//! Behavior order:
//! 1. Emit `VerbBanner::Start { repo_path }`
//! 2. Strict vault precondition — `VerbError::VaultMissing` if missing
//! 3. Load `lint.fix` config — `VerbError::ConfigParse { which: "lint.fix" }`
//!    on parse failure
//! 4. Apply `options.no_fix` override → if disabled, return
//!    `FixReport { status: Skipped, .. }` (no agent spawn, no commit)
//! 5. Load `claude_code` config — `ConfigParse { which: "claude_code" }`
//! 6. Build env overrides — `VerbError::KeyringMissing` if Azure key
//!    unreachable
//! 7. Emit `VerbBanner::LintStart` + spawn-start lifecycle
//! 8. Run `wiki::fix::run_fix_loop` with the caller `on_event` wrapping
//!    each `StreamEvent` as `VerbEvent::Stream(_)`
//! 9. Emit `VerbBanner::LintDone` after the fix loop returns
//! 10. If `InitialClean` → return `FixReport { status: InitialClean, .. }`
//!     (no commit, no RunLog)
//! 11. Otherwise auto-commit `wiki: lint fix loop`; emit
//!     `VerbBanner::CommitDone` when a commit lands
//! 12. Load `log` config and write `RunLog`
//! 13. Return `FixReport` with status reflecting termination reason

use crate::agent::EnvOverrides;
use crate::config::{
    Verb, build_env_overrides, default_config_path, load_claude_code_config, load_lint_fix_config,
};
use crate::git::auto_commit;
use crate::log::verb_log::{
    load_verb_log_config, resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};
use crate::log::{RunLog, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbBanner, VerbEvent, VerbLifecycleEvent};
use crate::wiki::fix::{TerminationReason, run_fix_loop};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Caller-controllable inputs to `run_fix`.
#[derive(Debug, Clone, Default)]
pub struct FixOptions {
    /// When true, short-circuit before lint pre-check. Mirrors the CLI's
    /// `codebus fix --no-fix` flag.
    pub no_fix: bool,
}

/// Outcome of a `run_fix` invocation.
#[derive(Debug, Clone)]
pub struct FixReport {
    pub accumulated_tokens: TokenUsage,
    pub wiki_changed: bool,
    pub final_lint_error_count: usize,
    pub final_lint_warn_count: usize,
    /// Number of times the fix agent was spawned during the run. Always
    /// 0 or 1 today (single-shot model); future multi-iteration changes
    /// would bump this.
    pub fix_iterations: u8,
    /// Optional spawn timestamps. `None` when no agent spawned (skipped
    /// or InitialClean paths).
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub status: FixStatus,
}

/// Why `run_fix` terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixStatus {
    /// Fix was skipped because `options.no_fix` was true OR
    /// `lint.fix.enabled` was false in config.
    Skipped { reason: SkipReason },
    /// Initial lint pre-check reported zero issues; no agent spawn,
    /// no commit, no RunLog.
    InitialClean,
    /// Agent terminated; final lint reports zero issues.
    PostLintClean,
    /// Agent terminated; final lint still reports issues.
    PostLintIssuesRemain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// `FixOptions.no_fix` flag was set by the caller.
    NoFixFlag,
    /// `~/.codebus/config.yaml` `lint.fix.enabled` was false.
    DisabledByConfig,
}

/// Run the fix verb against `repo`. See module docs for full behavior.
pub fn run_fix(
    repo: &Path,
    options: FixOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<FixReport, VerbError> {
    let paths = vault_paths(repo);

    // Step 1: Start banner.
    on_event(VerbEvent::Banner(VerbBanner::Start {
        repo_path: repo.to_path_buf(),
    }));

    // Step 2: strict vault precondition.
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    // Step 3: load lint.fix config.
    let fix_cfg = match default_config_path() {
        Some(p) if p.exists() => load_lint_fix_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "lint.fix",
            source: e,
        })?,
        _ => Default::default(),
    };

    // Step 4: apply CLI override and check enabled.
    let fix_cfg = fix_cfg.merge_cli_overrides(options.no_fix);
    if !fix_cfg.enabled {
        let reason = if options.no_fix {
            SkipReason::NoFixFlag
        } else {
            SkipReason::DisabledByConfig
        };
        return Ok(FixReport {
            accumulated_tokens: TokenUsage::default(),
            wiki_changed: false,
            final_lint_error_count: 0,
            final_lint_warn_count: 0,
            fix_iterations: 0,
            started_at: None,
            finished_at: None,
            status: FixStatus::Skipped { reason },
        });
    }

    // Step 5: load claude_code config.
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => load_claude_code_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "claude_code",
            source: e,
        })?,
        _ => Default::default(),
    };

    // Step 6: resolve fix verb config + build env overrides.
    let fix_resolved = cc_cfg.resolve(Verb::Fix);
    let fix_env: EnvOverrides =
        build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;

    // Step 7: emit LintStart banner + SpawnStart lifecycle.
    on_event(VerbEvent::Banner(VerbBanner::LintStart));
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Fix,
    }));
    let lint_started = Instant::now();

    // Step 8: run fix loop. Wrap StreamEvent in VerbEvent::Stream.
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
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Fix,
        exit_code: report.invoke.as_ref().and_then(|r| r.exit.code()),
    }));
    let lint_elapsed_ms = lint_started.elapsed().as_millis();

    // Step 9: emit LintDone banner.
    on_event(VerbEvent::Banner(VerbBanner::LintDone {
        errors: report.final_lint.error_count,
        warns: report.final_lint.warn_count,
        elapsed_ms: lint_elapsed_ms,
    }));

    // Check cancel signal — if observed during the run, propagate.
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        return Err(VerbError::Cancelled);
    }

    // Step 10: InitialClean short-circuit (no commit, no RunLog).
    if report.termination == TerminationReason::InitialClean {
        return Ok(FixReport {
            accumulated_tokens: TokenUsage::default(),
            wiki_changed: false,
            final_lint_error_count: report.final_lint.error_count,
            final_lint_warn_count: report.final_lint.warn_count,
            fix_iterations: 0,
            started_at: None,
            finished_at: None,
            status: FixStatus::InitialClean,
        });
    }

    // Step 11: auto-commit (no-op when working tree is clean).
    match auto_commit(&paths.root, "wiki: lint fix loop") {
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
                message: format!("vault auto-commit: {e}"),
            });
        }
    }

    // Step 12: write RunLog (only when invoke ran — InitialClean path
    // returned earlier).
    let invoke_report = report.invoke.clone();
    let (started_at, finished_at, tokens) = match invoke_report {
        Some(r) => (
            Some(r.started_at),
            Some(r.finished_at),
            r.accumulated_tokens,
        ),
        None => (None, None, TokenUsage::default()),
    };

    if let (Some(s), Some(f)) = (started_at.as_ref(), finished_at.as_ref()) {
        let run_log = RunLog {
            goal: String::new(),
            mode: "fix".into(),
            model: fix_resolved.model.clone(),
            effort: fix_resolved.effort.clone(),
            started_at: s.clone(),
            finished_at: f.clone(),
            tokens: tokens.clone(),
            wiki_changed: wiki_changed_since_last_commit(&paths.root),
            lint_error_count: report.final_lint.error_count,
            lint_warn_count: report.final_lint.warn_count,
        };
        let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
            which: "log",
            source: e,
        })?;
        let sink_cfg = resolve_sink_dir(log_cfg, &paths.log);
        write_run_log(sink_cfg, &run_log);
    }

    // Step 13: return FixReport with status reflecting termination.
    let status = match report.termination {
        TerminationReason::InitialClean => FixStatus::InitialClean,
        TerminationReason::PostLintClean => FixStatus::PostLintClean,
        TerminationReason::PostLintIssuesRemain => FixStatus::PostLintIssuesRemain,
    };
    Ok(FixReport {
        accumulated_tokens: tokens,
        wiki_changed: wiki_changed_since_last_commit(&paths.root),
        final_lint_error_count: report.final_lint.error_count,
        final_lint_warn_count: report.final_lint.warn_count,
        fix_iterations: if report.agent_skipped { 0 } else { 1 },
        started_at,
        finished_at,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_fix_returns_vault_missing_when_no_codebus_dir() {
        let tmp = TempDir::new().unwrap();
        let events: std::cell::RefCell<Vec<VerbEvent>> = std::cell::RefCell::new(Vec::new());
        let result = run_fix(
            tmp.path(),
            FixOptions::default(),
            |event| events.borrow_mut().push(event),
            None,
        );
        match result {
            Err(VerbError::VaultMissing { path }) => {
                assert!(path.ends_with(".codebus"));
            }
            other => panic!("expected VaultMissing, got {other:?}"),
        }
    }
}
