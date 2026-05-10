//! Spawn `claude -p --output-format stream-json --verbose` with the
//! canonical sandbox flags + slash command + cwd at the vault root, parse
//! the stream-json stdout into [`StreamEvent`]s on the main thread (rendering
//! each via `render::stream_event` and accumulating Usage events), pass
//! stderr through to the parent's stderr on a background thread, and return
//! an [`InvokeReport`] with the exit status, accumulated tokens, and RFC
//! 3339 timestamps bracketing the spawn-wait.
//!
//! Sandbox triple flag was verified by 2026-05-09 spike (`_pii-toolgate-spike`,
//! 5 cells): `--tools` is the real toolset hard gate; `--allowedTools` is the
//! redundant auto-approval safety net; `--permission-mode acceptEdits` is
//! mandatory in `-p` mode. v3-config added `--model` / `--effort` per-verb.
//! v3-run-log adds `--output-format stream-json --verbose` and switches
//! stdout/stderr from `Stdio::inherit()` to `Stdio::piped()` so the parent
//! can parse + render. Input format stays default `text` because the prompt
//! is delivered via `-p <slash_command>`; `--input-format stream-json` would
//! make claude wait for streaming JSON messages on stdin and conflict with
//! the closed `Stdio::null()` stdin.

use crate::log::{TokenUsage, accumulate_token_usage};
use crate::render::{RenderOptions, print_event};
use crate::stream::{StreamEvent, parse_claude_stream_line};
use chrono::SecondsFormat;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Inputs for [`invoke`]. Caller (verb command modules) constructs this with
/// verb-specific values and a `'static` toolset slice.
pub struct InvokeAgentOptions {
    pub slash_command: String,
    pub vault_root: PathBuf,
    pub toolset: &'static [&'static str],
    pub bash_whitelist: Option<&'static str>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// Result of one [`invoke`] call. Returned to the verb command so it can
/// build a `RunLog` entry (mode / goal / model+effort / lint counts /
/// wiki_changed are filled in by the caller; this struct carries only the
/// agent-process-derived fields).
#[derive(Debug)]
pub struct InvokeReport {
    pub exit: ExitStatus,
    pub accumulated_tokens: TokenUsage,
    /// RFC 3339 UTC timestamp captured immediately before `Command::spawn`.
    pub started_at: String,
    /// RFC 3339 UTC timestamp captured immediately after `child.wait`
    /// returns. SHALL be greater than or equal to `started_at`.
    pub finished_at: String,
}

/// Spawn the configured `claude -p` child process, consume its stream-json
/// stdout for rendering + token accumulation, and wait for it to exit.
///
/// The claude binary path is read from env var `CODEBUS_CLAUDE_BIN` (test
/// override hook for integration tests; production path: env unset, falls
/// back to the literal `"claude"` and relies on PATH lookup).
///
/// Stdin is closed (`Stdio::null`) so the child does not block on input.
/// Stdout is piped to a `BufReader::lines()` loop on the main thread; each
/// line is parsed by [`parse_claude_stream_line`] and the resulting events
/// are rendered (`Thought` / `ToolUse` / `ToolResult` to stdout via
/// `render::stream_event::print_event`) or accumulated (`Usage` into
/// `InvokeReport::accumulated_tokens`). Stderr is piped to a background
/// thread that copies it to the parent's stderr verbatim — agent error
/// messages remain visible without codebus interpretation.
///
/// Returns the [`InvokeReport`] on successful spawn-and-wait. Errors here
/// mean the spawn itself failed (binary not found, fork error, etc.).
pub fn invoke(
    opts: InvokeAgentOptions,
    render_opts: &RenderOptions,
) -> io::Result<InvokeReport> {
    let claude_bin = std::env::var("CODEBUS_CLAUDE_BIN")
        .unwrap_or_else(|_| "claude".to_string());
    let tools_csv = build_tools_csv(opts.toolset, opts.bash_whitelist);
    let allowed_tools_csv = build_allowed_tools_csv(opts.toolset, opts.bash_whitelist);

    let mut cmd = Command::new(&claude_bin);
    cmd.arg("-p")
        .arg(&opts.slash_command)
        .arg("--tools")
        .arg(&tools_csv)
        .arg("--allowedTools")
        .arg(&allowed_tools_csv)
        .arg("--permission-mode")
        .arg("acceptEdits")
        // v3-run-log: enable stream-json so we can parse usage events and
        // render thought/tool/result inline. `--verbose` is required by the
        // claude CLI when `--output-format stream-json` is set. Input
        // format stays default `text` (prompt comes via `-p`).
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose");

    if let Some(model) = opts.model.as_deref() {
        cmd.arg("--model").arg(model);
    }
    if let Some(effort) = opts.effort.as_deref() {
        cmd.arg("--effort").arg(effort);
    }

    let started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    let mut child = cmd
        .current_dir(&opts.vault_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Hand stderr to a background thread so it streams to the parent's
    // stderr without blocking the main loop. The thread exits when the
    // child closes stderr (i.e. on child termination).
    let stderr = child.stderr.take().expect("stderr piped");
    let stderr_handle = thread::spawn(move || {
        let mut stderr = stderr;
        // io::copy returns Err only on read/write failure; ignore — the
        // thread's job is best-effort passthrough.
        let _ = io::copy(&mut stderr, &mut io::stderr().lock());
    });

    // Main-thread stream loop: read lines, parse, render, accumulate.
    let mut accumulated = TokenUsage::default();
    let stdout = child.stdout.take().expect("stdout piped");
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        for event in parse_claude_stream_line(&line) {
            if let StreamEvent::Usage(u) = &event {
                accumulate_token_usage(&mut accumulated, u);
            }
            print_event(&event, render_opts);
        }
    }

    // Reap the child — stdout EOF doesn't strictly mean exit yet on some
    // platforms, but `wait()` blocks until truly terminated.
    let exit = child.wait()?;
    let finished_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Best-effort join on the stderr passthrough thread. 5s deadline:
    // longer is unlikely to help; if the thread is wedged, detach.
    join_within(stderr_handle, Duration::from_secs(5));

    Ok(InvokeReport {
        exit,
        accumulated_tokens: accumulated,
        started_at,
        finished_at,
    })
}

