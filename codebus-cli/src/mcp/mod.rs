//! `codebus mcp` — stdio MCP server exposing codebus vault wikis as query-only
//! tools (`vault_list` / `wiki_list` / `wiki_read` / `wiki_search`).
//!
//! Two startup modes (see [`registry::ServeMode`]):
//! - **Registry** (`codebus mcp`): serves every vault registered in
//!   `~/.codebus/app-state.json`, read on each call; the wiki tools take an
//!   optional `vault` selector and aggregate across vaults when it is omitted.
//! - **Pinned** (`codebus mcp --vault <path>`): one vault fixed at startup
//!   (backward-compatible with v1).
//!
//! Gated behind the default-on `mcp` cargo feature so a `--no-default-features`
//! build drops rmcp from the binary.

pub mod registry;
pub mod server;
pub mod tools;

pub use registry::ServeMode;
pub use server::serve;
