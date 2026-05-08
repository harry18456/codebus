//! `jsonl` sink — append one JSON line per [`RunLog`] to a date-rotated
//! file under a configurable directory.
//!
//! File path: `<dir>/runs-YYYY-MM-DD.jsonl` where `YYYY-MM-DD` is derived
//! from `entry.started_at` (UTC). A run that starts at 23:55 UTC and
//! finishes at 00:10 UTC the next day still lands in the older date's
//! file — the spec pins selection on `started_at` (not write-time clock,
//! not `finished_at`) so a long-running invocation never accidentally
//! splits its log line into a different file from where it was attributed.
//!
//! Audit lens (sharp edges):
//!   - Opens with `OpenOptions::append(true).create(true)` — concurrent
//!     writes from different processes are atomic line-wise on POSIX (one
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

use crate::log::sink::{LogError, LogSink, RunLog, TokenUsage};
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
        // RFC 3339 always starts with `YYYY-MM-DD` (10 chars). Slice
        // defensively in case the field is shorter than expected (e.g.
        // empty string from a misconfigured caller).
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

    fn write_token_usage(&mut self, _usage: &TokenUsage) -> Result<(), LogError> {
        // Token-usage incremental updates are folded into the final RunLog
        // by callers that care; the jsonl sink only persists run-level
        // rows. Future: add a separate tokens.jsonl if streaming detail is
        // needed.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn nanos() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codebus-jsonl-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample_run_at(goal: &str, started: &str, finished: &str) -> RunLog {
        RunLog {
            goal: goal.into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: started.into(),
            finished_at: finished.into(),
            tokens: TokenUsage {
                input_tokens: 100,
                output_tokens: 200,
                cache_read_tokens: Some(50),
                cache_write_tokens: Some(25),
                reasoning_tokens: None,
                extras: serde_json::Value::Null,
            },
            wiki_changed: true,
            lint_error_count: 0,
            lint_warn_count: 2,
        }
    }

    fn sample_run(goal: &str) -> RunLog {
        sample_run_at(goal, "2026-05-06T10:00:00Z", "2026-05-06T10:00:30Z")
    }

    #[test]
    fn write_run_creates_file_named_runs_yyyy_mm_dd() {
        // Spec scenario: "First run of a UTC day creates a new file"
        let dir = tmp_dir("today");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run_at(
            "g1",
            "2026-05-07T23:30:00Z",
            "2026-05-07T23:30:10Z",
        ))
        .expect("write_run");
        let target = dir.join("runs-2026-05-07.jsonl");
        assert!(target.exists(), "expected {} to exist", target.display());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_run_appends_jsonl_line() {
        // Spec scenario: "Subsequent run on same UTC date appends to existing file"
        let dir = tmp_dir("appendline");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run_at(
            "g1",
            "2026-05-07T23:30:00Z",
            "2026-05-07T23:30:30Z",
        ))
        .unwrap();
        s.write_run(&sample_run_at(
            "g2",
            "2026-05-07T23:55:00Z",
            "2026-05-07T23:55:30Z",
        ))
        .unwrap();
        let target = dir.join("runs-2026-05-07.jsonl");
        let content = fs::read_to_string(&target).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("each line must parse as JSON");
            assert!(parsed.get("goal").is_some());
            assert!(parsed.get("tokens").is_some());
        }
        assert!(lines[0].contains("\"goal\":\"g1\""));
        assert!(lines[1].contains("\"goal\":\"g2\""));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn run_crossing_utc_midnight_writes_to_started_at_file() {
        // Spec scenario: "Run crossing UTC midnight writes to file matching started_at"
        let dir = tmp_dir("midnight");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run_at(
            "long",
            "2026-05-07T23:55:00Z",
            "2026-05-08T00:10:00Z",
        ))
        .expect("write_run");
        // Selection by started_at — file is for 05-07, NOT 05-08.
        assert!(dir.join("runs-2026-05-07.jsonl").exists());
        assert!(!dir.join("runs-2026-05-08.jsonl").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn different_utc_dates_produce_different_files() {
        let dir = tmp_dir("twodays");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run_at(
            "day1",
            "2026-05-07T10:00:00Z",
            "2026-05-07T10:01:00Z",
        ))
        .unwrap();
        s.write_run(&sample_run_at(
            "day2",
            "2026-05-08T10:00:00Z",
            "2026-05-08T10:01:00Z",
        ))
        .unwrap();
        assert!(dir.join("runs-2026-05-07.jsonl").exists());
        assert!(dir.join("runs-2026-05-08.jsonl").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_run_creates_parent_dir_if_missing() {
        let dir = tmp_dir("nested").join("deep").join("path");
        assert!(!dir.exists());
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run("g1")).expect("creates dirs");
        assert!(dir.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn jsonl_sink_name_is_stable() {
        let s = JsonlSink::new(tmp_dir("name"));
        assert_eq!(s.name(), "jsonl");
    }

    #[test]
    fn jsonl_sink_is_object_safe() {
        let _: Box<dyn LogSink> = Box::new(JsonlSink::new(tmp_dir("dyn")));
    }

    #[test]
    fn write_token_usage_is_noop_for_jsonl() {
        let dir = tmp_dir("tokenusage");
        let mut s = JsonlSink::new(&dir);
        assert!(s.write_token_usage(&TokenUsage::default()).is_ok());
        // No file should be created (we don't persist incremental token usage).
        assert!(!dir.exists() || fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn token_usage_with_none_cache_fields_omits_keys_in_json() {
        // Spec scenario: "TokenUsage with all None cache fields serializes
        // without those keys"
        let usage = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
            extras: serde_json::Value::Null,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"output_tokens\":50"));
        assert!(!json.contains("cache_read_tokens"));
        assert!(!json.contains("cache_write_tokens"));
        assert!(!json.contains("reasoning_tokens"));
        assert!(!json.contains("extras"));
    }

    #[test]
    fn run_log_with_none_metadata_fields_omits_keys_in_json() {
        let run = RunLog {
            goal: "g".into(),
            mode: "query".into(),
            model: None,
            effort: None,
            started_at: "2026-05-07T10:00:00Z".into(),
            finished_at: "2026-05-07T10:00:01Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
        };
        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("\"mode\":\"query\""));
        assert!(!json.contains("\"model\""));
        assert!(!json.contains("\"effort\""));
    }
}
