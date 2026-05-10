//! Run-log subsystem — RunLog struct, LogSink trait, and concrete sinks.
//!
//! v3-run-log carry from `legacy/v2-rust/codebus-core/src/log/`. The TokenUsage
//! shape is provider-agnostic (Anthropic-shaped now; OpenAI/Ollama/Gemini will
//! map onto the same fields via per-provider stream parsers). The LogSink
//! trait is object-safe so callers can pass `&mut dyn LogSink` and swap impls
//! based on `~/.codebus/config.yaml log.sink` discriminator.

pub mod factory;
pub mod sink;
pub mod sinks;

pub use factory::{SinkConfig, SinkError, build_sink};
pub use sink::{LogError, LogSink, RunLog, TokenUsage, accumulate_token_usage};
