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
    ClaudeCodeConfig, LintFixConfig, default_config_path, load_claude_code_config,
    load_lint_fix_config,
};
use codebus_core::git::auto_commit;
use codebus_core::vault::layout::vault_paths;
use codebus_core::wiki::fix::{TerminationReason, run_fix_loop};

pub async fn run(
    repo_override: Option<&Path>,
    no_fix: bool,
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
        eprintln!("[debug] fix: --no-fix = {no_fix}");
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
    let cfg = cfg.merge_cli_overrides(no_fix);

    if !cfg.enabled {
        eprintln!("fix: disabled by --no-fix or lint.fix.enabled = false");
        return ExitCode::from(0);
    }

    // Load claude_code.fix config so the spawned agent gets the configured
    // model/effort flags. Failure → defaults (sonnet/medium) per spec.
    let cc_cfg = match default_config_path() {
        Some(p) => match load_claude_code_config(&p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("warning: claude_code config load failed (using defaults): {e}");
                ClaudeCodeConfig::default()
            }
        },
        None => ClaudeCodeConfig::default(),
    };

    // Run the single-shot flow. The fix module handles initial-clean
    // short-circuit internally — no spawn on clean vault.
    let report = match run_fix_loop(
        paths.root.clone(),
        cc_cfg.fix.model,
        cc_cfg.fix.effort,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: fix loop: {e}");
            return ExitCode::from(1);
        }
    };

    if debug {
        eprintln!(
            "[debug] fix: agent_skipped = {}, termination = {:?}, errors = {}, warns = {}",
            report.agent_skipped,
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
        println!("✓ fix complete (vault clean)");
        ExitCode::from(0)
    } else {
        eprintln!(
            "✗ fix: {} error(s), {} warning(s) remain after agent terminated",
            report.final_lint.error_count, report.final_lint.warn_count
        );
        ExitCode::from(1)
    }
}
