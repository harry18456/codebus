//! Run-log sink plugin domain.
//!
//! Day-one wiring lands [`sink::LogSink`] trait + [`factory::build_sink`] +
//! [`sinks::null_sink::NullSink`] (default, behavior-neutral with 0.2.0) +
//! [`sinks::jsonl_sink::JsonlSink`] (always available, deps already in
//! tree). `goal` / `query` flows accept `&mut dyn LogSink` but pass
//! `NullSink` by default — wiring up real run summaries is a follow-up
//! change.

pub mod factory;
pub mod sink;
pub mod sinks;

pub use factory::{SinkConfig, SinkError, build_sink};
pub use sink::{LogError, LogSink, RunLog, TokenUsage};
