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
    /// Path to the source repository (default: current directory). Used by `init`.
    #[arg(long, default_value = ".", global = true)]
    repo: PathBuf,

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
    /// Trigger the codebus-goal skill in the user's agentic AI product.
    Goal,
    /// Trigger the codebus-query skill in the user's agentic AI product.
    Query,
    /// Run wiki lint and report findings.
    Lint,
    /// Trigger the codebus-fix skill in the user's agentic AI product.
    Fix,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        None | Some(Command::Init) => {
            commands::init::run(&cli.repo, cli.no_obsidian_register, cli.debug).await
        }
        Some(Command::Goal) => commands::goal::run().await,
        Some(Command::Query) => commands::query::run().await,
        Some(Command::Lint) => commands::lint::run().await,
        Some(Command::Fix) => commands::fix::run().await,
    }
}
