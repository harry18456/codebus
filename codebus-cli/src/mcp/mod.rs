//! `codebus mcp` — stdio MCP server exposing one vault's wiki as query-only
//! tools (`wiki_list` / `wiki_read` / `wiki_search`). Single-vault: the wiki
//! root is pinned at server construction; no tool accepts a path argument.
//!
//! Gated behind the default-on `mcp` cargo feature so a `--no-default-features`
//! build drops rmcp from the binary.

pub mod server;
pub mod tools;

pub use server::serve;
