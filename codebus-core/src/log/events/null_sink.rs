//! `EventsNullSink` — opt-out path for `log.sink: none` config.
//!
//! All operations are no-ops (no filesystem state changes, no I/O).
//! Used when the user explicitly disables logging via the shared
//! `log.sink` discriminator — `EventsNullSink` and the existing
//! `NullSink` (for runs.jsonl) opt out together so CLI users get a
//! single user-visible knob covering all logging.

use crate::log::events::sink::{EventEnvelope, EventsSink};
use crate::log::sink::LogError;

pub struct EventsNullSink;

impl EventsNullSink {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EventsNullSink {
    fn default() -> Self {
        Self::new()
    }
}

impl EventsSink for EventsNullSink {
    fn name(&self) -> &str {
        "null"
    }

    fn write_event(&mut self, _envelope: &EventEnvelope) -> Result<(), LogError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use crate::verb::VerbEvent;
    use tempfile::TempDir;

    fn envelope(text: &str) -> EventEnvelope {
        EventEnvelope {
            ts: "2026-05-13T00:00:00Z".into(),
            event: VerbEvent::Stream(StreamEvent::Thought { text: text.into() }),
        }
    }

    #[test]
    fn name_is_stable_string_null() {
        assert_eq!(EventsNullSink::new().name(), "null");
    }

    #[test]
    fn write_event_returns_ok_without_io() {
        let tmp = TempDir::new().unwrap();
        let before: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        let mut sink = EventsNullSink::new();
        for i in 0..5 {
            sink.write_event(&envelope(&format!("event {i}"))).unwrap();
        }
        // No filesystem state changed.
        let after: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        assert_eq!(before.len(), after.len());
    }
}
