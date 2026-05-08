//! Concrete `LogSink` implementations. `null_sink` is the day-one default;
//! `jsonl_sink` is the always-available file logger. Heavy-dep sinks
//! (`otel`) land here behind cargo feature gates in follow-up changes.

pub mod jsonl_sink;
pub mod null_sink;