/// Wait for the thread to finish for at most `deadline`. If it doesn't,
/// detach (drop the handle without joining). Used for the stderr
/// passthrough thread which SHOULD exit when the child terminates but
/// might wedge on a pathological pipe state.
fn join_within(handle: thread::JoinHandle<()>, deadline: Duration) {
    let started = Instant::now();
    // std::thread doesn't expose a timed join, so poll is_finished.
    while !handle.is_finished() && started.elapsed() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if handle.is_finished() {
        let _ = handle.join();
    }
    // Else: detach. The OS will clean up the thread when it eventually exits.
}

/// Compose the `--tools` value: bare tool names (the toolset hard-gate).
pub(crate) fn build_tools_csv(
    toolset: &[&str],
    bash_whitelist: Option<&str>,
) -> String {
    let mut parts: Vec<&str> = toolset.to_vec();
    if bash_whitelist.is_some() {
        parts.push("Bash");
    }
    parts.join(",")
}

/// Compose the `--allowedTools` value: bare tool names (auto-approval) plus
/// any fine-grained permission specifiers.
pub(crate) fn build_allowed_tools_csv(
    toolset: &[&str],
    bash_whitelist: Option<&str>,
) -> String {
    match bash_whitelist {
        None => toolset.join(","),
        Some(spec) if toolset.is_empty() => spec.to_string(),
        Some(spec) => format!("{},{}", toolset.join(","), spec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn invoke_options_struct_carries_required_fields() {
        let opts = InvokeAgentOptions {
            slash_command: "/codebus-goal \"x\"".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read", "Glob", "Grep"],
            bash_whitelist: None,
            model: None,
            effort: None,
        };
        let InvokeAgentOptions {
            slash_command,
            vault_root,
            toolset,
            bash_whitelist,
            model,
            effort,
        } = opts;
        assert_eq!(slash_command, "/codebus-goal \"x\"");
        assert_eq!(vault_root, PathBuf::from("/tmp/v"));
        assert_eq!(toolset, &["Read", "Glob", "Grep"]);
        assert!(bash_whitelist.is_none());
        assert!(model.is_none());
        assert!(effort.is_none());
    }

    #[test]
    fn build_tools_csv_no_bash_whitelist() {
        let csv = build_tools_csv(&["Read", "Glob", "Grep"], None);
        assert_eq!(csv, "Read,Glob,Grep");
    }

    #[test]
    fn build_tools_csv_appends_bare_bash_when_whitelist_supplied() {
        let csv = build_tools_csv(
            &["Read", "Glob", "Grep", "Write", "Edit"],
            Some("Bash(codebus lint *)"),
        );
        assert_eq!(csv, "Read,Glob,Grep,Write,Edit,Bash");
    }

    #[test]
    fn build_allowed_tools_csv_no_bash_whitelist() {
        let csv = build_allowed_tools_csv(&["Read", "Glob", "Grep"], None);
        assert_eq!(csv, "Read,Glob,Grep");
    }

    #[test]
    fn build_allowed_tools_csv_appends_restricted_bash_pattern() {
        let csv = build_allowed_tools_csv(
            &["Read", "Glob", "Grep", "Write", "Edit"],
            Some("Bash(codebus lint *)"),
        );
        assert_eq!(csv, "Read,Glob,Grep,Write,Edit,Bash(codebus lint *)");
    }

    #[test]
    fn build_csvs_diverge_on_bash_when_whitelist_supplied() {
        let toolset = &["Read", "Glob", "Grep", "Write", "Edit"];
        let whitelist = Some("Bash(codebus lint *)");
        let tools = build_tools_csv(toolset, whitelist);
        let allowed = build_allowed_tools_csv(toolset, whitelist);
        assert_ne!(tools, allowed);
        assert!(tools.ends_with(",Bash"));
        assert!(allowed.ends_with(",Bash(codebus lint *)"));
    }

    #[test]
    fn invoke_returns_io_error_when_binary_missing() {
        unsafe {
            std::env::set_var(
                "CODEBUS_CLAUDE_BIN",
                "/nonexistent/path/to/no-such-claude-binary-xyz",
            );
        }
        let r = invoke(
            InvokeAgentOptions {
                slash_command: "/x".into(),
                vault_root: std::env::temp_dir(),
                toolset: &["Read"],
                bash_whitelist: None,
                model: None,
                effort: None,
            },
            &RenderOptions::no_styling(),
        );
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        assert!(r.is_err(), "expected spawn err, got io::Result");
    }

    #[test]
    fn invoke_report_struct_carries_required_fields() {
        // Construction-only test — the spawn pathway requires a real binary
        // and is exercised by integration tests. Pin the field shape.
        use std::process::ExitStatus;
        let report = InvokeReport {
            exit: dummy_exit_zero(),
            accumulated_tokens: TokenUsage::default(),
            started_at: "2026-05-10T00:00:00Z".into(),
            finished_at: "2026-05-10T00:00:01Z".into(),
        };
        let _: ExitStatus = report.exit;
        assert_eq!(report.accumulated_tokens.input_tokens, 0);
        assert!(report.finished_at >= report.started_at);
    }

    #[cfg(unix)]
    fn dummy_exit_zero() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn dummy_exit_zero() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }
}
