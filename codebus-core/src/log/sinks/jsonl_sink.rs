//! `jsonl` sink — append one JSON line per [`RunLog`] to a date-rotated
//! file under a configurable directory.
//!
//! File path: `<dir>/runs-YYYY-MM-DD.jsonl` where `YYYY-MM-DD` is the first
//! 10 characters of `entry.started_at` (RFC 3339 prefix). A run that starts
//! at 23:55 UTC and finishes at 00:10 UTC the next day still lands in the
//! older date's file — selection pins on `started_at` (not write-time
//! clock, not `finished_at`) so a long-running invocation never accidentally
//! splits its log line into a different file from where it was attributed.
//!
//! Audit lens (sharp edges):
//!   - Opens with `OpenOptions::append(true).create(true)` — concurrent
//!     writes from different processes are line-wise atomic on POSIX (one
//!     `write()` ≤ PIPE_BUF). On Windows this is best-effort; treat it as
//!     append-mostly-safe.
//!   - Newline is unconditionally appended after the JSON payload — even
//!     if `serde_json` someday emits a trailing newline, we still get
//!     valid jsonl (extra blank lines are tolerated by readers).
//!   - Parent directory is created lazily on first write so misconfigured
//!     paths fail loudly at the first run, not at construction.
//!   - Date extraction: takes the first 10 chars of the RFC 3339 string.
//!     `RunLog::started_at` is constructed by codebus from `chrono::Utc`
//!     so the format is guaranteed `YYYY-MM-DDTHH:MM:SS[.fff]Z`. If a
//!     malformed string is somehow passed, the slice still yields a
//!     non-empty fragment used as a filename suffix — degraded but not
//!     crashing.

use crate::log::sink::{LogError, LogSink, RunLog};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub struct JsonlSink {
    dir: PathBuf,
}

impl JsonlSink {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `<dir>/runs-YYYY-MM-DD.jsonl`. Date taken from `entry.started_at`
    /// so a run is always attributed to the day it began.
    fn target_path_for(&self, entry: &RunLog) -> PathBuf {
        let date_part = if entry.started_at.len() >= 10 {
            &entry.started_at[..10]
        } else {
            entry.started_at.as_str()
        };
        self.dir.join(format!("runs-{date_part}.jsonl"))
    }
}

impl LogSink for JsonlSink {
    fn name(&self) -> &str {
        "jsonl"
    }

    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError> {
        fs::create_dir_all(&self.dir)?;
        let mut line = serde_json::to_string(entry)?;
        line.push('\n');
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.target_path_for(entry))?;
        f.write_all(line.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::sink::TokenUsage;
    use tempfile::TempDir;

    fn run(date_iso: &str) -> RunLog {
        RunLog {
            goal: "x".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: date_iso.to_string(),
            finished_at: date_iso.to_string(),
            tokens: TokenUsage {
                input_tokens: 1,
                output_tokens: 2,
                ..Default::default()
            },
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "succeeded".into(),
            session_id: None,
        }
    }

    #[test]
    fn name_is_stable_string_jsonl() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(JsonlSink::new(tmp.path()).name(), "jsonl");
    }

    #[test]
    fn first_write_creates_file_with_one_json_line_terminated_by_newline() {
        let tmp = TempDir::new().unwrap();
        let mut sink = JsonlSink::new(tmp.path());
        sink.write_run(&run("2026-05-10T03:00:00Z")).unwrap();

        let target = tmp.path().join("runs-2026-05-10.jsonl");
        assert!(target.exists());
        let body = fs::read_to_string(&target).unwrap();
        assert!(body.ends_with('\n'));
        // Strip trailing newline and parse the remainder as one JSON object.
        let parsed: serde_json::Value = serde_json::from_str(body.trim_end()).unwrap();
        assert_eq!(parsed["mode"], "goal");
    }

    #[test]
    fn second_write_appends_second_line_to_same_file_when_same_date() {
        let tmp = TempDir::new().unwrap();
        let mut sink = JsonlSink::new(tmp.path());
        sink.write_run(&run("2026-05-10T01:00:00Z")).unwrap();
        sink.write_run(&run("2026-05-10T23:50:00Z")).unwrap();

        let target = tmp.path().join("runs-2026-05-10.jsonl");
        let body = fs::read_to_string(&target).unwrap();
        let line_count = body.lines().count();
        assert_eq!(line_count, 2, "expected 2 lines, got body: {body:?}");
    }

    /// Spec scenario: "JsonlSink date rotation by started_at" — split occurs
    /// at the date boundary, not at the write-time clock.
    #[test]
    fn date_rotation_splits_by_started_at_not_wall_clock() {
        let tmp = TempDir::new().unwrap();
        let mut sink = JsonlSink::new(tmp.path());
        sink.write_run(&run("2026-05-10T23:55:00Z")).unwrap();
        sink.write_run(&run("2026-05-11T00:05:00Z")).unwrap();

        assert!(tmp.path().join("runs-2026-05-10.jsonl").exists());
        assert!(tmp.path().join("runs-2026-05-11.jsonl").exists());
        assert_eq!(
            fs::read_to_string(tmp.path().join("runs-2026-05-10.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert_eq!(
            fs::read_to_string(tmp.path().join("runs-2026-05-11.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }

    /// Spec scenario: "JsonlSink creates directory lazily on first write".
    #[test]
    fn parent_directory_created_lazily_on_first_write() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("nested").join("dirs").join("here");
        assert!(!nested.exists());
        let mut sink = JsonlSink::new(&nested);
        sink.write_run(&run("2026-05-10T00:00:00Z")).unwrap();
        assert!(nested.exists());
        assert!(nested.join("runs-2026-05-10.jsonl").exists());
    }

    #[test]
    fn malformed_started_at_does_not_panic_uses_truncated_filename() {
        let tmp = TempDir::new().unwrap();
        let mut sink = JsonlSink::new(tmp.path());
        let mut entry = run("short");
        entry.started_at = "short".into();
        // Should not panic even with non-RFC-3339 string; uses literal as
        // filename suffix.
        sink.write_run(&entry).unwrap();
        assert!(tmp.path().join("runs-short.jsonl").exists());
    }
}
