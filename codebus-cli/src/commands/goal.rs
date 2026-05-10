//! `codebus goal "..."` — spawn `claude -p` with the codebus-goal slash
//! command, drive the agent against the vault, and commit the resulting
//! wiki snapshot. See openspec/changes/v3-goal/ for the full design.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::agent::{InvokeAgentOptions, invoke};
use codebus_core::config::{
    ClaudeCodeConfig, LintFixConfig, PiiConfig, PiiScannerKind, default_config_path,
    load_claude_code_config, load_lint_fix_config, load_pii_config,
};
use codebus_core::git::auto_commit;
use codebus_core::pii::PiiScanner;
use codebus_core::pii::scanners::null_scanner::NullScanner;
use codebus_core::pii::scanners::regex_basic::RegexBasicScanner;
use codebus_core::vault::layout::vault_paths;
use codebus_core::vault::manifest::{self, ManifestOutcome, SourceSignal};
use codebus_core::vault::raw_sync::{sync_with_scanner, walk_source_for_signal, SyncSummary};
use codebus_core::vault::source_signal_detect::detect_drift;
use codebus_core::wiki::fix::{TerminationReason, run_fix_loop};

use crate::commands::init;

/// Toolset for goal: read code, write/edit wiki pages. No Bash (fix gets that),
/// no WebFetch / Task / NotebookEdit / etc. v2 iter-9 carry, spike-verified
/// 2026-05-09 to be a real hard gate.
const GOAL_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

#[derive(Args, Debug)]
pub struct GoalArgs {
    /// What you want the wiki to capture or update.
    #[arg(value_name = "GOAL")]
    pub text: String,

    /// Force re-syncing the raw mirror even if source-signal detection
    /// reports no drift. Use when you know the source changed in a way
    /// the signal does not capture (e.g. file content changed but size
    /// stayed identical).
    #[arg(long = "force-resync")]
    pub force_resync: bool,
}

pub async fn run(
    repo: &Path,
    args: GoalArgs,
    no_obsidian_register: bool,
    no_fix: bool,
    debug: bool,
) -> ExitCode {
    let paths = vault_paths(repo);

    if debug {
        eprintln!(
            "[debug] goal: repo={}, vault={}, force_resync={}",
            repo.display(),
            paths.root.display(),
            args.force_resync
        );
    }

    // Step 1+2: vault precondition. v2 carry: missing vault → auto-init.
    if !paths.root.exists() {
        if debug {
            eprintln!(
                "[debug] goal: vault {} missing, running init flow",
                paths.root.display()
            );
        }
        let init_exit = init::run(repo, no_obsidian_register, debug).await;
        if !paths.root.exists() {
            // ExitCode is opaque (no PartialEq); detect failure by post-condition.
            eprintln!(
                "error: goal: auto-init did not create vault at {}",
                paths.root.display()
            );
            return init_exit;
        }
    }

    // Load fix loop config (used by Step 5 lint-and-fix phase).
    let fix_cfg = match default_config_path() {
        Some(p) => match load_lint_fix_config(&p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: lint.fix config load failed (using defaults): {e}");
                LintFixConfig::default()
            }
        },
        None => LintFixConfig::default(),
    };
    let fix_cfg = fix_cfg.merge_cli_overrides(no_fix);

    // Step 3: source-signal detection + conditional re-sync.
    let current_signal = match compute_current_signal(repo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: goal: compute source signal: {e}");
            return ExitCode::from(1);
        }
    };

    let needs_resync = args.force_resync || detect_drift(&paths.manifest_yaml, &current_signal);
    if debug {
        eprintln!(
            "[debug] goal: detect_drift={}, force_resync={}, needs_resync={}",
            !args.force_resync && needs_resync,
            args.force_resync,
            needs_resync
        );
    }

    // Load pii + claude_code config once; scanner dispatch + agent flag
    // forwarding both use them.
    let pii_cfg = load_pii_config_with_warning();
    let cc_cfg = load_claude_code_config_with_warning();

    if needs_resync {
        let scanner = build_pii_scanner(&pii_cfg);
        let summary = match sync_with_scanner(
            repo,
            &paths.raw_code,
            scanner.as_ref(),
            pii_cfg.on_hit,
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: raw mirror re-sync: {e}");
                return ExitCode::from(1);
            }
        };
        println!(
            "✓ raw mirror: {} files, {} bytes, {} PII matches (re-sync)",
            summary.files, summary.bytes, summary.pii_matches
        );
        let signal = manifest::compute_source_signal(repo, &summary);
        match manifest::write_or_update_manifest(
            repo,
            &paths.root,
            env!("CARGO_PKG_VERSION"),
            signal,
        ) {
            Ok(ManifestOutcome::Written) | Ok(ManifestOutcome::Updated) => {
                if debug {
                    eprintln!("[debug] goal: manifest source_signal updated after re-sync");
                }
            }
            Err(e) => {
                eprintln!("error: manifest update: {e}");
                return ExitCode::from(1);
            }
        }
    } else if debug {
        eprintln!("[debug] goal: source unchanged, skipping raw mirror re-sync");
    }

    // Step 4: spawn agent with canonical sandbox.
    let slash_command = format!("/codebus-goal \"{}\"", args.text);
    if debug {
        eprintln!(
            "[debug] goal: spawn claude with cwd={} slash={:?} toolset={:?}",
            paths.root.display(),
            slash_command,
            GOAL_TOOLSET
        );
    }
    let child_status = match invoke(InvokeAgentOptions {
        slash_command,
        vault_root: paths.root.clone(),
        toolset: GOAL_TOOLSET,
        bash_whitelist: None,
        model: cc_cfg.goal.model.clone(),
        effort: cc_cfg.goal.effort.clone(),
    }) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("error: spawn claude: {e}");
            return ExitCode::from(1);
        }
    };
    let child_exit_code: u8 = child_status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1);
    if debug {
        eprintln!(
            "[debug] goal: agent exited code={}, success={}",
            child_status.code().unwrap_or(-1),
            child_status.success()
        );
    }

    // Step 5: lint-and-fix phase between goal agent and auto-commit.
    // v3-fix-trust-agent: single-shot — spawn fix agent at most once, then
    // CLI runs final lint as authoritative state. Per Goal Subcommand
    // Behavior: insert this phase after the goal agent terminates and
    // BEFORE auto-commit, so wiki writes from the goal agent and any
    // repair edits land in the same single commit.
    let mut fix_exit: u8 = 0;
    if fix_cfg.enabled {
        if debug {
            eprintln!("[debug] goal: running lint-and-fix phase (single-shot)");
        }
        match run_fix_loop(
            paths.root.clone(),
            cc_cfg.fix.model.clone(),
            cc_cfg.fix.effort.clone(),
        ) {
            Ok(report) => {
                if debug {
                    eprintln!(
                        "[debug] goal: fix agent_skipped={}, termination={:?}, errors={}, warns={}",
                        report.agent_skipped,
                        report.termination,
                        report.final_lint.error_count,
                        report.final_lint.warn_count
                    );
                }
                if !report.clean
                    && report.termination == TerminationReason::PostLintIssuesRemain
                {
                    eprintln!(
                        "✗ fix: {} error(s), {} warning(s) remain after agent terminated",
                        report.final_lint.error_count, report.final_lint.warn_count
                    );
                    fix_exit = 1;
                }
            }
            Err(e) => {
                eprintln!("error: lint-and-fix phase: {e}");
                fix_exit = 1;
            }
        }
    } else if debug {
        eprintln!("[debug] goal: lint-and-fix phase skipped (--no-fix or config disabled)");
    }

    // Step 6: auto-commit (v2 carry behavior — fold ingest + fix edits
    // into ONE commit). Clean working tree → no-op; dirty → commit
    // "wiki: <goal>" preserving partial work even when agent / fix failed.
    let commit_msg = format!("wiki: {}", args.text);
    match auto_commit(&paths.root, &commit_msg) {
        Ok(sha) => {
            let sha7: String = sha.chars().take(7).collect();
            println!("✓ vault git: committed {sha7} \"{commit_msg}\"");
        }
        Err(e) => {
            eprintln!("error: vault git auto-commit: {e}");
            return ExitCode::from(1);
        }
    }

    // Exit code precedence: goal agent failure preempts fix exit.
    // Auto-commit failure (above) preempts both.
    if child_exit_code != 0 {
        ExitCode::from(child_exit_code)
    } else {
        ExitCode::from(fix_exit)
    }
}

