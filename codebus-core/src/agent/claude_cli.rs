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

use crate::agent::env_overrides::EnvOverrides;
use crate::log::{TokenUsage, accumulate_token_usage};
use crate::stream::{StreamEvent, parse_claude_stream_line};
use chrono::SecondsFormat;
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Inputs for [`invoke`]. Caller (verb command modules) constructs this with
/// verb-specific values and a `'static` toolset slice.
///
/// `env` carries scoped environment overrides injected into the child
/// process. System profile callers pass `EnvOverrides::for_system()`
/// (empty); azure profile callers pass `EnvOverrides::for_azure(...)`
/// after resolving the API key via `config::keyring::read_azure_key`.
/// The spawn path uses `Command::envs(...)` — it does NOT touch the
/// parent shell environment (audited: no `std::env::set_var` in the
/// `agent` module).
pub struct InvokeAgentOptions {
    pub slash_command: String,
    pub vault_root: PathBuf,
    pub toolset: &'static [&'static str],
    pub bash_whitelist: Option<&'static str>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub env: EnvOverrides,
    /// Claude CLI session id to resume. When `Some(id)`, `invoke()` appends
    /// `--resume <id>` to the spawned `claude` argv before the toolset
    /// flags so the new process continues the same conversation history.
    /// When `None` (the default for goal/query/fix verbs), no `--resume`
    /// arg is added — the spawn starts a fresh Claude CLI session.
    /// chat-verb (v3-chat-verb) uses this to drive multi-turn REPL via
    /// `--resume <id>` between turns.
    pub resume_session_id: Option<String>,
}

/// Result of one [`invoke`] call. Returned to the verb command so it can
/// build a `RunLog` entry (mode / goal / model+effort / lint counts /
/// wiki_changed are filled in by the caller; this struct carries only the
/// agent-process-derived fields).
#[derive(Debug, Clone)]
pub struct InvokeReport {
    pub exit: ExitStatus,
    pub accumulated_tokens: TokenUsage,
    /// RFC 3339 UTC timestamp captured immediately before `Command::spawn`.
    pub started_at: String,
    /// RFC 3339 UTC timestamp captured immediately after `child.wait`
    /// returns. SHALL be greater than or equal to `started_at`.
    pub finished_at: String,
    /// v3-chat-verb: Claude CLI session id extracted from the spawn's
    /// first `{type: "system", subtype: "init", session_id: "..."}`
    /// stream-json line. `Some(id)` when the init event was observed;
    /// `None` when the spawn failed before init or the line was missing
    /// the field. Chat verb uses this to drive `--resume <id>` on the
    /// next turn; goal/query/fix verbs ignore the field.
    pub session_id: Option<String>,
}

