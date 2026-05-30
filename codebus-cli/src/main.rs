//! codebus CLI entry — clap subcommand routing.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use codebus_core::render::RenderOptions;

mod commands;

#[derive(Parser, Debug)]
#[command(
    name = "codebus",
    version,
    about = "Build an AI-curated, Obsidian-compatible markdown wiki for any codebase"
)]
struct Cli {
    /// Path to the source repository. Defaults to current directory for
    /// init/goal/query. For lint/fix, when omitted the verb auto-detects
    /// the vault root from cwd (lint can run from inside `.codebus/`).
    #[arg(long, global = true)]
    repo: Option<PathBuf>,

    /// Skip auto-registering `.codebus/wiki/` as an Obsidian vault. Used by `init`.
    #[arg(long = "no-obsidian-register", global = true)]
    no_obsidian_register: bool,

    /// Also materialize codebus skill bundles at the source repo root
    /// (`<repo>/.claude/skills/codebus-*/`) so a raw Claude Code session
    /// opened at the repo root can invoke `/codebus-<verb>` directly.
    /// Default off: the vault-internal copy alone covers the codebus
    /// binary / GUI spawn paths.
    #[arg(long = "with-repo-root-skills", global = true)]
    with_repo_root_skills: bool,

    /// Skip the lint-and-fix phase for this invocation (overrides `lint.fix.enabled`).
    /// Affects both `goal` (post-agent fix phase) and `fix` (whole flow).
    #[arg(long = "no-fix", global = true)]
    no_fix: bool,

    /// Verbose output: print internal decisions, fs operations, computed signals.
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize a `.codebus/` vault under the target repository.
    Init,
    /// Spawn the codebus-goal agent flow against the vault.
    Goal(commands::goal::GoalArgs),
    /// Spawn the codebus-query agent flow (read-only) against the vault.
    Query(commands::query::QueryArgs),
    /// Run wiki lint and report findings.
    Lint(commands::lint::LintArgs),
    /// Trigger the codebus-fix skill in the user's agentic AI product.
    Fix,
    /// Launch interactive multi-turn read-only chat REPL on the vault.
    Chat(commands::chat::ChatArgs),
    /// Generate a read-only multiple-choice quiz from the vault wiki.
    Quiz(commands::quiz::QuizArgs),
    /// Manage the Azure API key in the OS keyring.
    Config(commands::config::ConfigArgs),
    /// Internal: PreToolUse hook for fix sandbox (called by Claude Code, not users).
    #[command(hide = true, subcommand)]
    Hook(commands::hook::HookArgs),
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let repo_default: PathBuf = cli.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    // v3-render-polish: detect terminal capabilities ONCE at process start.
    // Each verb command receives the snapshot and SHALL NOT re-detect mid-
    // run (per the "Detection runs once per process" scenario). Hook sub-
    // command is internal (Claude Code stdin/stdout JSON contract) and
    // therefore does not consume render options.
    let mut render_opts = RenderOptions::detect();
    // cli-debug-stream-detail: --debug switches the agent-stream renderer to
    // verbose (full tool input / result, no truncation). Default mode keeps
    // the compact rendering. Applies to the agent-spawning verbs that consume
    // this snapshot (goal / query / fix / chat / quiz).
    render_opts.verbose = cli.debug;
    match cli.command {
        None | Some(Command::Init) => {
            commands::init::run(
                &repo_default,
                cli.no_obsidian_register,
                cli.with_repo_root_skills,
                cli.debug,
                &render_opts,
            )
            .await
        }
        Some(Command::Goal(args)) => {
            commands::goal::run(
                &repo_default,
                args,
                cli.no_obsidian_register,
                cli.no_fix,
                cli.debug,
                &render_opts,
            )
            .await
        }
        Some(Command::Query(args)) => {
            commands::query::run(&repo_default, args, cli.debug, &render_opts).await
        }
        Some(Command::Lint(args)) => {
            commands::lint::run(cli.repo.as_deref(), args, cli.debug, &render_opts).await
        }
        Some(Command::Fix) => {
            commands::fix::run(cli.repo.as_deref(), cli.no_fix, cli.debug, &render_opts).await
        }
        Some(Command::Chat(args)) => {
            commands::chat::run(&repo_default, args, cli.debug, &render_opts).await
        }
        Some(Command::Quiz(args)) => {
            commands::quiz::run(&repo_default, args, cli.debug, &render_opts).await
        }
        Some(Command::Config(args)) => commands::config::run(args).await,
        Some(Command::Hook(args)) => commands::hook::run(args).await,
    }
}
