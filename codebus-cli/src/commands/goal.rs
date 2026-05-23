//! `codebus goal "..."` — CLI thin wrapper over
//! `codebus_core::verb::goal::run_goal`.
//!
//! All orchestration (auto-init / drift detection / re-sync / agent
//! spawn / fix loop / auto-commit / RunLog) lives in
//! `codebus_core::verb::goal`. This file is the clap surface plus the
//! `VerbEvent` dispatch closure plus the `VerbError` translation table
//! plus the exit-code precedence rule (agent exit > fix exit).

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::config::{ConfigLoadError, default_config_path, load_goal_config};
use codebus_core::render::{RenderOptions, print_banner, print_event};
use codebus_core::verb::goal::{GoalOptions, run_goal};
use codebus_core::verb::{VerbError, VerbEvent};

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
    render_opts: &RenderOptions,
) -> ExitCode {
    if debug {
        eprintln!(
            "[debug] goal: repo={}, force_resync={}",
            repo.display(),
            args.force_resync
        );
    }

    // goal-content-verify D6: resolve `goal.content_verify` from the
    // shared `goal.*` config (never the app-only `app.*` namespace). A
    // load error conservatively defaults to `false` — do NOT silently
    // enable extra verify/repair spawns on a malformed config. No new
    // subcommand: this is an internal stage of `run_goal`. The
    // originating goal text is already threaded via `text` (the off-goal
    // check needs it).
    let content_verify = match default_config_path() {
        Some(p) => match load_goal_config(&p) {
            Ok(cfg) => cfg.content_verify,
            Err(e) => {
                if debug {
                    eprintln!("[debug] goal: goal config load failed, content_verify=false: {e}");
                }
                false
            }
        },
        None => false,
    };

    let options = GoalOptions {
        text: args.text.clone(),
        force_resync: args.force_resync,
        no_fix,
        no_obsidian_register,
        content_verify,
    };

    let render_opts_for_closure = render_opts.clone();
    let on_event = move |event: VerbEvent| match event {
        VerbEvent::Banner(b) => print_banner(b.as_banner(), &render_opts_for_closure),
        VerbEvent::Stream(s) => print_event(&s, &render_opts_for_closure),
        VerbEvent::Lifecycle(_) => {}
    };

    match run_goal(repo, options, on_event, None) {
        Ok(report) => {
            // Match pre-refactor stderr: when the fix loop terminated
            // with issues remaining, emit the same warning line.
            if report.fix_post_lint_issues_remain {
                eprintln!(
                    "✗ fix: {} error(s), {} warning(s) remain after agent terminated",
                    report.lint_error_count, report.lint_warn_count
                );
            }
            // Exit code precedence: goal agent failure preempts fix exit.
            let child_exit: u8 = report
                .agent_exit_code
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(0);
            if child_exit != 0 {
                ExitCode::from(child_exit)
            } else if report.fix_post_lint_issues_remain {
                ExitCode::from(1)
            } else {
                ExitCode::from(0)
            }
        }
        Err(e) => translate_error(&e),
    }
}

fn translate_error(err: &VerbError) -> ExitCode {
    match err {
        VerbError::VaultMissing { path } => {
            // Goal auto-inits when vault is missing, so this variant
            // surfaces only if auto-init itself failed to create the
            // vault. Mirror the pre-refactor stderr line.
            eprintln!(
                "error: goal: auto-init did not create vault at {}",
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
            eprintln!("error: goal: {source}");
            ExitCode::from(3)
        }
        VerbError::Spawn { source } => {
            eprintln!("error: spawn claude: {source}");
            ExitCode::from(1)
        }
        VerbError::Cancelled => ExitCode::from(0),
        VerbError::AgentFailed { exit_code } => {
            // Defensive arm: run_goal SHALL NOT emit AgentFailed (per spec
            // verb-library §Verb Error Enum — one-shot verbs propagate
            // child exit via Ok(GoalReport).agent_exit_code). Generic
            // fallback is used instead of unreachable!() so a future
            // regression does NOT panic the binary.
            match exit_code {
                Some(code) => eprintln!("error: goal: agent exited with code {code}"),
                None => eprintln!("error: goal: agent exited without a recorded exit code"),
            }
            ExitCode::from(err.cli_exit_code())
        }
        VerbError::Internal { message } => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