/// Load `pii.*` config from default path with warn-and-default on parse failure.
fn load_pii_config_with_warning() -> PiiConfig {
    let path = match default_config_path() {
        Some(p) => p,
        None => return PiiConfig::default(),
    };
    match load_pii_config(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: pii config load failed (using defaults): {e}");
            PiiConfig::default()
        }
    }
}

/// Load `claude_code.*` config from default path with warn-and-default on parse failure.
fn load_claude_code_config_with_warning() -> ClaudeCodeConfig {
    let path = match default_config_path() {
        Some(p) => p,
        None => return ClaudeCodeConfig::default(),
    };
    match load_claude_code_config(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: claude_code config load failed (using defaults): {e}");
            ClaudeCodeConfig::default()
        }
    }
}

/// Construct the active PII scanner from `pii.scanner` discriminator.
/// On `RegexBasic` with malformed `patterns_extra`, falls back to the built-in
/// pattern set only after emitting a stderr warning.
fn build_pii_scanner(cfg: &PiiConfig) -> Box<dyn PiiScanner> {
    match cfg.scanner {
        PiiScannerKind::Null => Box::new(NullScanner::new()),
        PiiScannerKind::RegexBasic => match RegexBasicScanner::new(&cfg.patterns_extra) {
            Ok(s) => Box::new(s),
            Err(e) => {
                eprintln!(
                    "warning: pii config patterns_extra failed to compile (using built-in patterns only): {e}"
                );
                Box::new(
                    RegexBasicScanner::new(&[])
                        .expect("built-in patterns must compile"),
                )
            }
        },
    }
}

/// Compute the current source signal: walk source for file_count/total_bytes,
/// read git_head from `<repo>/.git/HEAD`. Reuses the manifest module's
/// `compute_source_signal` after replacing the SyncSummary's file/bytes with
/// the walk-only result (so we don't depend on having just run a real sync).
fn compute_current_signal(repo: &Path) -> std::io::Result<SourceSignal> {
    let (files, bytes) = walk_source_for_signal(repo)?;
    let stub_summary = SyncSummary {
        files,
        bytes,
        pii_matches: 0,
        pii_skipped_files: 0,
        pii_masked_matches: 0,
    };
    Ok(manifest::compute_source_signal(repo, &stub_summary))
}
