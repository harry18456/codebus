//! Claude CLI stream-json parser. v3-run-log carry from v2's
//! `legacy/v2-rust/codebus-core/src/stream/parser.rs`.

pub mod parser;

pub use parser::{StreamEvent, parse_claude_stream_line};
