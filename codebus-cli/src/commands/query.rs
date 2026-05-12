//! `codebus query "..."` — read-only spawn against the active vault.
//! Agent reads `wiki/`, follows wikilinks, prints an answer to stdout.
//! No Write/Edit (binary `--tools` hard gate enforces; SKILL.md restates).
//! No auto-init / no source-signal detection / no auto_commit — query is a
//! wiki user, not an ingest producer. See openspec/changes/v3-query/.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::agent::{EnvOverrides, InvokeAgentOptions, invoke};
use codebus_core::config::{Verb, build_env_overrides};
use codebus_core::config::{
    ClaudeCodeConfig, default_config_path, load_claude_code_config,
};
use codebus_core::log::RunLog;
use codebus_core::render::{Banner, RenderOptions, print_banner};
use codebus_core::vault::layout::vault_paths;

use crate::run_log::{
    load_log_config_with_warning, resolve_sink_dir, write_run_log,
};

/// Read-only toolset for the query verb. Excludes Write/Edit/Bash. v2
/// iter-9 carry, spike-verified 2026-05-09 to be a real hard gate.
const QUERY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

#[derive(Args, Debug)]
pub struct QueryArgs {
    /// What you want to know about the codebase.
    #[arg(value_name = "QUERY")]
    pub text: String,
}

pub async fn run(
    repo: &Path,
    args: QueryArgs,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    let paths = vault_paths(repo);

    if debug {
        eprintln!(
            "[debug] query: repo={}, vault={}",
            repo.display(),
            paths.root.display()
        );
    }

    // Banner: 駛入 — query has no Done banner because it doesn't write the
    // wiki ("下車" implies cargo delivered; query just rides the bus).
    print_banner(Banner::Start { repo_path: repo }, render_opts);

    // Step 2: vault precondition — strict refuse, no auto-init fallback.
    // Query is a wiki user, not an ingest producer; missing vault is a
    // user input error, not a trigger to mutate state.
    if !paths.root.exists() {
        eprintln!(
            "error: query: vault not found at {}; run `codebus init` first",
            paths.root.display()
        );
        return ExitCode::from(2);
    }

    // Step 3: spawn agent with read-only triple-flag sandbox.
    let cc_cfg = match load_claude_code_config_with_warning() {
        Ok(c) => c,
        Err(code) => return code,
    };
    let query_resolved = cc_cfg.resolve(Verb::Query);
    let slash_command = format!("/codebus-query \"{}\"", args.text);
    if debug {
        eprintln!(
            "[debug] query: spawn claude with cwd={} slash={:?} toolset={:?} model={:?} effort={:?}",
            paths.root.display(),
            slash_command,
            QUERY_TOOLSET,
            query_resolved.model,
            query_resolved.effort,
        );
    }
    // Profile-aware env injection. Azure profile + key missing → exit
    // non-zero before spawning. See spec: Scoped Environment Injection
    // At Spawn / OS Keyring Integration With Env Fallback.
    let query_env = match build_env_overrides(&cc_cfg) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("error: query: {e}");
            return ExitCode::from(3);
        }
    };
    let _ = EnvOverrides::for_system; // suppress unused-import warning
    let invoke_report = match invoke(
        InvokeAgentOptions {
            slash_command,
            vault_root: paths.root.clone(),
            toolset: QUERY_TOOLSET,
            bash_whitelist: None,
            model: query_resolved.model.clone(),
            effort: query_resolved.effort.clone(),
            env: query_env,
        },
        render_opts,
    ) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("error: spawn claude: {e}");
            return ExitCode::from(1);
        }
    };

    // Step 4: propagate child exit code. NO auto_commit (read-only).
    let child_exit_code: u8 = invoke_report
        .exit
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1);
    if debug {
        eprintln!(
            "[debug] query: agent exited code={}, success={}",
            invoke_report.exit.code().unwrap_or(-1),
            invoke_report.exit.success()
        );
    }

    // v3-run-log: persist a RunLog entry. Query is read-only so wiki_changed
    // is always false and lint counts are 0; tokens come from the agent.
    let run_log = RunLog {
        goal: args.text.clone(),
        mode: "query".into(),
        model: query_resolved.model.clone(),
        effort: query_resolved.effort.clone(),
        started_at: invoke_report.started_at,
        finished_at: invoke_report.finished_at,
        tokens: invoke_report.accumulated_tokens,
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
    };
    let log_cfg = match load_log_config_with_warning() {
        Ok(c) => c,
        Err(code) => return code,
    };
    let sink_cfg = resolve_sink_dir(log_cfg, &paths.log);
    write_run_log(sink_cfg, &run_log);

    ExitCode::from(child_exit_code)
}

/// Load `claude_code.*` config. Returns `Default::default()` when the
/// config file does not exist (first-time setup). Returns `Err(ExitCode)`
/// when the file exists but fails to parse — caller SHALL propagate the
/// exit code without spawning the agent (spec: cli / `Config Parse
/// Failure Aborts Invocation`).
fn load_claude_code_config_with_warning() -> Result<ClaudeCodeConfig, ExitCode> {
    let path = match default_config_path() {
        Some(p) => p,
        None => return Ok(ClaudeCodeConfig::default()),
    };
    match load_claude_code_config(&path) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            eprintln!(
                "error: claude_code config parse failed at {}: {e}",
                path.display()
            );
            Err(ExitCode::from(2))
        }
    }
}
