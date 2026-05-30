//! `codebus fix` — CLI thin wrapper over `codebus_core::verb::fix::run_fix`.
//!
//! Vault precondition / lint pre-check / agent spawn / fix loop /
//! final lint / auto-commit / run-log persistence all live in
//! `codebus_core::verb::fix`. This file is the clap surface plus the
//! `VerbEvent` dispatch closure plus the `VerbError` / `FixStatus`
//! translation to CLI exit code and stderr.

use std::path::Path;
use std::process::ExitCode;

use codebus_core::config::ConfigLoadError;
use codebus_core::render::{RenderOptions, print_banner, print_event};
use codebus_core::verb::fix::{FixOptions, FixStatus, run_fix};
use codebus_core::verb::{VerbError, VerbEvent};

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

    if debug {
        eprintln!("[debug] fix: repo = {}", repo.display());
        eprintln!("[debug] fix: --no-fix = {no_fix}");
    }

    let options = FixOptions { no_fix };

    let render_opts_for_closure = render_opts.clone();
    let on_event = move |event: VerbEvent| match event {
        VerbEvent::Banner(b) => print_banner(b.as_banner(), &render_opts_for_closure),
        VerbEvent::Stream(s) => print_event(&s, &render_opts_for_closure),
        VerbEvent::Lifecycle(_) => {}
    };

    let timeout = super::resolve_run_timeout(debug);
    match run_fix(&repo, options, on_event, None, timeout) {
        Ok(report) => match report.status {
            FixStatus::Skipped { reason: _ } => {
                eprintln!("fix: disabled by --no-fix or lint.fix.enabled = false");
                ExitCode::from(0)
            }
            FixStatus::InitialClean => {
                if debug {
                    println!("✓ fix: vault already clean, no agent spawned");
                }
                ExitCode::from(0)
            }
            FixStatus::PostLintClean => {
                if debug {
                    println!("✓ fix complete (vault clean)");
                }
                ExitCode::from(0)
            }
            FixStatus::PostLintIssuesRemain => {
                eprintln!(
                    "✗ fix: {} error(s), {} warning(s) remain after agent terminated",
                    report.final_lint_error_count, report.final_lint_warn_count
                );
                ExitCode::from(1)
            }
        },
        Err(e) => translate_error(&e),
    }
}

fn translate_error(err: &VerbError) -> ExitCode {
    match err {
        VerbError::VaultMissing { path } => {
            eprintln!(
                "error: no codebus vault at {} — run `codebus init` first",
                path.display()
            );
            ExitCode::from(2)
        }
        VerbError::ConfigParse { which, source } => {
            let path_disp = codebus_core::config::default_config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            match source {
                ConfigLoadError::Io(_) | ConfigLoadError::YamlParse(_) => {
                    eprintln!("error: {which} config parse failed at {path_disp}: {source}");
                }
            }
            ExitCode::from(2)
        }
        VerbError::KeyringMissing { source } => {
            eprintln!("error: fix: {source}");
            ExitCode::from(3)
        }
        VerbError::Spawn { source } => {
            // fix.rs pre-refactor wrapped invoke spawn failures inside the
            // fix loop as "fix loop: …" — match that.
            eprintln!("error: fix loop: spawn fix agent: {source}");
            ExitCode::from(1)
        }
        VerbError::Cancelled => ExitCode::from(0),
        VerbError::AgentFailed { exit_code } => {
            // Defensive arm: run_fix SHALL NOT emit AgentFailed (per spec
            // verb-library §Verb Error Enum — one-shot verbs propagate
            // child exit via Ok(report).agent_exit_code). Generic fallback
            // is used instead of unreachable!() so a future regression that
            // emits AgentFailed on this path does NOT panic the binary.
            match exit_code {
                Some(code) => eprintln!("error: fix: agent exited with code {code}"),
                None => eprintln!("error: fix: agent exited without a recorded exit code"),
            }
            ExitCode::from(err.cli_exit_code())
        }
        VerbError::Internal { message } => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
