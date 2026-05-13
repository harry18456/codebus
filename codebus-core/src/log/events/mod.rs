//! events.jsonl persistence — per-event live append, parallel to the
//! existing `LogSink` (per-run summary) trait.
//!
//! Two impls land here: `EventsJsonlSink` (production; writes one
//! `EventEnvelope` JSON line per event with per-write flush for crash
//! resilience) and `EventsNullSink` (opt-out; no-op for `log.sink: none`
//! config).
//!
//! Lifecycle / consumers:
//!
//! - `verb::{goal, query, fix}::run_*` build an `EventsSink` per run via
//!   `log::factory::build_events_sink` and fan out each `VerbEvent` to
//!   (a) the caller's `on_event` closure AND (b) `EventsSink::write_event`.
//! - GUI (post v3-app-workspace-goal) tails the events.jsonl file to
//!   reconstruct the goal detail view's Stream history + lifecycle
//!   timeline.

pub mod jsonl_sink;
pub mod null_sink;
pub mod sink;

pub use jsonl_sink::EventsJsonlSink;
pub use null_sink::EventsNullSink;
pub use sink::{EventEnvelope, EventsSink};
