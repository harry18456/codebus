//! Claude CLI stream-json parser. v3-run-log carry from v2's
//! the v2 implementation.

pub mod codex_parser;
pub mod parser;
pub mod sandbox_signal;

pub use codex_parser::{parse_codex_stream_line, sniff_codex_thread_id};
pub use parser::{StreamEvent, parse_claude_stream_line};
pub use sandbox_signal::{classify_stderr_lines, is_sandbox_denial};
