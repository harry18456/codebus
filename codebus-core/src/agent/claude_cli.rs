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

use crate::agent::backend::AgentBackend;
use crate::agent::env_overrides::EnvOverrides;
use crate::agent::spawn_spec::SpawnSpec;
use crate::log::{TokenUsage, accumulate_token_usage};
use crate::stream::StreamEvent;
use chrono::SecondsFormat;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};


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

/// Spawn the agent child process described by `backend` + `spec`, consume
/// its stdout, deliver each parsed `StreamEvent` to the caller-supplied
/// `on_event` closure, accumulate `Usage` events, and wait for the child
/// to exit.
///
/// This loop is provider-agnostic: it owns spawn / stdio piping / cancel
/// polling / stderr passthrough / token accumulation, and delegates the
/// three provider-specific concerns to the [`AgentBackend`] — command
/// construction ([`AgentBackend::build_command`]), stdout line parsing
/// ([`AgentBackend::parse_stream_line`]), and session-id extraction
/// ([`AgentBackend::extract_session_id`]). It contains no `claude` binary
/// name, Claude argv flags, or Claude stream-json field names.
///
/// `vault_root` is the working directory for the spawned child (neutral
/// spawn context, not provider-specific).
///
/// Stdin is closed (`Stdio::null`) so the child does not block on input.
/// Stdout is piped to a `BufReader::lines()` loop on the main thread; each
/// line is parsed by `backend.parse_stream_line` and the resulting events
/// are delivered to `on_event` (caller decides rendering). `Usage` events
/// are accumulated into `InvokeReport::accumulated_tokens` before being
/// forwarded. Stderr is piped to a background thread that copies it to the
/// parent's stderr verbatim.
///
/// When `cancel` is `Some(flag)`, the function reads the flag with
/// `Ordering::Relaxed` after processing each stdout line. When the flag
/// is observed as `true`, the function invokes `child.kill()` on the
/// spawned child (best-effort), drains remaining stdout without invoking
/// `on_event` further, reaps the child, and returns `Ok(InvokeReport)`
/// with `exit` reflecting the killed state.
///
/// Returns the [`InvokeReport`] on successful spawn-and-wait. Errors here
/// mean the spawn itself failed (binary not found, fork error, etc.).
pub fn invoke(
    backend: &dyn AgentBackend,
    spec: SpawnSpec,
    vault_root: &Path,
    mut on_event: impl FnMut(StreamEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> io::Result<InvokeReport> {
    let mut cmd = backend.build_command(&spec);

    let started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    let mut child = cmd
        .current_dir(vault_root)
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
        // Capture the session id out-of-band: the backend recognises its
        // own provider's session/thread line (Claude `system`/`init`). First
        // hit wins, never overwrites. Chat verb needs it to drive resume on
        // the next turn.
        if session_id.is_none() {
            if let Some(id) = backend.extract_session_id(&line) {
                session_id = Some(id);
            }
        }
        for event in backend.parse_stream_line(&line) {
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
pub(crate) fn sniff_init_session_id(line: &str) -> Option<String> {
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

/// Compose the full `claude -p` argv from non-`'static` pieces. This is the
/// single source of truth for the Claude argv —
/// [`super::claude_backend::ClaudeBackend::build_command`] maps a `SpawnSpec`
/// onto these parameters and delegates here. Pulled out as a separate helper
/// so unit tests can inspect `Command::get_args()` without spawning. The
/// caller (`invoke` via the backend) is responsible for `current_dir` /
/// stdio piping / spawn.
///
/// Argv order (stable, exercised by tests; see the `agent-backend` spec
/// `Claude Backend Argv Equivalence`):
///   1. `-p <slash_command>`
///   2. `--resume <id>` — appended only when `resume_session_id` is `Some(_)`
///   3. `--tools <csv>`
///   4. `--allowedTools <csv>`
///   5. `--permission-mode acceptEdits`
///   6. `--output-format stream-json`
///   7. `--verbose`
///   8. `--strict-mcp-config` + `--mcp-config {"mcpServers":{}}` — MCP load-layer isolation
///   9. `--model <m>` — optional
///  10. `--effort <e>` — optional
pub(crate) fn compose_claude_cmd(
    claude_bin: &str,
    slash_command: &str,
    resume_session_id: Option<&str>,
    toolset: &[&str],
    bash_whitelist: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    env: &EnvOverrides,
) -> Command {
    let tools_csv = build_tools_csv(toolset, bash_whitelist);
    let allowed_tools_csv = build_allowed_tools_csv(toolset, bash_whitelist);

    let mut cmd = Command::new(claude_bin);
    cmd.arg("-p").arg(slash_command);
    // v3-chat-verb: when caller supplies a session id, append `--resume <id>`
    // BEFORE the toolset flags so the spawned claude process resumes the
    // same conversation history (spike-verified: --resume + --tools 三旗 並存).
    // For goal/query/fix this is always None → no --resume arg → byte-equivalent
    // to pre-chat-verb spawn argv.
    if let Some(id) = resume_session_id {
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
        .arg("--verbose")
        // spawn-mcp-isolation: hard-isolate the MCP load layer. `--tools` /
        // `--allowedTools` only gate built-in tools — they do NOT exclude MCP
        // tools (verified 2026-05-21: ambient connector / user-scope MCP tools
        // leak into the spawned session otherwise). `--strict-mcp-config` makes
        // claude use ONLY the servers from `--mcp-config`; the empty config
        // declares zero servers, so no ambient MCP (user / project / connector)
        // is loaded. Unconditional, no escape hatch.
        .arg("--strict-mcp-config")
        .arg("--mcp-config")
        .arg(r#"{"mcpServers":{}}"#);

    if let Some(model) = model {
        cmd.arg("--model").arg(model);
    }
    if let Some(effort) = effort {
        cmd.arg("--effort").arg(effort);
    }

    // Scoped env injection. `cmd.envs(...)` sets vars on the child only;
    // the parent shell environment is never modified (the `agent` module
    // contains zero `std::env::set_var` calls — see env_overrides.rs docs).
    cmd.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));

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
    use crate::agent::{ClaudeBackend, Permission};
    use crate::config::Verb;
    use crate::config::endpoint::ClaudeCodeConfig;
    use crate::stream::parse_claude_stream_line;

    fn cmd_args_collected(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn pos(args: &[String], flag: &str) -> usize {
        args.iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} must appear in argv: {args:?}"))
    }

    fn compose(resume: Option<&str>, model: Option<&str>, effort: Option<&str>) -> Command {
        compose_claude_cmd(
            "claude",
            "/codebus-goal \"x\"",
            resume,
            &["Read", "Glob", "Grep"],
            None,
            model,
            effort,
            &EnvOverrides::for_system(),
        )
    }

    // === MCP isolation (compose-level; spawn-mcp-isolation invariant) ===

    #[test]
    fn compose_carries_mcp_isolation_flags() {
        let args = cmd_args_collected(&compose(None, None, None));
        assert!(
            args.iter().any(|a| a == "--strict-mcp-config"),
            "--strict-mcp-config must be present: {args:?}"
        );
        let mcp = pos(&args, "--mcp-config");
        assert_eq!(
            args.get(mcp + 1).map(String::as_str),
            Some(r#"{"mcpServers":{}}"#)
        );
    }

    #[test]
    fn compose_positions_mcp_flags_after_verbose_before_model() {
        let args = cmd_args_collected(&compose(None, Some("claude-opus-4-6"), Some("high")));
        let verbose = pos(&args, "--verbose");
        let strict = pos(&args, "--strict-mcp-config");
        let mcp = pos(&args, "--mcp-config");
        let model = pos(&args, "--model");
        assert!(verbose < strict && verbose < mcp);
        assert!(strict < model && mcp < model);
    }

    #[test]
    fn compose_resume_before_tools_with_mcp_after_tools() {
        let args = cmd_args_collected(&compose(Some("abc-123"), None, None));
        let resume = pos(&args, "--resume");
        let tools = pos(&args, "--tools");
        let strict = pos(&args, "--strict-mcp-config");
        assert_eq!(args.get(resume + 1).map(String::as_str), Some("abc-123"));
        assert!(resume < tools, "--resume must precede --tools");
        assert!(tools < strict, "MCP flags follow --tools");
    }

    #[test]
    fn compose_omits_resume_when_none() {
        let args = cmd_args_collected(&compose(None, None, None));
        assert!(!args.iter().any(|a| a == "--resume"));
    }

    // === sniff_init_session_id ===

    #[test]
    fn sniff_init_session_id_extracts_from_system_init_line() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc-123","tools":["Read"]}"#;
        assert_eq!(sniff_init_session_id(line), Some("abc-123".to_string()));
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

    // === csv helpers ===

    #[test]
    fn build_tools_csv_no_bash_whitelist() {
        assert_eq!(
            build_tools_csv(&["Read", "Glob", "Grep"], None),
            "Read,Glob,Grep"
        );
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
        assert_eq!(
            build_allowed_tools_csv(&["Read", "Glob", "Grep"], None),
            "Read,Glob,Grep"
        );
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

    // === invoke (provider-agnostic loop, driven by a backend) ===

    fn test_spec() -> SpawnSpec {
        SpawnSpec {
            verb: Verb::Query,
            prompt: "/codebus-query \"x\"".into(),
            permission: Permission::ReadOnly,
            command_allowance: None,
            resume_session_id: None,
        }
    }

    #[test]
    fn invoke_returns_io_error_when_binary_missing() {
        unsafe {
            std::env::set_var(
                "CODEBUS_CLAUDE_BIN",
                "/nonexistent/path/to/no-such-claude-binary-xyz",
            );
        }
        let backend = ClaudeBackend::new(ClaudeCodeConfig::default(), EnvOverrides::for_system());
        let tmp = std::env::temp_dir();
        let r = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None);
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        assert!(r.is_err(), "expected spawn err, got io::Result");
    }

    #[test]
    fn invoke_none_cancel_path_compiles_and_returns_spawn_error() {
        unsafe {
            std::env::set_var("CODEBUS_CLAUDE_BIN", "/nonexistent/codebus-test-no-such-bin");
        }
        let backend = ClaudeBackend::new(ClaudeCodeConfig::default(), EnvOverrides::for_system());
        let mut events: Vec<StreamEvent> = Vec::new();
        let tmp = std::env::temp_dir();
        let r = invoke(
            &backend,
            test_spec(),
            tmp.as_path(),
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

    #[test]
    fn invoke_report_struct_carries_required_fields() {
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

    /// The closure-dispatch pattern delivers events in parser-output order.
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
            for event in events {
                collected.push(event);
            }
        }
        assert_eq!(collected, expected);
        assert_eq!(collected.len(), 3);
    }

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
}
