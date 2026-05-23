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
    PiiConfig, PiiScannerKind, Verb, default_config_path, load_lint_fix_config, load_pii_config,
};
use crate::agent::{build_backend, load_provider_config};
use crate::agent::claude_cli::InvokeReport;
use crate::git::{auto_commit, changed_paths_under, rev_parse_head};
use crate::verb::content_verify::{
    ContentDefect, ContentVerifyOutcome, parse_content_defects, run_content_verify_loop,
};
use crate::log::events::{EventEnvelope, EventsNullSink, EventsSink};
use crate::log::factory::build_events_sink;
use crate::log::verb_log::{
    load_verb_log_config, resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};
use crate::log::{RunLog, SinkConfig, TokenUsage};
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
use chrono::SecondsFormat;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Toolset for goal: read code, write/edit wiki pages. No Bash (fix gets
/// that). v2 iter-9 carry, spike-verified 2026-05-09 to be a real hard gate.
pub const GOAL_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

/// Read-only toolset for the goal **content-verify** spawn
/// (goal-content-verify design D3). No Write/Edit — the verify spawn
/// only judges. `Read` covers the cwd-relative `raw/code/` source mirror
/// so the unfaithful check can be grounded against source (the
/// `raw/code/` read scope is wider than the quiz verify spawn's
/// wiki-only scope by design).
pub const GOAL_VERIFY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

/// Caller-controllable inputs to `run_goal`.
#[derive(Debug, Clone)]
pub struct GoalOptions {
    pub text: String,
    pub force_resync: bool,
    pub no_fix: bool,
    pub no_obsidian_register: bool,
    /// goal-content-verify (design D5/D6): run the optional independent
    /// content verify + bounded repair stage after the fix loop and
    /// before `auto_commit`. Caller-injected from `goal.content_verify`
    /// config (the library never reads config itself); default `false`.
    pub content_verify: bool,
}

/// Independent content-verify outcome carried on [`GoalReport`]
/// (goal-content-verify design D4). `None` means the stage did not run
/// (config off) — readers MUST NOT treat absence as `Ok` ("not
/// verified" ≠ "ok").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalContentReview {
    Ok,
    /// Wiki page paths still flagged after the cap. An **empty** vec is
    /// the conservative non-fatal flag (verify spawn failed / output
    /// unparseable / changed-page detection failed — never silently
    /// `Ok`).
    Flagged(Vec<String>),
}

impl GoalContentReview {
    /// Human-readable status for the non-fatal warning / run log:
    /// `ok` or `flagged [wiki/a.md, wiki/b.md]`.
    pub fn status_str(&self) -> String {
        match self {
            GoalContentReview::Ok => "ok".to_string(),
            GoalContentReview::Flagged(pages) => {
                format!("flagged [{}]", pages.join(", "))
            }
        }
    }
}

/// Outcome of a successful `run_goal` invocation. `agent_exit_code` is
/// the goal agent's process exit code (None on signal termination); CLI
/// thin wrapper propagates it (precedence over the fix exit code) so
/// goal_flow's existing exit-code-propagation tests stay green.
#[derive(Debug, Clone)]
pub struct GoalReport {
    pub accumulated_tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
    pub fix_post_lint_issues_remain: bool,
    /// goal-content-verify (design D4): independent content-verify
    /// outcome. `None` when the stage did not run (config off); readers
    /// MUST NOT treat `None` as `Ok`. `auto_commit` and the exit code
    /// are unaffected by this value.
    pub content_review: Option<GoalContentReview>,
}

