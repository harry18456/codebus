//! `EventsSink` trait + `EventEnvelope` schema.
//!
//! Lifecycle: per-event live append (distinct from `LogSink::write_run`
//! which is per-run summary). Each verb run constructs one sink (via
//! `log::factory::build_events_sink`), calls `write_event` once per
//! `VerbEvent` emission, and drops the sink at run end.

use crate::log::sink::LogError;
use crate::verb::VerbEvent;
use serde::{Deserialize, Serialize};

/// One line in events.jsonl. Captures a wall-clock timestamp at append
/// time plus the originating `VerbEvent` payload.
///
/// Why a separate `ts` from any timestamp inside `event`: events.jsonl
/// is consumed by GUIs / replay tools that need a monotonic wall-clock
/// to render the timeline order, independent of whatever internal
/// timestamps the agent's stream-json may carry. The `ts` is captured
/// in the verb function immediately before `EventsSink::write_event`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// RFC 3339 UTC wall-clock timestamp at envelope creation.
    pub ts: String,
    /// The `VerbEvent` payload (Banner / Stream / Lifecycle).
    pub event: VerbEvent,
}

/// Object-safe sink for `EventEnvelope` writes. `Send + Sync` so a single
/// sink instance can outlive a verb invocation if a future daemon mode
/// keeps one process across multiple runs (mirrors the `LogSink` shape).
pub trait EventsSink: Send + Sync {
    /// Stable identifier (`"null"` / `"jsonl"`).
    fn name(&self) -> &str;

    /// Persist one envelope. Synchronous — caller blocks until the
    /// underlying I/O completes (or fails). Sinks SHOULD flush before
    /// returning so that a crash after `Ok(())` still leaves the row
    /// readable from disk.
    fn write_event(&mut self, envelope: &EventEnvelope) -> Result<(), LogError>;

    /// Flush any internal buffers. Default no-op for buffer-free sinks.
    fn flush(&mut self) -> Result<(), LogError> {
        Ok(())
    }

    /// The on-disk path this sink appends to, when it has one. Default
    /// `None` for path-less sinks (e.g. the null sink). The jsonl sink
    /// returns its resolved `events-<slug>.jsonl` path. Used by the
    /// quiz verb to record an `events_log` pointer in the persisted
    /// quiz frontmatter (v3-app-quiz design D4).
    fn events_path(&self) -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use crate::verb::{Verb, VerbBanner, VerbLifecycleEvent};

    #[test]
    fn envelope_serializes_with_ts_and_event_keys() {
        let env = EventEnvelope {
            ts: "2026-05-13T03:25:11Z".into(),
            event: VerbEvent::Stream(StreamEvent::Thought {
                text: "hi".into(),
            }),
        };
        let json = serde_json::to_string(&env).unwrap();
        assert!(json.contains("\"ts\":\"2026-05-13T03:25:11Z\""));
        assert!(json.contains("\"event\""));
        let parsed: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ts, "2026-05-13T03:25:11Z");
        match parsed.event {
            VerbEvent::Stream(StreamEvent::Thought { text }) => assert_eq!(text, "hi"),
            _ => panic!("unexpected event variant after round-trip"),
        }
    }

    #[test]
    fn envelope_round_trips_for_banner_variant() {
        let env = EventEnvelope {
            ts: "2026-05-13T03:25:11Z".into(),
            event: VerbEvent::Banner(VerbBanner::SyncStart),
        };
        let json = serde_json::to_string(&env).unwrap();
        let parsed: EventEnvelope = serde_json::from_str(&json).unwrap();
        match parsed.event {
            VerbEvent::Banner(VerbBanner::SyncStart) => {}
            _ => panic!("expected Banner(SyncStart)"),
        }
    }

    #[test]
    fn envelope_round_trips_for_lifecycle_variant() {
        let env = EventEnvelope {
            ts: "2026-05-13T03:25:11Z".into(),
            event: VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { verb: Verb::Goal }),
        };
        let json = serde_json::to_string(&env).unwrap();
        let parsed: EventEnvelope = serde_json::from_str(&json).unwrap();
        match parsed.event {
            VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { verb: Verb::Goal }) => {}
            _ => panic!("expected Lifecycle(SpawnStart{{Goal}})"),
        }
    }

    /// Trait object-safety check — if this fn compiles, the trait can be
    /// used as `Box<dyn EventsSink>`.
    #[test]
    fn events_sink_trait_is_object_safe() {
        fn _accept_box(_: Box<dyn EventsSink>) {}
    }
}
