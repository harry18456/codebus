//! `codebus mcp --vault <path>` — start the stdio MCP server that exposes one
//! vault's wiki (`<vault>/.codebus/wiki/`) to external MCP clients as
//! query-only tools. Single-vault: the wiki root is pinned here at startup and
//! no tool accepts a path. The server only ever reads under the wiki subtree —
//! `raw/code/` (the PII-redacted mirror) is never served.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

#[derive(Args, Debug)]
pub struct McpArgs {
    /// Path to the repository whose `.codebus/wiki/` will be served over MCP.
    #[arg(long)]
    pub vault: PathBuf,
}

pub async fn run(args: McpArgs) -> ExitCode {
    let wiki_root = args.vault.join(".codebus").join("wiki");
    if !wiki_root.is_dir() {
        eprintln!(
            "error: mcp: no wiki at {} — run `codebus init` / `codebus goal` against this repo first",
            wiki_root.display()
        );
        return ExitCode::from(2);
    }
    // All server diagnostics go to stderr; stdout is the JSON-RPC channel.
    eprintln!("codebus mcp: serving wiki at {}", wiki_root.display());
    match crate::mcp::serve(wiki_root).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: mcp server: {e}");
            ExitCode::FAILURE
        }
    }
}
