//! codebus CLI entry — clap subcommand routing only.
//!
//! Verbs are registered here but their bodies are stubs in v3-workspace.
//! Each subsequent change populates one verb (see `docs/v3-roadmap.md` §4).

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
        None | Some(Command::Init) => commands::init::run().await,
        Some(Command::Goal) => commands::goal::run().await,
        Some(Command::Query) => commands::query::run().await,
        Some(Command::Lint) => commands::lint::run().await,
        Some(Command::Fix) => commands::fix::run().await,
    }
}
