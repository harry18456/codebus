//! `codebus fix` subcommand — run the fix loop against an existing vault.
//!
//! Per v3-lint Fix Subcommand Behavior:
//! - Vault precondition: `<repo>/.codebus/` MUST exist (no auto-init).
//! - `--no-fix` short-circuit: exit 0 with stderr message, no agent spawn.
//! - Lint pre-check: 0 issues → exit 0, no agent spawn.
//! - Else run fix loop, then auto-commit `wiki: lint fix loop`, then exit
//!   with status reflecting final lint state (0 clean, 1 issues remain).

use std::path::Path;
use std::process::ExitCode;

use codebus_core::config::{LintFixConfig, default_config_path, load_lint_fix_config};
use codebus_core::git::auto_commit;
use codebus_core::vault::layout::vault_paths;
use codebus_core::wiki::fix::{TerminationReason, run_fix_loop};

pub async fn run(
    repo_override: Option<&Path>,
    no_fix: bool,
    fix_max_iter: Option<u32>,
    debug: bool,
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

    if debug {
        eprintln!("[debug] fix: vault = {}", paths.root.display());
        eprintln!("[debug] fix: --no-fix = {no_fix}, --fix-max-iter = {fix_max_iter:?}");
    }

    // Load config and merge CLI overrides.
    let cfg = match default_config_path() {
        Some(p) => match load_lint_fix_config(&p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: lint.fix config load failed (using defaults): {e}");
                LintFixConfig::default()
            }
        },
        None => LintFixConfig::default(),
    };
    let cfg = cfg.merge_cli_overrides(no_fix, fix_max_iter);

    if !cfg.enabled {
        eprintln!("fix: disabled by --no-fix or lint.fix.enabled = false");
        return ExitCode::from(0);
    }

    if debug {
        eprintln!("[debug] fix: outer_ping_max = {}", cfg.outer_ping_max);
    }

    // Run the loop. The fix module handles initial-clean short-circuit
    // internally — no spawn on clean vault.
    let report = match run_fix_loop(paths.root.clone(), cfg.outer_ping_max) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: fix loop: {e}");
            return ExitCode::from(1);
        }
    };

    if debug {
        eprintln!(
            "[debug] fix: invocations = {}, termination = {:?}, errors = {}, warns = {}",
            report.agent_invocations,
            report.termination,
            report.final_lint.error_count,
            report.final_lint.warn_count
        );
    }

    // Initial-clean termination: nothing to commit, exit 0.
    if report.termination == TerminationReason::InitialClean {
        println!("✓ fix: vault already clean, no agent spawned");
        return ExitCode::from(0);
    }

    // Auto-commit any changes (no-op when working tree is clean).
    match auto_commit(&paths.root, "wiki: lint fix loop") {
        Ok(sha) => {
            let sha7: String = sha.chars().take(7).collect();
            if !sha.is_empty() {
                println!("✓ fix: committed {sha7} \"wiki: lint fix loop\"");
            } else {
                println!("✓ fix: no changes to commit");
            }
        }
        Err(e) => {
            eprintln!("error: vault auto-commit: {e}");
            return ExitCode::from(1);
        }
    }

    if report.clean {
        println!(
            "✓ fix complete (vault clean, {} agent invocation(s))",
            report.agent_invocations
        );
        ExitCode::from(0)
    } else {
        eprintln!(
            "✗ fix exhausted ping budget; {} error(s), {} warning(s) remain",
            report.final_lint.error_count, report.final_lint.warn_count
        );
        ExitCode::from(1)
    }
}
