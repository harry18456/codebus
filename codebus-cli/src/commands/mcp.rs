//! `codebus mcp [--vault <path>]` — start the stdio MCP server that exposes
//! codebus vault wikis (`<vault>/.codebus/wiki/`) to external MCP clients as
//! query-only tools.
//!
//! Two startup modes:
//! - `codebus mcp --vault <path>` — **pinned** to one vault, fixed at startup
//!   (backward-compatible with v1).
//! - `codebus mcp` (no `--vault`) — **registry** mode: serves every vault in
//!   `~/.codebus/app-state.json`, re-read on each request (read-only). The wiki
//!   tools take an optional `vault` selector.
//!
//! The server only ever reads under each vault's wiki subtree — `raw/code/`
//! (the PII-redacted mirror) is never served — and never writes the registry.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

use crate::mcp::ServeMode;

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Path to a single repository whose `.codebus/wiki/` is served (pinned
    /// mode). Omit to serve every vault registered in the app-state registry
    /// (`~/.codebus/app-state.json`).
    #[arg(long)]
    pub vault: Option<PathBuf>,
}

pub async fn run(args: McpArgs) -> ExitCode {
    let mode = match args.vault {
        Some(vault) => {
            let wiki_root = vault.join(".codebus").join("wiki");
            if !wiki_root.is_dir() {
                eprintln!(
                    "error: mcp: no wiki at {} — run `codebus init` / `codebus goal` against this repo first",
                    wiki_root.display()
                );
                return ExitCode::from(2);
            }
            let name = vault
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("vault")
                .to_string();
            // All server diagnostics go to stderr; stdout is the JSON-RPC channel.
            eprintln!("codebus mcp: pinned to wiki at {}", wiki_root.display());
            ServeMode::Pinned {
                vault,
                name,
                wiki_root,
            }
        }
        None => {
            // Registry mode: no startup wiki check — vaults are resolved (and
            // the registry re-read) per request. Print a one-line count for
            // operator feedback; this read is read-only.
            let count = crate::mcp::registry::list_entries(&ServeMode::Registry).len();
            eprintln!(
                "codebus mcp: registry mode — {count} vault(s) from ~/.codebus/app-state.json (re-read per request)"
            );
            ServeMode::Registry
        }
    };

    match crate::mcp::serve(mode).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: mcp server: {e}");
            ExitCode::FAILURE
        }
    }
}
