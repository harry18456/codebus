//! `EventsJsonlSink` — live-append events.jsonl writer.
//!
//! File path: `<dir>/events-<slug>.jsonl` where `<slug>` is the run's
//! `started_at` RFC 3339 string with every `:` character replaced by
//! `-` (Windows filename compatibility — `:` is forbidden in NTFS).
//!
//! Audit lens (sharp edges):
//!
//! - Opens with `OpenOptions::append(true).create(true)` — concurrent
//!   writes from different processes are line-wise atomic on POSIX
//!   (one `write()` ≤ PIPE_BUF). On Windows this is best-effort.
//! - Newline is unconditionally appended after the JSON payload, even
//!   if `serde_json` someday emits a trailing newline (extra blank
//!   lines are tolerated by readers).
//! - Parent directory is created lazily on the first write so
//!   misconfigured paths fail loudly at the first event, not at
//!   construction.
//! - BufWriter is flushed per `write_event` call so a process crash
//!   after a successful return still leaves the row readable from
//!   the OS page cache (caller crash-resilience contract).
//! - Slug rule replaces every `:` regardless of position; the only
//!   `:` characters in an RFC 3339 secs-precision timestamp appear
//!   in `HH:MM:SS`, so the result is always a valid Windows filename.

use crate::log::events::sink::{EventEnvelope, EventsSink};
use crate::log::sink::LogError;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub struct EventsJsonlSink {
    dir: PathBuf,
    target_path: PathBuf,
    writer: Option<BufWriter<File>>,
}

impl EventsJsonlSink {
    /// Construct a sink targeting `<dir>/events-<slug>.jsonl` where
    /// `slug` is `started_at` with `:` characters replaced by `-`.
    pub fn new(dir: impl Into<PathBuf>, started_at: &str) -> Self {
        let dir = dir.into();
        let slug = started_at.replace(':', "-");
        let target_path = dir.join(format!("events-{slug}.jsonl"));
        Self {
            dir,
            target_path,
            writer: None,
        }
    }

    /// Exposed for tests so they can inspect the resolved filename.
    #[cfg(test)]
    pub(crate) fn target_path(&self) -> &PathBuf {
        &self.target_path
    }
}

impl EventsSink for EventsJsonlSink {
    fn name(&self) -> &str {
        "jsonl"
    }

    fn events_path(&self) -> Option<PathBuf> {
        Some(self.target_path.clone())
    }

    fn write_event(&mut self, envelope: &EventEnvelope) -> Result<(), LogError> {
        if self.writer.is_none() {
            fs::create_dir_all(&self.dir)?;
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.target_path)?;
            self.writer = Some(BufWriter::new(f));
        }
        let mut line = serde_json::to_string(envelope)?;
        line.push('\n');
        let writer = self.writer.as_mut().expect("writer initialised above");
        writer.write_all(line.as_bytes())?;
        writer.flush()?; // per-write flush for crash resilience
        Ok(())
    }

    fn flush(&mut self) -> Result<(), LogError> {
        if let Some(writer) = self.writer.as_mut() {
            writer.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use crate::verb::{VerbBanner, VerbEvent};
    use tempfile::TempDir;

    fn envelope(text: &str) -> EventEnvelope {
        EventEnvelope {
            ts: "2026-05-13T00:00:00Z".into(),
            event: VerbEvent::Stream(StreamEvent::Thought { text: text.into() }),
        }
    }

    #[test]
    fn slug_replaces_every_colon_with_dash() {
        let tmp = TempDir::new().unwrap();
        let sink = EventsJsonlSink::new(tmp.path(), "2026-05-13T23:55:00Z");
        let name = sink.target_path().file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name, "events-2026-05-13T23-55-00Z.jsonl");
        // Strict invariant: no `:` anywhere in the resulting filename.
        assert!(!name.contains(':'), "filename must not contain colon: {name}");
    }

    #[test]
    fn name_is_stable_string_jsonl() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(
            EventsJsonlSink::new(tmp.path(), "2026-05-13T03:25:11Z").name(),
            "jsonl"
        );
    }

    #[test]
    fn first_write_creates_file_with_one_envelope_line() {
        let tmp = TempDir::new().unwrap();
        let mut sink = EventsJsonlSink::new(tmp.path(), "2026-05-13T03:25:11Z");
        sink.write_event(&envelope("hello")).unwrap();

        let target = tmp.path().join("events-2026-05-13T03-25-11Z.jsonl");
        assert!(target.exists());
        let body = fs::read_to_string(&target).unwrap();
        assert!(body.ends_with('\n'));
        let parsed: serde_json::Value = serde_json::from_str(body.trim_end()).unwrap();
        assert!(parsed.get("ts").is_some());
        assert!(parsed.get("event").is_some());
    }

    #[test]
    fn five_writes_append_five_lines() {
        let tmp = TempDir::new().unwrap();
        let mut sink = EventsJsonlSink::new(tmp.path(), "2026-05-13T03:25:11Z");
        for i in 0..5 {
            sink.write_event(&envelope(&format!("event {i}"))).unwrap();
        }
        let target = tmp.path().join("events-2026-05-13T03-25-11Z.jsonl");
        let body = fs::read_to_string(&target).unwrap();
        assert_eq!(body.lines().count(), 5);
    }

    #[test]
    fn parent_directory_created_lazily_on_first_write() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("nested").join("deep").join("here");
        assert!(!nested.exists());
        let mut sink = EventsJsonlSink::new(&nested, "2026-05-13T00:00:00Z");
        sink.write_event(&envelope("first")).unwrap();
        assert!(nested.exists());
        assert!(nested.join("events-2026-05-13T00-00-00Z.jsonl").exists());
    }

    #[test]
    fn three_variants_all_persist() {
        let tmp = TempDir::new().unwrap();
        let mut sink = EventsJsonlSink::new(tmp.path(), "2026-05-13T00:00:00Z");
        sink.write_event(&EventEnvelope {
            ts: "2026-05-13T00:00:00Z".into(),
            event: VerbEvent::Banner(VerbBanner::SyncStart),
        })
        .unwrap();
        sink.write_event(&envelope("thought")).unwrap();
        sink.write_event(&EventEnvelope {
            ts: "2026-05-13T00:00:01Z".into(),
            event: VerbEvent::Lifecycle(
                crate::verb::VerbLifecycleEvent::FixIterationStart { iteration: 1 },
            ),
        })
        .unwrap();
        let target = tmp.path().join("events-2026-05-13T00-00-00Z.jsonl");
        let body = fs::read_to_string(&target).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 3);
        // event.kind on each line distinguishable
        assert!(lines[0].contains("\"kind\":\"banner\""));
        assert!(lines[1].contains("\"kind\":\"stream\""));
        assert!(lines[2].contains("\"kind\":\"lifecycle\""));
    }
}
