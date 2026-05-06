//! `jsonl` sink — append one JSON line per [`RunLog`] to a date-rotated
//! file under a configurable directory.
//!
//! File path: `<dir>/YYYY-MM-DD.jsonl` (UTC date). Date is taken at write
//! time so a long-running daemon naturally rolls files at midnight UTC.
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

use crate::log::sink::{LogError, LogSink, RunLog, TokenUsage};
use crate::wiki::date::utc_today_iso;
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

    fn target_path(&self) -> PathBuf {
        self.dir.join(format!("{}.jsonl", utc_today_iso()))
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
            .open(self.target_path())?;
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

    fn sample_run(goal: &str) -> RunLog {
        RunLog {
            goal: goal.into(),
            started_at: "2026-05-06T10:00:00Z".into(),
            finished_at: "2026-05-06T10:00:30Z".into(),
            tokens: TokenUsage {
                input_tokens: 100,
                output_tokens: 200,
                cache_read_tokens: 50,
                cache_write_tokens: 25,
            },
            wiki_changed: true,
            lint_error_count: 0,
            lint_warn_count: 2,
        }
    }

    #[test]
    fn write_run_creates_file_with_today_date() {
        let dir = tmp_dir("today");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run("g1")).expect("write_run");
        let target = dir.join(format!("{}.jsonl", utc_today_iso()));
        assert!(target.exists(), "expected {} to exist", target.display());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_run_appends_jsonl_line() {
        let dir = tmp_dir("appendline");
        let mut s = JsonlSink::new(&dir);
        s.write_run(&sample_run("g1")).unwrap();
        s.write_run(&sample_run("g2")).unwrap();
        let target = dir.join(format!("{}.jsonl", utc_today_iso()));
        let content = fs::read_to_string(&target).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for line in &lines {
            let parsed: serde_json::Value =
                serde_json::from_str(line).expect("each line must parse as JSON");
            assert!(parsed.get("goal").is_some());
            assert!(parsed.get("tokens").is_some());
        }
        // Lines preserve insertion order.
        assert!(lines[0].contains("\"goal\":\"g1\""));
        assert!(lines[1].contains("\"goal\":\"g2\""));
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
        let target = dir.join(format!("{}.jsonl", utc_today_iso()));
        assert!(!target.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
