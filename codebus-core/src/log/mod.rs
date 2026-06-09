//! Run-log subsystem — RunLog struct, LogSink trait, and concrete sinks.
//!
//! v3-run-log carry from the v2 implementation. The TokenUsage
//! shape is provider-agnostic (Anthropic-shaped now; OpenAI/Ollama/Gemini will
//! map onto the same fields via per-provider stream parsers). The LogSink
//! trait is object-safe so callers can pass `&mut dyn LogSink` and swap impls
//! based on `~/.codebus/config.yaml log.sink` discriminator.

pub mod events;
pub mod factory;
pub mod sink;
pub mod sinks;
pub mod verb_log;

pub use events::{EventEnvelope, EventsJsonlSink, EventsNullSink, EventsSink};
pub use factory::{SinkConfig, SinkError, build_events_sink, build_sink};
pub use sink::{
    InterruptReason, LogError, LogSink, RunLog, TokenUsage, TokenUsageSemantics,
    accumulate_token_usage, apply_token_usage,
};
pub use verb_log::{
    load_verb_log_config, resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};
