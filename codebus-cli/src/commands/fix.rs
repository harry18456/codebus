//! `codebus fix` subcommand — run the single-shot fix flow against a vault.
//!
//! Per v3-fix-trust-agent Fix Subcommand Behavior:
//! - Vault precondition: `<repo>/.codebus/` MUST exist (no auto-init).
//! - `--no-fix` short-circuit: exit 0 with stderr message, no agent spawn.
//! - Lint pre-check: 0 issues → exit 0, no agent spawn.
//! - Else spawn fix agent exactly once, run lint final check, auto-commit
//!   `wiki: lint fix loop`, exit 0 if final lint clean / 1 if issues remain.

use std::path::Path;
use std::process::ExitCode;

use codebus_core::config::{
    ClaudeCodeConfig, LintFixConfig, Verb, build_env_overrides, default_config_path,
    load_claude_code_config, load_lint_fix_config,
};
use codebus_core::git::auto_commit;
use codebus_core::log::RunLog;
use codebus_core::render::{Banner, RenderOptions, print_banner};
use codebus_core::vault::layout::vault_paths;
use codebus_core::wiki::fix::{TerminationReason, run_fix_loop};
use std::time::Instant;

use crate::run_log::{
    load_log_config_with_warning, resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};

pub async fn run(
    repo_override: Option<&Path>,
    no_fix: bool,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    // Resolve target repo (Some from --repo, else cwd).
    let repo = match repo_override {
        Some(p) => p.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: read current dir: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let paths = vault_paths(&repo);

    // Vault precondition: refuse if .codebus/ missing (no auto-init).
    if !paths.root.exists() {
        eprintln!(
            "error: no codebus vault at {} — run `codebus init` first",
            paths.root.display()
        );
        return ExitCode::from(2);
    }

    print_banner(Banner::Start { repo_path: &repo }, render_opts);

    if debug {
        eprintln!("[debug] fix: vault = {}", paths.root.display());
        eprintln!("[debug] fix: --no-fix = {no_fix}");
    }

    // Load config and merge CLI overrides. Fail-loud on parse error
    // (spec: cli / Config Parse Failure Aborts Invocation).
    let cfg = match default_config_path() {
        Some(p) => match load_lint_fix_config(&p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "error: lint.fix config parse failed at {}: {e}",
                    p.display()
                );
                return ExitCode::from(2);
            }
        },
        None => LintFixConfig::default(),
    };
    let cfg = cfg.merge_cli_overrides(no_fix);

    if !cfg.enabled {
        eprintln!("fix: disabled by --no-fix or lint.fix.enabled = false");
        return ExitCode::from(0);
    }

    // Load claude_code.fix config so the spawned agent gets the configured
    // model/effort flags. Fail-loud on parse error per spec.
    let cc_cfg = match default_config_path() {
        Some(p) => match load_claude_code_config(&p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "error: claude_code config parse failed at {}: {e}",
                    p.display()
                );
                return ExitCode::from(2);
            }
        },
        None => ClaudeCodeConfig::default(),
    };

    // Run the single-shot flow. The fix module handles initial-clean
    // short-circuit internally — no spawn on clean vault.
    let fix_resolved = cc_cfg.resolve(Verb::Fix);
    // Profile-aware env injection (system → empty; azure → 3-key inject
    // with key resolved via keyring → env fallback chain). Failure to
    // resolve the key → exit non-zero BEFORE the fix lint-spawn loop
    // begins.
    let fix_env = match build_env_overrides(&cc_cfg) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("error: fix: {e}");
            return ExitCode::from(3);
        }
    };
    print_banner(Banner::LintStart, render_opts);
    let lint_started = Instant::now();
    let render_opts_for_fix = render_opts.clone();
    let report = match run_fix_loop(
        paths.root.clone(),
        fix_resolved.model.clone(),
        fix_resolved.effort.clone(),
        fix_env,
        move |event| codebus_core::render::print_event(&event, &render_opts_for_fix),
        None,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: fix loop: {e}");
            return ExitCode::from(1);
        }
    };
    let lint_elapsed_ms = lint_started.elapsed().as_millis();

    if debug {
        eprintln!(
            "[debug] fix: agent_skipped = {}, termination = {:?}, errors = {}, warns = {}",
            report.agent_skipped,
            report.termination,
            report.final_lint.error_count,
            report.final_lint.warn_count
        );
    }

    print_banner(
        Banner::LintDone {
            errors: report.final_lint.error_count,
            warns: report.final_lint.warn_count,
            elapsed_ms: lint_elapsed_ms,
        },
        render_opts,
    );

    // Initial-clean termination: nothing to commit, exit 0.
    if report.termination == TerminationReason::InitialClean {
        if debug {
            println!("✓ fix: vault already clean, no agent spawned");
        }
        return ExitCode::from(0);
    }

    // Auto-commit any changes (no-op when working tree is clean).
    match auto_commit(&paths.root, "wiki: lint fix loop") {
        Ok(sha) => {
            let sha7: String = sha.chars().take(7).collect();
            if debug {
                if !sha.is_empty() {
                    println!("✓ fix: committed {sha7} \"wiki: lint fix loop\"");
                } else {
                    println!("✓ fix: no changes to commit");
                }
            }
            if !sha.is_empty() {
                print_banner(Banner::CommitDone { sha7: &sha7 }, render_opts);
            }
        }
        Err(e) => {
            eprintln!("error: vault auto-commit: {e}");
            return ExitCode::from(1);
        }
    }

    // v3-run-log: persist a RunLog entry. Uses the InvokeReport from the
    // FixReport (Some only when the agent actually spawned — InitialClean
    // path returned earlier without writing a log entry).
    if let Some(invoke_report) = &report.invoke {
        let run_log = RunLog {
            goal: String::new(), // fix has no positional argument
            mode: "fix".into(),
            model: fix_resolved.model.clone(),
            effort: fix_resolved.effort.clone(),
            started_at: invoke_report.started_at.clone(),
            finished_at: invoke_report.finished_at.clone(),
            tokens: invoke_report.accumulated_tokens.clone(),
            wiki_changed: wiki_changed_since_last_commit(&paths.root),
            lint_error_count: report.final_lint.error_count,
            lint_warn_count: report.final_lint.warn_count,
        };
        let log_cfg = match load_log_config_with_warning() {
            Ok(c) => c,
            Err(code) => return code,
        };
        let sink_cfg = resolve_sink_dir(log_cfg, &paths.log);
        write_run_log(sink_cfg, &run_log);
    }

    if report.clean {
        if debug {
            println!("✓ fix complete (vault clean)");
        }
        ExitCode::from(0)
    } else {
        eprintln!(
            "✗ fix: {} error(s), {} warning(s) remain after agent terminated",
            report.final_lint.error_count, report.final_lint.warn_count
        );
        ExitCode::from(1)
    }
}
