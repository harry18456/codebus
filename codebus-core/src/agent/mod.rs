//! Agent CLI invocation surface.
//!
//! v3-goal #5 lands a single submodule [`claude_cli`] with one public function
//! [`invoke`](claude_cli::invoke) that spawns `claude -p` with the v2 iter-9
//! triple-flag sandbox (`--tools` + `--allowedTools` + `--permission-mode
//! acceptEdits`). No trait, no provider abstraction — second-impl
//! (codex / gemini / etc.) lands in a future change once the second CLI is
//! actually validated, not speculatively now.

pub mod claude_cli;

pub use claude_cli::{InvokeAgentOptions, invoke};