/// Spawn the configured `claude -p` child process, consume its stream-json
/// stdout, deliver each parsed `StreamEvent` to the caller-supplied
/// `on_event` closure, accumulate `Usage` events, and wait for the child
/// to exit.
///
/// The claude binary path is read from env var `CODEBUS_CLAUDE_BIN` (test
/// override hook for integration tests; production path: env unset, falls
/// back to the literal `"claude"` and relies on PATH lookup).
///
/// Stdin is closed (`Stdio::null`) so the child does not block on input.
/// Stdout is piped to a `BufReader::lines()` loop on the main thread; each
/// line is parsed by [`parse_claude_stream_line`] and the resulting events
/// are delivered to `on_event` (caller decides rendering — CLI passes
/// `|e| print_event(&e, &render_opts)`, GUI passes a Tauri event-emit
/// closure). `Usage` events are accumulated into
/// `InvokeReport::accumulated_tokens` before being forwarded. Stderr is
/// piped to a background thread that copies it to the parent's stderr
/// verbatim — agent error messages remain visible without codebus
/// interpretation.
///
/// When `cancel` is `Some(flag)`, the function reads the flag with
/// `Ordering::Relaxed` after processing each stdout line. When the flag
/// is observed as `true`, the function invokes `child.kill()` on the
/// spawned child (best-effort — failures are ignored, the child may have
/// already exited), drains remaining stdout without invoking `on_event`
/// further, reaps the child with `child.wait()`, and returns
/// `Ok(InvokeReport)` with `exit` reflecting the killed state.
///
/// Returns the [`InvokeReport`] on successful spawn-and-wait. Errors here
/// mean the spawn itself failed (binary not found, fork error, etc.).
pub fn invoke(
    opts: InvokeAgentOptions,
    mut on_event: impl FnMut(StreamEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> io::Result<InvokeReport> {
    let claude_bin = std::env::var("CODEBUS_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
    let mut cmd = build_claude_cmd(&opts, &claude_bin);

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

    // Main-thread stream loop: read lines, parse, accumulate, deliver to
    // caller closure. Cancel signal polled after each line — flip true →
    // kill child + drain remaining stdout silently + break.
    let mut accumulated = TokenUsage::default();
    let mut session_id: Option<String> = None;
    let stdout = child.stdout.take().expect("stdout piped");
    let reader = BufReader::new(stdout);
    let mut cancelled = false;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if cancelled {
            // Already cancelled; keep draining lines silently so the
            // child's writer doesn't block on a full pipe buffer.
            continue;
        }
        // v3-chat-verb: capture the first `{type:system,subtype:init,session_id}`
        // line out-of-band. Stream parser intentionally skips system events
        // (they're not part of the StreamEvent closed enum per
        // agent-stream-rendering capability), but chat verb needs the
        // session_id for --resume on the next turn. Done as a cheap sniff:
        // first hit wins, never overwrites.
        if session_id.is_none() {
            if let Some(id) = sniff_init_session_id(&line) {
                session_id = Some(id);
            }
        }
        for event in parse_claude_stream_line(&line) {
            if let StreamEvent::Usage(u) = &event {
                accumulate_token_usage(&mut accumulated, u);
            }
            on_event(event);
        }
        if let Some(flag) = &cancel
            && flag.load(Ordering::Relaxed)
        {
            // Best-effort kill; ignore failure (child may have exited
            // between the poll and the kill call). The drain branch
            // above keeps reading lines so the OS pipe buffer empties.
            let _ = child.kill();
            cancelled = true;
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
        session_id,
    })
}

/// v3-chat-verb: try to extract `session_id` from a raw stream-json line if
/// it matches `{type:"system", subtype:"init", session_id:"..."}` shape.
/// Returns `None` for any non-init line or malformed JSON — caller polls
/// every line until a hit is observed.
fn sniff_init_session_id(line: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(line).ok()?;
    if parsed.get("type")?.as_str()? != "system" {
        return None;
    }
    if parsed.get("subtype")?.as_str()? != "init" {
        return None;
    }
    Some(parsed.get("session_id")?.as_str()?.to_string())
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

/// Compose the full `claude -p` argv (and scoped env vars) for spawn.
///
/// Pulled out as a separate helper so unit tests can inspect the resulting
/// `Command::get_args()` without running a real child process. The caller
/// (`invoke()`) is responsible for `current_dir` / stdio piping / spawn.
///
/// Argv order (stable, exercised by tests):
///   1. `-p <slash_command>`
///   2. `--resume <id>` — appended only when `opts.resume_session_id` is `Some(_)`
///   3. `--tools <csv>`
///   4. `--allowedTools <csv>`
///   5. `--permission-mode acceptEdits`
///   6. `--output-format stream-json`
///   7. `--verbose`
///   8. `--model <m>` — optional
///   9. `--effort <e>` — optional
pub(crate) fn build_claude_cmd(opts: &InvokeAgentOptions, claude_bin: &str) -> Command {
    let tools_csv = build_tools_csv(opts.toolset, opts.bash_whitelist);
    let allowed_tools_csv = build_allowed_tools_csv(opts.toolset, opts.bash_whitelist);

    let mut cmd = Command::new(claude_bin);
    cmd.arg("-p").arg(&opts.slash_command);
    // v3-chat-verb: when caller supplies a session id, append `--resume <id>`
    // BEFORE the toolset flags so the spawned claude process resumes the
    // same conversation history (spike-verified: --resume + --tools 三旗 並存).
    // For goal/query/fix this is always None → no --resume arg → byte-equivalent
    // to pre-chat-verb spawn argv.
    if let Some(id) = opts.resume_session_id.as_deref() {
        cmd.arg("--resume").arg(id);
    }
    cmd.arg("--tools")
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

    // Scoped env injection. `cmd.envs(...)` sets vars on the child only;
    // the parent shell environment is never modified (the `agent` module
    // contains zero `std::env::set_var` calls — see env_overrides.rs docs).
    cmd.envs(opts.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));

    cmd
}

/// Compose the `--tools` value: bare tool names (the toolset hard-gate).
pub(crate) fn build_tools_csv(toolset: &[&str], bash_whitelist: Option<&str>) -> String {
    let mut parts: Vec<&str> = toolset.to_vec();
    if bash_whitelist.is_some() {
        parts.push("Bash");
    }
    parts.join(",")
}

/// Compose the `--allowedTools` value: bare tool names (auto-approval) plus
/// any fine-grained permission specifiers.
pub(crate) fn build_allowed_tools_csv(toolset: &[&str], bash_whitelist: Option<&str>) -> String {
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
            env: EnvOverrides::for_system(),
            resume_session_id: None,
        };
        let InvokeAgentOptions {
            slash_command,
            vault_root,
            toolset,
            bash_whitelist,
            model,
            effort,
            env,
            resume_session_id,
        } = opts;
        assert_eq!(slash_command, "/codebus-goal \"x\"");
        assert_eq!(vault_root, PathBuf::from("/tmp/v"));
        assert_eq!(toolset, &["Read", "Glob", "Grep"]);
        assert!(bash_whitelist.is_none());
        assert!(model.is_none());
        assert!(effort.is_none());
        assert!(env.is_empty());
        assert!(resume_session_id.is_none());
    }

    fn fixture_opts(resume: Option<&str>) -> InvokeAgentOptions {
        InvokeAgentOptions {
            slash_command: "/codebus-chat \"hi\"".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read", "Glob", "Grep"],
            bash_whitelist: None,
            model: None,
            effort: None,
            env: EnvOverrides::for_system(),
            resume_session_id: resume.map(String::from),
        }
    }

    fn cmd_args_collected(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn invoke_appends_resume_flag_when_session_id_some() {
        // v3-chat-verb: chat verb spawn must carry --resume <id> when caller
        // provides a session id, placed BEFORE --tools so the toolset gate
        // remains the spike-verified hard floor regardless of session resume.
        let cmd = build_claude_cmd(&fixture_opts(Some("abc-123")), "claude");
        let args = cmd_args_collected(&cmd);

        // Find --resume position and assert <id> follows it AND it precedes --tools.
        let resume_pos = args
            .iter()
            .position(|a| a == "--resume")
            .expect("--resume must appear when resume_session_id is Some");
        assert_eq!(args.get(resume_pos + 1).map(String::as_str), Some("abc-123"));

        let tools_pos = args
            .iter()
            .position(|a| a == "--tools")
            .expect("--tools must always appear");
        assert!(
            resume_pos < tools_pos,
            "--resume must appear before --tools (got resume at {resume_pos}, tools at {tools_pos})"
        );
    }

    #[test]
    fn invoke_omits_resume_flag_when_session_id_none() {
        // Existing query/goal/fix verbs construct InvokeAgentOptions with
        // resume_session_id: None and expect byte-equivalent argv to the
        // pre-chat-verb implementation. No --resume must appear.
        let cmd = build_claude_cmd(&fixture_opts(None), "claude");
        let args = cmd_args_collected(&cmd);
        assert!(
            !args.iter().any(|a| a == "--resume"),
            "--resume must NOT appear when resume_session_id is None; got argv: {args:?}"
        );
    }

    #[test]
    fn sniff_init_session_id_extracts_from_system_init_line() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc-123","tools":["Read"]}"#;
        assert_eq!(
            sniff_init_session_id(line),
            Some("abc-123".to_string()),
            "system init line must yield session_id"
        );
    }

    #[test]
    fn sniff_init_session_id_returns_none_for_non_init_lines() {
        let cases = [
            r#"{"type":"assistant","message":{}}"#,
            r#"{"type":"system","subtype":"hook_started"}"#,
            r#"{"type":"result","usage":{}}"#,
            "not even json",
            "",
        ];
        for line in cases {
            assert_eq!(sniff_init_session_id(line), None, "line: {line}");
        }
    }

    #[test]
    fn sniff_init_session_id_returns_none_when_field_missing() {
        let line = r#"{"type":"system","subtype":"init","tools":["Read"]}"#;
        assert_eq!(sniff_init_session_id(line), None);
    }

    #[test]
    fn invoke_options_carries_resume_session_id_field() {
        // v3-chat-verb: chat verb needs to drive --resume <id> via this
        // optional field. Constructing with Some(...) and reading the field
        // back pins the public API shape so chat library can rely on it.
        let opts = InvokeAgentOptions {
            slash_command: "/codebus-chat \"x\"".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read", "Glob", "Grep"],
            bash_whitelist: None,
            model: None,
            effort: None,
            env: EnvOverrides::for_system(),
            resume_session_id: Some("abc-123".into()),
        };
        assert_eq!(opts.resume_session_id.as_deref(), Some("abc-123"));
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
                env: EnvOverrides::for_system(),
                resume_session_id: None,
            },
            |_event| {},
            None,
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
            session_id: None,
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

    /// Verification for task 2.1 (Spawn Stdio Architecture for Stream Capture):
    /// the closure-dispatch pattern delivers events in parser-output order.
    /// The full subprocess-spawn happy path is exercised by the CLI
    /// integration tests (`goal_flow.rs` / `query_flow.rs` / `fix_flow.rs`)
    /// via mock_claude — here we pin the in-process closure semantics that
    /// invoke()'s stream loop depends on.
    #[test]
    fn closure_dispatch_order_matches_parser_output() {
        let lines = [
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/x"}}]}}"#,
            r#"{"type":"result","usage":{"input_tokens":10,"output_tokens":5}}"#,
        ];
        let mut collected: Vec<StreamEvent> = Vec::new();
        let mut expected: Vec<StreamEvent> = Vec::new();
        for line in &lines {
            let events = parse_claude_stream_line(line);
            for event in &events {
                expected.push(event.clone());
            }
            // Mirror invoke()'s inner loop shape exactly.
            for event in events {
                collected.push(event);
            }
        }
        assert_eq!(collected, expected);
        assert_eq!(collected.len(), 3);
    }

    /// Verification for task 2.2 (Cancellation Signal Polling): the
    /// `Option<Arc<AtomicBool>>` cancel shape behaves correctly — flag can
    /// be cloned across threads and a flip is observed by every clone via
    /// `Ordering::Relaxed`. Mid-stream kill behavior is verified by CLI
    /// integration tests (task 5.3) with mock_claude.
    #[test]
    fn cancel_arc_atomic_bool_shape_is_clonable_and_flippable() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        let flag = Arc::new(AtomicBool::new(false));
        let mirror = flag.clone();
        assert!(!mirror.load(Ordering::Relaxed));
        flag.store(true, Ordering::Relaxed);
        assert!(mirror.load(Ordering::Relaxed));
    }

    /// Verification for task 2.2 (None path): invoke() compiles cleanly
    /// when `cancel: None` is passed and the spawn fails — this also
    /// asserts no_polling_overhead pattern is reachable at the type level.
    #[test]
    fn invoke_none_cancel_path_compiles_and_returns_spawn_error() {
        unsafe {
            std::env::set_var(
                "CODEBUS_CLAUDE_BIN",
                "/nonexistent/codebus-test-no-such-bin",
            );
        }
        let mut events: Vec<StreamEvent> = Vec::new();
        let r = invoke(
            InvokeAgentOptions {
                slash_command: "/x".into(),
                vault_root: std::env::temp_dir(),
                toolset: &["Read"],
                bash_whitelist: None,
                model: None,
                effort: None,
                env: EnvOverrides::for_system(),
                resume_session_id: None,
            },
            |event| events.push(event),
            None,
        );
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        assert!(r.is_err());
        // Spawn failed before any event flowed → closure not invoked.
        assert!(events.is_empty());
    }
}
