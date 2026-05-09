//! codebus CLI entry — clap subcommand routing.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let repo_default: PathBuf = cli.repo.clone().unwrap_or_else(|| PathBuf::from("."));
    match cli.command {
        None | Some(Command::Init) => {
            commands::init::run(&repo_default, cli.no_obsidian_register, cli.debug).await
        }
        Some(Command::Goal(args)) => {
            commands::goal::run(&repo_default, args, cli.no_obsidian_register, cli.debug).await
        }
        Some(Command::Query(args)) => commands::query::run(&repo_default, args, cli.debug).await,
        Some(Command::Lint(args)) => commands::lint::run(cli.repo.as_deref(), args, cli.debug).await,
        Some(Command::Fix) => commands::fix::run().await,
    }
}