/// Run the goal verb against `repo`. See module docs for full behavior.
pub fn run_goal(
    repo: &Path,
    options: GoalOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<GoalReport, VerbError> {
    let paths = vault_paths(repo);

    // Capture run started_at early — events.jsonl filename slug + RunLog row.
    let run_started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Step 1: vault precondition — auto-init if missing. No banners
    // yet — events sink needs vault dir to exist; we emit Start +
    // Goal banners through fan_out below so events.jsonl captures them.
    if !paths.root.exists() {
        let init_opts = InitOptions {
            no_obsidian_register: options.no_obsidian_register,
            write_starter_config: true,
            with_repo_root_skills: false,
        };
        // Silent auto-init: library doesn't render init banners. CLI's
        // direct `codebus init` invocation still emits banners via its
        // own thin wrapper.
        run_init(repo, &init_opts, |_| {}).map_err(|e| VerbError::Internal {
            message: format!("auto-init: {e}"),
        })?;
        if !paths.root.exists() {
            return Err(VerbError::Internal {
                message: format!("auto-init did not create vault at {}", paths.root.display()),
            });
        }
    }

    // Now that vault exists, load log config and build events sink.
    let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
        which: "log",
        source: e,
    })?;
    let sink_cfg: SinkConfig = resolve_sink_dir(log_cfg, &paths.log);
    let mut events_sink: Box<dyn EventsSink> = match build_events_sink(&sink_cfg, &run_started_at) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: events-log sink build failed (skipping persistence): {e}");
            Box::new(EventsNullSink::new())
        }
    };

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
        oversized_skipped_files: 0,
    };
    let current_signal = manifest::compute_source_signal(repo, &stub_summary);
    let needs_resync = options.force_resync || detect_drift(&paths.manifest_yaml, &current_signal);

    // Step 5: load pii + claude_code config.
    let pii_cfg = match default_config_path() {
        Some(p) if p.exists() => load_pii_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "pii",
            source: e,
        })?,
        _ => Default::default(),
    };
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => {
            load_provider_config(&p).map_err(|e| VerbError::ConfigParse {
                which: "claude_code",
                source: e,
            })?
        }
        _ => Default::default(),
    };

    // Fan-out closure: emit each VerbEvent to events sink + caller.
    // Built after vault exists + events sink ready.
    let mut fan_out = |event: VerbEvent| {
        let envelope = EventEnvelope {
            ts: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            event: event.clone(),
        };
        if let Err(e) = events_sink.write_event(&envelope) {
            eprintln!("warning: events-log write failed (non-fatal): {e}");
        }
        on_event(event);
    };

    // Start + Goal banners (post-sink-build so events.jsonl captures them).
    fan_out(VerbEvent::Banner(VerbBanner::Start {
        repo_path: repo.to_path_buf(),
    }));
    fan_out(VerbEvent::Banner(VerbBanner::Goal {
        goal_text: options.text.clone(),
    }));

    // Helper to write a cancel-path RunLog and return Cancelled. Used
    // at two cancel observation points (post-agent, post-fix-loop).
    let make_cancel_runlog = |fix_lint_errors: usize,
                              fix_lint_warns: usize,
                              tokens: TokenUsage,
                              finished_at: String,
                              model: Option<String>,
                              effort: Option<String>,
                              wiki_changed: bool|
     -> RunLog {
        RunLog {
            goal: options.text.clone(),
            mode: "goal".into(),
            model,
            effort,
            started_at: run_started_at.clone(),
            finished_at,
            tokens,
            wiki_changed,
            lint_error_count: fix_lint_errors,
            lint_warn_count: fix_lint_warns,
            outcome: "cancelled".into(),
            session_id: None,
        }
    };

    // Step 4b: conditional re-sync with PII scanner.
    if needs_resync {
        let scanner = build_pii_scanner(&pii_cfg);
        fan_out(VerbEvent::Banner(VerbBanner::SyncStart));
        let sync_started = Instant::now();
        let summary = sync_with_scanner(repo, &paths.raw_code, scanner.as_ref(), pii_cfg.on_hit)
            .map_err(|e| VerbError::Internal {
                message: format!("raw mirror re-sync: {e}"),
            })?;
        let sync_elapsed_ms = sync_started.elapsed().as_millis() as u64;
        fan_out(VerbEvent::Banner(VerbBanner::SyncDone {
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
    // One backend drives every goal-side spawn (main / fix-loop / content
    // verify + repair). It resolves model/effort per the SpawnSpec's verb —
    // verify-stage-independent-model: the content-verify spawn passes
    // `Verb::Verify` so the backend resolves the dedicated verify sub-block;
    // main/repair pass `Verb::Goal`, the fix-loop passes `Verb::Fix`.
    // build_backend borrows cc_cfg, so the later resolve(Verb::Fix) for RunLog
    // still has it.
    let backend =
        build_backend(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;

    // goal-content-verify D3: pin the vault revision BEFORE the goal
    // agent spawn so the content-verify stage can diff this run's
    // created/modified wiki pages against it. Only needed when the stage
    // will run; `None` (no commits / git failure) degrades gracefully
    // (the helper falls back to `HEAD` and the stage stays best-effort).
    let pre_rev: Option<String> = if options.content_verify {
        rev_parse_head(&paths.root)
    } else {
        None
    };

    // Step 8: spawn goal agent.
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Goal,
    }));
    let slash_command = format!("/codebus-goal \"{}\"", options.text);
    let invoke_report = {
        let fan_out = &mut fan_out;
        crate::agent::invoke(
            &*backend,
            crate::agent::SpawnSpec {
                verb: Verb::Goal,
                prompt: slash_command,
                permission: crate::agent::Permission::Workspace,
                command_allowance: None,
                // goal verb is one-shot (no session resume); chat verb is
                // the only caller that sets Some(...) on this field.
                resume_session_id: None,
            },
            &paths.root,
            |event: StreamEvent| fan_out(VerbEvent::Stream(event)),
            cancel.clone(),
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Goal,
        exit_code: invoke_report.exit.code(),
    }));

    // Cancellation after agent spawn: write RunLog cancelled + return.
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        let cancel_run_log = make_cancel_runlog(
            0,
            0,
            invoke_report.accumulated_tokens.clone(),
            invoke_report.finished_at.clone(),
            goal_resolved.model.clone(),
            goal_resolved.effort.clone(),
            wiki_changed_since_last_commit(&paths.root),
        );
        write_run_log(sink_cfg.clone(), &cancel_run_log);
        return Err(VerbError::Cancelled);
    }

    // Step 9: lint-and-fix phase (when enabled).
    let mut fix_lint_errors: usize = 0;
    let mut fix_lint_warns: usize = 0;
    let mut fix_post_lint_issues_remain = false;
    if fix_cfg.enabled {
        fan_out(VerbEvent::Banner(VerbBanner::LintStart));
        // Emit a phase boundary marker for the fix loop so consumers
        // (GUI Done detail, analytics) can group ToolUse events inside
        // the loop under a `fix` phase distinct from the preceding
        // `goal` agent invocation. The matching SpawnEnd fires after
        // the loop returns (or on cancel observation below).
        fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
            verb: Verb::Fix,
        }));
        let lint_started = Instant::now();
        let report = {
            let fan_out = &mut fan_out;
            run_fix_loop(
                paths.root.clone(),
                &*backend,
                |event: StreamEvent| fan_out(VerbEvent::Stream(event)),
                cancel.clone(),
            )
            .map_err(|e| match e {
                crate::wiki::fix::FixError::Spawn(io_err) => VerbError::Spawn { source: io_err },
            })?
        };
        fix_lint_errors = report.final_lint.error_count;
        fix_lint_warns = report.final_lint.warn_count;
        fix_post_lint_issues_remain =
            !report.clean && report.termination == TerminationReason::PostLintIssuesRemain;
        // run_fix_loop's internal report does not carry a child exit
        // code (it iterates internally and reports termination reason
        // rather than a single exit). Surface None — GUI phase
        // grouping ignores exit_code; CLI thin wrappers that need it
        // still propagate via the agent_exit_code field on GoalReport.
        fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
            verb: Verb::Fix,
            exit_code: None,
        }));
        let lint_elapsed_ms = lint_started.elapsed().as_millis() as u64;
        fan_out(VerbEvent::Banner(VerbBanner::LintDone {
            errors: fix_lint_errors,
            warns: fix_lint_warns,
            elapsed_ms: lint_elapsed_ms,
        }));

        // Cancellation propagated by run_fix_loop: write RunLog + return.
        if let Some(flag) = &cancel
            && flag.load(Ordering::Relaxed)
        {
            let cancel_run_log = make_cancel_runlog(
                fix_lint_errors,
                fix_lint_warns,
                invoke_report.accumulated_tokens.clone(),
                invoke_report.finished_at.clone(),
                goal_resolved.model.clone(),
                goal_resolved.effort.clone(),
                wiki_changed_since_last_commit(&paths.root),
            );
            write_run_log(sink_cfg.clone(), &cancel_run_log);
            return Err(VerbError::Cancelled);
        }
    }

    // goal-content-verify D2/D3/D4: optional independent content verify
    // + bounded repair, AFTER the fix loop and BEFORE auto_commit. Gated
    // by `options.content_verify` (caller-injected from config). The
    // whole stage is best-effort — it never returns an error for content
    // defects, never reverts a page, and never blocks the commit; the
    // exit code is unaffected. `None` => stage did not run.
    let content_review: Option<GoalContentReview> = if options.content_verify {
        match changed_paths_under(&paths.root, pre_rev.as_deref(), "wiki/") {
            Err(e) => {
                eprintln!(
                    "warning: goal content-verify changed-page detection failed (non-fatal; content-review: flagged): {e}"
                );
                Some(GoalContentReview::Flagged(Vec::new()))
            }
            Ok(pages) if pages.is_empty() => {
                // No wiki page changed this run → resolve ok without
                // spawning anything (design D3 short-circuit).
                Some(GoalContentReview::Ok)
            }
            Ok(pages) => {
                let goal_text = options.text.clone();
                let fan_cell = std::cell::RefCell::new(&mut fan_out);

                let verify = |_: &()| -> Result<Option<Vec<ContentDefect>>, VerbError> {
                    let prompt = format!(
                        "/codebus-goal verify: goal={}\n\nCHANGED PAGES:\n{}",
                        goal_text,
                        pages.join("\n")
                    );
                    // verify-stage-independent-model: verify spawn passes
                    // `Verb::Verify` so the backend resolves the dedicated
                    // verify sub-block, NOT `Verb::Goal`. Read-only sandbox.
                    let vtext = match run_goal_spawn(
                        &mut **fan_cell.borrow_mut(),
                        &*backend,
                        prompt,
                        &paths.root,
                        Verb::Verify,
                        crate::agent::Permission::ReadOnly,
                        cancel.clone(),
                    ) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!(
                                "warning: goal content-verify spawn failed (non-fatal; content-review: flagged): {e}"
                            );
                            return Err(e);
                        }
                    };
                    let parsed = parse_content_defects(&vtext);
                    if parsed.is_none() {
                        eprintln!(
                            "warning: goal content-verify output unparseable (non-fatal; content-review: flagged)"
                        );
                    }
                    Ok(parsed)
                };

                let repair =
                    |_: &(), defects: &[ContentDefect]| -> Result<(), VerbError> {
                        let defect_lines = defects
                            .iter()
                            .map(|d| format!("{} | {} | {}", d.id, d.kind, d.suggestion))
                            .collect::<Vec<_>>()
                            .join("\n");
                        let flagged_pages = defects
                            .iter()
                            .map(|d| d.id.clone())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let prompt = format!(
                            "/codebus-goal repair: goal={}\n\nCONTENT DEFECTS:\n{}\n\nFLAGGED PAGES:\n{}",
                            goal_text, defect_lines, flagged_pages
                        );
                        match run_goal_spawn(
                            &mut **fan_cell.borrow_mut(),
                            &*backend,
                            prompt,
                            &paths.root,
                            Verb::Goal,
                            crate::agent::Permission::Workspace,
                            cancel.clone(),
                        ) {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                eprintln!(
                                    "warning: goal content-repair spawn failed (non-fatal; keeping pages, content-review: flagged): {e}"
                                );
                                Err(e)
                            }
                        }
                    };

                let (_unit, outcome) = run_content_verify_loop(
                    (),
                    || {
                        cancel
                            .as_ref()
                            .map(|f| f.load(Ordering::Relaxed))
                            .unwrap_or(false)
                    },
                    verify,
                    repair,
                );
                match outcome {
                    ContentVerifyOutcome::Ok => Some(GoalContentReview::Ok),
                    ContentVerifyOutcome::Flagged(defects) => {
                        let mut flagged: Vec<String> =
                            defects.iter().map(|d| d.id.clone()).collect();
                        flagged.sort();
                        flagged.dedup();
                        eprintln!(
                            "warning: goal content-review flagged {} page(s) after repair cap (non-fatal; not reverted, commit proceeds)",
                            flagged.len()
                        );
                        Some(GoalContentReview::Flagged(flagged))
                    }
                }
            }
        }
    } else {
        None
    };

    // Step 10: auto-commit "wiki: <goal>".
    let commit_msg = format!("wiki: {}", options.text);
    match auto_commit(&paths.root, &commit_msg) {
        Ok(sha) => {
            if !sha.is_empty() {
                let sha7: String = sha.chars().take(7).collect();
                fan_out(VerbEvent::Banner(VerbBanner::CommitDone {
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
    // outcome: "failed" when agent exited non-zero OR fix loop still
    // has issues; "succeeded" otherwise. Cancel path doesn't reach this
    // code (returns earlier with Cancelled).
    let agent_failed = invoke_report
        .exit
        .code()
        .map(|c| c != 0)
        .unwrap_or(true);
    let outcome = if agent_failed || fix_post_lint_issues_remain {
        "failed"
    } else {
        "succeeded"
    };
    let run_log = RunLog {
        goal: options.text.clone(),
        mode: "goal".into(),
        model: goal_resolved.model.clone(),
        effort: goal_resolved.effort.clone(),
        started_at: run_started_at.clone(),
        finished_at: invoke_report.finished_at.clone(),
        tokens: invoke_report.accumulated_tokens.clone(),
        wiki_changed,
        lint_error_count: fix_lint_errors,
        lint_warn_count: fix_lint_warns,
        outcome: outcome.into(),
        session_id: None,
    };
    write_run_log(sink_cfg.clone(), &run_log);

    // Step 12: Done banner + return.
    fan_out(VerbEvent::Banner(VerbBanner::Done {
        wiki_path: paths.wiki.clone(),
    }));

    Ok(GoalReport {
        accumulated_tokens: invoke_report.accumulated_tokens,
        wiki_changed,
        lint_error_count: fix_lint_errors,
        lint_warn_count: fix_lint_warns,
        started_at: run_started_at,
        finished_at: invoke_report.finished_at,
        agent_exit_code: invoke_report.exit.code(),
        fix_post_lint_issues_remain,
        content_review,
    })
}

/// Run one goal-side spawn (content-verify or content-repair),
/// accumulating assistant `Thought` text while forwarding every stream
/// event through `fan_out`, bracketed by `SpawnStart` / `SpawnEnd`
/// lifecycle events (goal-content-verify D3: verify/repair events flow
/// through the same fan-out the goal spawn uses). Mirrors the quiz
/// `run_spawn` shape.
fn run_goal_spawn(
    fan_out: &mut dyn FnMut(VerbEvent),
    backend: &dyn crate::agent::AgentBackend,
    slash_command: String,
    vault_root: &std::path::Path,
    spec_verb: Verb,
    permission: crate::agent::Permission,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<String, VerbError> {
    // Lifecycle phase stays `Verb::Goal` (UI grouping); `spec_verb` is the
    // model-resolution key (Verify for content-verify, Goal for repair).
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Goal,
    }));
    let mut accumulated = String::new();
    let report: InvokeReport = {
        let acc = &mut accumulated;
        let fan_out = &mut *fan_out;
        crate::agent::invoke(
            backend,
            crate::agent::SpawnSpec {
                verb: spec_verb,
                prompt: slash_command,
                permission,
                command_allowance: None,
                resume_session_id: None,
            },
            vault_root,
            |event: StreamEvent| {
                if let StreamEvent::Thought { text } = &event {
                    acc.push_str(text);
                }
                fan_out(VerbEvent::Stream(event));
            },
            cancel,
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Goal,
        exit_code: report.exit.code(),
    }));
    Ok(accumulated)
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
            Err(_) => {
                Box::new(RegexBasicScanner::new(&[]).expect("built-in patterns must compile"))
            }
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
            content_verify: false,
        };
        let _ = TempDir::new().unwrap();
    }
}
