//! `codebus query "..."` — read-only spawn against the active vault.
//! Agent reads `wiki/`, follows wikilinks, prints an answer to stdout.
//! No Write/Edit (binary `--tools` hard gate enforces; SKILL.md restates).
//! No auto-init / no source-signal detection / no auto_commit — query is a
//! wiki user, not an ingest producer. See openspec/changes/v3-query/.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::agent::{InvokeAgentOptions, invoke};
use codebus_core::config::{
    ClaudeCodeConfig, default_config_path, load_claude_code_config,
};
use codebus_core::vault::layout::vault_paths;

/// Read-only toolset for the query verb. Excludes Write/Edit/Bash. v2
/// iter-9 carry, spike-verified 2026-05-09 to be a real hard gate.
const QUERY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

#[derive(Args, Debug)]
pub struct QueryArgs {
    /// What you want to know about the codebase.
    #[arg(value_name = "QUERY")]
    pub text: String,
}

pub async fn run(repo: &Path, args: QueryArgs, debug: bool) -> ExitCode {
    let paths = vault_paths(repo);

    if debug {
        eprintln!(
            "[debug] query: repo={}, vault={}",
            repo.display(),
            paths.root.display()
        );
    }

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
    let cc_cfg = load_claude_code_config_with_warning();
    let slash_command = format!("/codebus-query \"{}\"", args.text);
    if debug {
        eprintln!(
            "[debug] query: spawn claude with cwd={} slash={:?} toolset={:?} model={:?} effort={:?}",
            paths.root.display(),
            slash_command,
            QUERY_TOOLSET,
            cc_cfg.query.model,
            cc_cfg.query.effort,
        );
    }
    let child_status = match invoke(InvokeAgentOptions {
        slash_command,
        vault_root: paths.root.clone(),
        toolset: QUERY_TOOLSET,
        bash_whitelist: None,
        model: cc_cfg.query.model,
        effort: cc_cfg.query.effort,
    }) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("error: spawn claude: {e}");
            return ExitCode::from(1);
        }
    };

    // Step 4: propagate child exit code. NO auto_commit (read-only).
    let child_exit_code: u8 = child_status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1);
    if debug {
        eprintln!(
            "[debug] query: agent exited code={}, success={}",
            child_status.code().unwrap_or(-1),
            child_status.success()
        );
    }

    ExitCode::from(child_exit_code)
}

/// Load `claude_code.*` config with stderr warning + default fallback on
/// parse failure. Same shape as init.rs / goal.rs / fix.rs PII config helper.
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
