//! Test-only mock for `claude -p`. Replaces the real claude binary in
//! integration tests via the `CODEBUS_CLAUDE_BIN` env override hook on
//! `agent::claude_cli::invoke`. Behavior is controlled by env vars so a
//! single mock binary covers multiple test scenarios.
//!
//! Env contract:
//!   CODEBUS_MOCK_LOG       — path to write a structured args+cwd dump
//!                            (one `key=value` line per arg / cwd field);
//!                            unset → don't write log
//!   CODEBUS_MOCK_BEHAVIOR  — one of:
//!     `success-noop`              (default) exit 0 without touching files
//!     `success-write-page`        write `wiki/concepts/test.md` then exit 0
//!     `failure-write-then-exit-1` write `wiki/concepts/partial.md` then exit 1
//!
//! Working directory at invocation time is whatever the parent `Command`
//! set via `current_dir()` — for goal verb integration tests this is the
//! `.codebus/` vault root, so the relative `wiki/concepts/...` writes land
//! in the test's temp vault.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let log_path = env::var("CODEBUS_MOCK_LOG").ok();
    let behavior = env::var("CODEBUS_MOCK_BEHAVIOR")
        .unwrap_or_else(|_| "success-noop".to_string());
    let args: Vec<String> = env::args().skip(1).collect();
    let cwd: PathBuf = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(path) = log_path.as_deref() {
        let mut log = String::new();
        log.push_str(&format!("cwd={}\n", cwd.display()));
        for a in &args {
            log.push_str(&format!("arg={a}\n"));
        }
        let _ = fs::write(path, log);
    }

    match behavior.as_str() {
        "success-noop" => ExitCode::SUCCESS,

        "success-write-page" => {
            if write_test_page("wiki/concepts/test.md", "test").is_err() {
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }

        "failure-write-then-exit-1" => {
            // Write a page (partial work) then exit non-zero so the test can
            // assert the codebus side commits the partial snapshot anyway
            // (v2 carry: commit on failure).
            let _ = write_test_page("wiki/concepts/partial.md", "partial");
            ExitCode::from(1)
        }

        other => {
            eprintln!("mock-claude: unknown behavior `{other}`");
            ExitCode::from(2)
        }
    }
}

fn write_test_page(rel_path: &str, name: &str) -> std::io::Result<()> {
    let path = PathBuf::from(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!("---\nname: {name}\n---\n\nbody from mock-claude\n");
    fs::write(&path, body)
}
