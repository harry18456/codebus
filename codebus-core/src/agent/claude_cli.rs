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
use crate::agent::env_overrides::{EnvOverrides, passthrough_env};
use crate::agent::process_kill::KillHandle;
use crate::agent::spawn_spec::SpawnSpec;
use crate::log::{TokenUsage, apply_token_usage};
use crate::stream::StreamEvent;
use chrono::SecondsFormat;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Poll interval for the background cancel watcher thread (see
/// [`spawn_cancel_watcher`]). 100ms balances "user expects ≤ 1s
/// terminal-state latency" against "do not burn CPU spinning on a flag".
const CANCEL_WATCHER_POLL_INTERVAL: Duration = Duration::from_millis(100);


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
    /// next turn; the quiz verb records it in its `RunLog` entry (for
    /// logging, not resume); goal/query/fix verbs ignore the field.
    pub session_id: Option<String>,
    /// run-outcome-lifecycle-integrity (Part A): `true` if and only if the
    /// per-run wall-clock `timeout` elapsed and the watcher terminated the
    /// agent process tree. A timeout-induced kill is indistinguishable from
    /// any other kill by exit status alone, so the verb layer reads THIS
    /// flag (not `exit`) to classify the run as `outcome == "failed"` +
    /// `interrupt_reason == Timeout`. Always `false` when `timeout` was
    /// `None` or the run finished before the limit.
    pub timed_out: bool,
    /// run-outcome-lifecycle-integrity (Part B): count of tool results during
    /// the run that both terminated non-zero (`is_error == true`) AND carried
    /// a locale-independent sandbox / permission-denial marker (per
    /// [`crate::stream::is_sandbox_denial`]). Best-effort observability for
    /// the codex "top-level exit 0 but inner command blocked" case. The verb
    /// copies this into `RunLog.sandbox_denial_count`; it does NOT alter
    /// `outcome`.
    pub sandbox_denial_count: usize,
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
    timeout: Option<Duration>,
) -> io::Result<InvokeReport> {
    let mut cmd = backend.build_command(&spec);
    let stdin_payload = backend.stdin_payload(&spec);

    let started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    // run-outcome-lifecycle-integrity (Part A): monotonic clock for the
    // wall-clock timeout. The RFC 3339 `started_at` string above is for the
    // RunLog; `Instant` is what the watcher compares `elapsed()` against.
    let started_instant = Instant::now();

    let stdin_mode = if stdin_payload.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };

    cmd.current_dir(vault_root)
        .stdin(stdin_mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Cross-platform process-tree control: on Unix this configures
    // `process_group(0)` so the child becomes a new PGID leader; on
    // Windows it is a no-op (the Job Object is attached after spawn).
    KillHandle::pre_spawn(&mut cmd);
    let mut child = cmd.spawn()?;
    // Wrap the spawned child in a KillHandle so the watcher can take
    // out the entire descendant tree, not just the immediate child.
    // Agent CLIs on Windows are `cmd.exe` → `node.exe` (claude.cmd /
    // codex.cmd are .cmd shims), and on Unix they spawn node + shell
    // tooling underneath. A naive single-PID kill leaves grandchildren
    // holding the stdout pipe open, which wedges the main-loop reader
    // and `invoke` never returns — see the cancelling-stuck-fix design
    // doc's "Pre-apply 校準" section.
    let kill_handle = Arc::new(KillHandle::install(&child)?);

    // Feed the optional stdin payload (codex multi-line prompt workaround for
    // Windows .cmd batch-file argv validation) and immediately close stdin so
    // the child does not block waiting on more input.
    if let Some(payload) = stdin_payload {
        if let Some(mut stdin) = child.stdin.take() {
            io::Write::write_all(&mut stdin, payload.as_bytes())?;
        }
    }

    // Background cancel watcher: the main loop below reads stdout with a
    // blocking `BufReader::lines()`. When the child stops emitting stdout
    // (LLM hung on a network call, child waiting on a stalled tool
    // result), that read blocks indefinitely and the inline cancel-flag
    // check inside the loop never runs. The watcher polls the cancel
    // flag on its own schedule (every CANCEL_WATCHER_POLL_INTERVAL) and
    // terminates the child's process tree via the KillHandle — the
    // resulting EOF on stdout unblocks the main loop. `done` is the
    // watcher's "main loop completed, you can stop polling" signal so
    // the watcher does not outlive `invoke`.
    let done = Arc::new(AtomicBool::new(false));
    // run-outcome-lifecycle-integrity (Part A): the watcher sets this when
    // the wall-clock `timeout` elapses and it terminates the tree. Read into
    // `InvokeReport.timed_out` after the watcher is joined.
    let timed_out = Arc::new(AtomicBool::new(false));
    let watcher_handle = spawn_cancel_watcher(
        kill_handle.clone(),
        cancel.clone(),
        done.clone(),
        started_instant,
        timeout,
        timed_out.clone(),
    );

    // Hand stderr to a background thread so it drains without blocking
    // the main loop. The thread exits when the child closes stderr (i.e.
    // on child termination).
    //
    // Default: drop child stderr into a sink. Agent CLIs (codex in
    // particular) print informational diagnostics to stderr — the Azure
    // `/openai/models` capabilities JSON, init progress, model lookup
    // — that overwhelm the dev terminal even when no error occurred.
    // Errors that matter to the user surface through verb events
    // (events.jsonl) and the non-zero exit code; the dev terminal does
    // not need the raw stream.
    //
    // Escape hatch: set `CODEBUS_FORWARD_AGENT_STDERR=1` to forward
    // child stderr to the parent terminal when debugging spawn / auth /
    // init failures that do not surface elsewhere.
    let stderr = child.stderr.take().expect("stderr piped");
    let forward_stderr = std::env::var("CODEBUS_FORWARD_AGENT_STDERR")
        .ok()
        .filter(|v| !v.is_empty() && v != "0")
        .is_some();
    // agent-run-integrity (vertical A): the thread now CLASSIFIES each stderr
    // line for sandbox-denial markers as well as disposing of it. A denial
    // that surfaces only on stderr (never produced a stdout `ToolResult`) is
    // otherwise invisible. Classification runs REGARDLESS of `forward_stderr`
    // — that toggle only decides whether the raw stream reaches the dev
    // terminal, not whether denials are observable. The thread returns the
    // per-line denial count via its `JoinHandle`, which the main thread sums
    // into `sandbox_denial_count` below (no de-dup against the stdout source;
    // over-count is acceptable per the design).
    let stderr_handle = thread::spawn(move || -> usize {
        let reader = BufReader::new(stderr);
        if forward_stderr {
            crate::stream::classify_stderr_lines(reader, io::stderr().lock(), true)
        } else {
            crate::stream::classify_stderr_lines(reader, io::sink(), false)
        }
    });

    // Main-thread stream loop: read lines, parse, accumulate, deliver to
    // caller closure. Cancel signal polled after each line — flip true →
    // kill child + drain remaining stdout silently + break.
    let mut accumulated = TokenUsage::default();
    // Provider-declared token-usage combination (provider-agnostic): read once
    // from the backend trait, then dispatch per `Usage` event on the enum only
    // — the loop never names a provider. Delta sums (Claude); Cumulative takes
    // the latest snapshot (codex `turn.completed.usage`, avoiding double-count).
    let token_semantics = backend.token_usage_semantics();
    // run-outcome-lifecycle-integrity (Part B): count tool results that both
    // failed (`is_error`) AND carry a locale-independent sandbox-denial
    // marker. Accumulated like tokens; surfaced on `InvokeReport`.
    let mut sandbox_denial_count: usize = 0;
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
                apply_token_usage(&mut accumulated, u, token_semantics);
            }
            // Part B: only failed tool results are denial candidates; within
            // those, only ones carrying a curated marker count (grep-no-match
            // exits non-zero but has no marker → not counted).
            if let StreamEvent::ToolResult { output, is_error } = &event
                && *is_error
                && crate::stream::is_sandbox_denial(output)
            {
                sandbox_denial_count += 1;
            }
            on_event(event);
        }
        if let Some(flag) = &cancel
            && flag.load(Ordering::Relaxed)
        {
            // Best-effort tree kill; ignore failure (the tree may
            // already be gone if the watcher beat us to it, or the
            // child may have exited between the poll and the kill
            // call). The drain branch above keeps reading lines so
            // the OS pipe buffer empties.
            let _ = kill_handle.terminate_tree();
            cancelled = true;
        }
    }

    // Signal the cancel watcher to exit before reaping. The watcher
    // sleeps for up to CANCEL_WATCHER_POLL_INTERVAL between checks, so
    // its `join()` below waits at most that long even when `cancel` was
    // never flipped. This must happen before `child.wait()` so the
    // watcher does not race with reap and accidentally kill a freshly
    // recycled PID.
    done.store(true, Ordering::SeqCst);

    // Reap the child — stdout EOF doesn't strictly mean exit yet on some
    // platforms, but `wait()` blocks until truly terminated.
    let exit = child.wait()?;
    let finished_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Best-effort join on the stderr classification thread. 5s deadline:
    // longer is unlikely to help; if the thread is wedged, detach (and lose
    // its denial count — `None` → contributes 0, best-effort observability).
    let stderr_denials = join_within(stderr_handle, Duration::from_secs(5)).unwrap_or(0);
    // agent-run-integrity (vertical A): sum the stderr-derived denials into the
    // stdout-derived count. The two sources are NOT de-duplicated — a denial
    // appearing on both is counted twice; over-count is acceptable for this
    // best-effort observability signal, which never changes `outcome`.
    let sandbox_denial_count = sandbox_denial_count + stderr_denials;

    // Strict join on the cancel watcher: it must not outlive `invoke`,
    // since it holds the child's PID and could otherwise fire a kill
    // against a recycled PID once the OS reuses it. Bounded by
    // CANCEL_WATCHER_POLL_INTERVAL plus a small slack — if the watcher
    // wedges (it should not), surface it as a panic rather than detach.
    let _ = watcher_handle.join();

    Ok(InvokeReport {
        exit,
        accumulated_tokens: accumulated,
        started_at,
        finished_at,
        session_id,
        // Read after the strict watcher join above so the flag is settled.
        timed_out: timed_out.load(Ordering::SeqCst),
        sandbox_denial_count,
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

/// Spawn the background cancel watcher described in [`invoke`]'s comment.
///
/// The watcher polls two flags on a fixed interval:
/// - `done`: set by `invoke` when the main loop finishes and the
///   watcher must stop. Checked first so the watcher exits promptly
///   when no cancel ever fires.
/// - `cancel`: the caller-supplied cancel signal. When set to `true`,
///   the watcher terminates the child's entire process tree via
///   the shared `KillHandle` (idempotent across both platforms, so
///   the inline fast-path cancel inside the main loop may safely
///   fire in parallel) and exits.
///
/// run-outcome-lifecycle-integrity (Part A): the watcher gained a third
/// check — `timeout`. When `Some(limit)` and `started_instant.elapsed()`
/// exceeds it, the watcher terminates the tree via the SAME
/// `KillHandle::terminate_tree()` the cancel path uses (no second kill
/// mechanism), sets `timed_out` so the verb layer can classify the run, and
/// exits. The resulting stdout EOF unblocks the main loop exactly as cancel
/// does. `done`/`cancel` keep precedence (checked first) so a user cancel or
/// a clean finish is never mislabeled as a timeout.
///
/// The `KillHandle` is shared via `Arc` because the main thread also
/// holds it for the inline fast-path kill. Holding a `&Child` here
/// would conflict with the main thread's `child.wait()`.
fn spawn_cancel_watcher(
    kill_handle: Arc<KillHandle>,
    cancel: Option<Arc<AtomicBool>>,
    done: Arc<AtomicBool>,
    started_instant: Instant,
    timeout: Option<Duration>,
    timed_out: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            if done.load(Ordering::SeqCst) {
                return;
            }
            if let Some(flag) = &cancel
                && flag.load(Ordering::SeqCst)
            {
                // Idempotent across both platforms; the main-loop fast
                // path may have already terminated the tree by now.
                let _ = kill_handle.terminate_tree();
                return;
            }
            if let Some(limit) = timeout
                && started_instant.elapsed() > limit
            {
                // Wall-clock limit hit. Same tree-kill as cancel; record
                // `timed_out` so the verb derives outcome=failed + Timeout.
                let _ = kill_handle.terminate_tree();
                timed_out.store(true, Ordering::SeqCst);
                return;
            }
            thread::sleep(CANCEL_WATCHER_POLL_INTERVAL);
        }
    })
}

/// Wait for the thread to finish for at most `deadline`, returning its value
/// (`Some(T)`) if it joined cleanly. If it doesn't finish in time, detach
/// (drop the handle without joining) and return `None`. Used for the stderr
/// classification thread which SHOULD exit when the child terminates but
/// might wedge on a pathological pipe state; a detached thread contributes
/// no denial count (best-effort).
fn join_within<T>(handle: thread::JoinHandle<T>, deadline: Duration) -> Option<T> {
    let started = Instant::now();
    // std::thread doesn't expose a timed join, so poll is_finished.
    while !handle.is_finished() && started.elapsed() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if handle.is_finished() {
        // The thread finished; a join here will not block. A panicked
        // thread yields `Err` → `None` (no count, best-effort).
        handle.join().ok()
    } else {
        // Detach. The OS will clean up the thread when it eventually exits.
        None
    }
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
///   8. `--no-session-persistence` — appended only when `no_session_persistence` is `true` (every verb except `Chat`); suppresses the Claude session rollout for single-shot verbs. Valid only in `-p` mode, which codebus always uses.
///   9. `--strict-mcp-config` + `--mcp-config {"mcpServers":{}}` — MCP load-layer isolation
///  10. `--setting-sources project,local` — user-global setting-source isolation (excludes `~/.claude` CLAUDE.md / settings / plugins)
///  11. `--model <m>` — optional
///  12. `--effort <e>` — optional
pub(crate) fn compose_claude_cmd(
    claude_bin: &str,
    slash_command: &str,
    resume_session_id: Option<&str>,
    no_session_persistence: bool,
    toolset: &[&str],
    bash_whitelist: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    env: &EnvOverrides,
) -> Command {
    let tools_csv = build_tools_csv(toolset, bash_whitelist);
    let allowed_tools_csv = build_allowed_tools_csv(toolset, bash_whitelist);

    let mut cmd = Command::new(claude_bin);
    // SEC (spawn env scrub): drop the inherited parent environment, then
    // re-inject ONLY the cross-platform system-essential allowlist. This
    // keeps parent-shell secrets (`GITHUB_TOKEN` / `AWS_*` / `KUBECONFIG`,
    // codebus's own `CODEBUS_*` keys) out of the agent child. The provider
    // injection (`cmd.envs(env.iter()...)` near the end of this fn) runs
    // AFTER this clear, so the azure keys survive. Order: env_clear →
    // passthrough → provider. Spec `claude-code-config / Scoped Environment
    // Injection At Spawn`.
    cmd.env_clear();
    cmd.envs(passthrough_env());
    cmd.arg("-p").arg(slash_command);
    // v3-chat-verb: when caller supplies a session id, append `--resume <id>`
    // BEFORE the toolset flags so the spawned claude process resumes the
    // same conversation history (spike-verified: --resume + --tools 三旗 並存).
    // For goal/query/fix/quiz this is always None → no --resume arg → byte-equivalent
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
        .arg("--verbose");

    // Session persistence gating (mirrors codex backend's `--ephemeral` gate):
    // non-chat verbs never resume, so suppress the Claude session rollout to
    // avoid orphan session files. Chat keeps persistence so `--resume` works.
    // Valid only in `-p` mode, which codebus always uses.
    if no_session_persistence {
        cmd.arg("--no-session-persistence");
    }

    cmd
        // spawn-mcp-isolation: hard-isolate the MCP load layer. `--tools` /
        // `--allowedTools` only gate built-in tools — they do NOT exclude MCP
        // tools (verified 2026-05-21: ambient connector / user-scope MCP tools
        // leak into the spawned session otherwise). `--strict-mcp-config` makes
        // claude use ONLY the servers from `--mcp-config`; the empty config
        // declares zero servers, so no ambient MCP (user / project / connector)
        // is loaded. Unconditional, no escape hatch.
        .arg("--strict-mcp-config")
        .arg("--mcp-config")
        .arg(r#"{"mcpServers":{}}"#)
        // claude-setting-sources-user-isolation: hard-isolate the user-global
        // setting layer. By default claude loads the `user`, `project`, and
        // `local` setting sources — the `user` source pulls in `~/.claude/
        // CLAUDE.md`, `~/.claude/settings.json`, and user-global plugins, which
        // bleed into every wiki-building spawn and bias its behaviour (e.g. a
        // user-global "always reply in zh-tw" rule overriding the schema's
        // "follow the prompt-context language" policy). Restricting to
        // `project,local` excludes the user source while keeping the vault's
        // own layers: the `.codebus/.claude/settings.json` check-bash /
        // check-read PreToolUse hook gate and the `.codebus/CLAUDE.md` schema
        // both remain in effect (2026-05-31 spike verified all three facets).
        // This mirrors the codex backend's `--ignore-user-config` user
        // isolation. Unconditional, no escape hatch.
        .arg("--setting-sources")
        .arg("project,local");

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
        // Goal-shaped (non-chat) → session persistence suppressed.
        compose_claude_cmd(
            "claude",
            "/codebus-goal \"x\"",
            resume,
            true,
            &["Read", "Glob", "Grep"],
            None,
            model,
            effort,
            &EnvOverrides::for_system(),
        )
    }

    #[test]
    fn compose_includes_no_session_persistence_after_verbose_before_mcp() {
        let args = cmd_args_collected(&compose(None, None, None));
        let verbose = pos(&args, "--verbose");
        let nsp = pos(&args, "--no-session-persistence");
        let strict = pos(&args, "--strict-mcp-config");
        assert!(verbose < nsp, "flag follows --verbose");
        assert!(nsp < strict, "flag precedes the MCP isolation flags");
    }

    #[test]
    fn compose_omits_no_session_persistence_when_false() {
        // Chat-shaped → persistence retained, flag absent.
        let cmd = compose_claude_cmd(
            "claude",
            "/codebus-chat \"x\"",
            Some("abc-123"),
            false,
            &["Read", "Glob", "Grep"],
            None,
            None,
            None,
            &EnvOverrides::for_system(),
        );
        let args = cmd_args_collected(&cmd);
        assert!(!args.iter().any(|a| a == "--no-session-persistence"));
        assert_eq!(args.get(pos(&args, "--resume") + 1).map(String::as_str), Some("abc-123"));
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
    fn compose_carries_setting_sources_user_isolation() {
        let args = cmd_args_collected(&compose(None, Some("claude-opus-4-6"), Some("high")));
        let ss = pos(&args, "--setting-sources");
        assert_eq!(
            args.get(ss + 1).map(String::as_str),
            Some("project,local"),
            "--setting-sources value must be project,local: {args:?}"
        );
        let strict = pos(&args, "--strict-mcp-config");
        let model = pos(&args, "--model");
        assert!(
            strict < ss,
            "--setting-sources must follow the MCP isolation flags"
        );
        assert!(ss < model, "--setting-sources must precede --model");
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
            resolve_as: None,
            sub_mode: None,
            input: "x".into(),
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
        let r = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None);
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
            timed_out: false,
            sandbox_denial_count: 0,
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

    // === cancelling-stuck-fix: bounded-latency cancel coverage ===
    //
    // These tests spawn real child processes via a minimal `TestBackend`
    // implementation so they exercise the same blocking-IO codepath as
    // production. They are the verification target for the
    // `Cancellation Polling Not Coupled To Stdout` requirement.

    enum TestChild {
        /// Silent: spawns a process that sleeps without emitting any
        /// stdout. Exercises the watcher-thread cancel path.
        Silent,
        /// Streaming: spawns a process that emits one line, then sleeps
        /// for several seconds. Exercises the main-loop fast-path cancel.
        StreamingThenSleep,
        /// Finite: spawns a process that emits three lines and exits
        /// promptly. Exercises clean shutdown / no-leak path with no
        /// cancel flag flipped.
        Finite,
    }

    struct TestBackend(TestChild);

    impl AgentBackend for TestBackend {
        fn build_command(&self, _spec: &SpawnSpec) -> Command {
            match self.0 {
                TestChild::Silent => silent_child_command(),
                TestChild::StreamingThenSleep => streaming_then_sleep_command(),
                TestChild::Finite => finite_child_command(),
            }
        }
        fn parse_stream_line(&self, _line: &str) -> Vec<StreamEvent> {
            Vec::new()
        }
        fn extract_session_id(&self, _line: &str) -> Option<String> {
            None
        }
    }

    #[cfg(unix)]
    fn silent_child_command() -> Command {
        let mut c = Command::new("sleep");
        c.arg("30");
        c
    }

    #[cfg(windows)]
    fn silent_child_command() -> Command {
        // PowerShell `Start-Sleep` blocks silently and tolerates a closed
        // stdin (Stdio::null). Windows `timeout.exe` would be the obvious
        // pick but it refuses redirected stdin, and Git-for-Windows ships
        // a GNU-coreutils `timeout` that shadows the native one on PATH.
        let mut c = Command::new("powershell.exe");
        c.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        c
    }

    #[cfg(unix)]
    fn streaming_then_sleep_command() -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", "echo line1; sleep 5"]);
        c
    }

    #[cfg(windows)]
    fn streaming_then_sleep_command() -> Command {
        // Same rationale as `silent_child_command`: PowerShell tolerates
        // Stdio::null stdin and gives us a portable way to emit one line
        // then block silently.
        let mut c = Command::new("powershell.exe");
        c.args([
            "-NoProfile",
            "-Command",
            "[Console]::Out.WriteLine('line1'); [Console]::Out.Flush(); Start-Sleep -Seconds 5",
        ]);
        c
    }

    #[cfg(unix)]
    fn finite_child_command() -> Command {
        let mut c = Command::new("sh");
        c.args(["-c", "echo line1; echo line2; echo line3"]);
        c
    }

    #[cfg(windows)]
    fn finite_child_command() -> Command {
        let mut c = Command::new("cmd");
        c.args(["/c", "echo line1 & echo line2 & echo line3"]);
        c
    }

    /// Silent-child cancel: the child emits no stdout, then `cancel` is set
    /// `true`. The new watcher thread SHALL observe the flag within ~100ms
    /// and kill the child via `kill_child_by_id(pid)`. `invoke` SHALL
    /// return within 200ms of the flag being set.
    ///
    /// RED phase: this test FAILS against the pre-fix `invoke` because the
    /// blocking `BufReader::lines()` never advances when the child has
    /// stopped writing, so the inline cancel check never runs.
    #[test]
    fn cancel_returns_within_bounded_latency_when_child_silent() {
        let backend = TestBackend(TestChild::Silent);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_mirror = cancel.clone();
        let tmp = std::env::temp_dir();

        // Flip cancel from a separate thread shortly after `invoke` starts,
        // so the spawn has time to materialise before the flag flips.
        let flipper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancel_mirror.store(true, Ordering::SeqCst);
        });

        let flip_observed = Instant::now();
        let r = invoke(
            &backend,
            test_spec(),
            tmp.as_path(),
            |_event| {},
            Some(cancel),
            None,
        );
        let elapsed_from_start = flip_observed.elapsed();
        let _ = flipper.join();

        let report = r.expect("spawn should succeed on a host with sleep/cmd available");
        assert!(
            !report.exit.success(),
            "killed child should not exit successfully (exit={:?})",
            report.exit
        );
        // Contract: ≤ 200ms from cancel-flag-set to child kill. Test
        // budget = 150ms pre-flip wait + 200ms contract + generous CI
        // slack. The slack absorbs PowerShell startup on Windows
        // (observed ~700-900ms) and shared-runner load; the spec
        // assertion is still that invoke returns *well* before the
        // child's natural 30s sleep elapses, which a 3s budget proves.
        assert!(
            elapsed_from_start < Duration::from_secs(3),
            "invoke should return shortly after cancel was set, well \
             before the child's natural 30s sleep; actual = {:?}",
            elapsed_from_start
        );
    }

    /// Streaming-child cancel: the child emits one stdout line, then
    /// sleeps. `cancel` is flipped immediately. The inline fast-path
    /// SHALL kill the child within one loop iteration. Locks in the
    /// existing behaviour so it does not regress when the watcher is
    /// added.
    #[test]
    fn cancel_during_streaming_returns_within_bounded_latency() {
        let backend = TestBackend(TestChild::StreamingThenSleep);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_mirror = cancel.clone();
        let tmp = std::env::temp_dir();

        // Flip very early so the first stdout line triggers cancel on the
        // very next iteration of the main loop.
        let flipper = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            cancel_mirror.store(true, Ordering::SeqCst);
        });

        let started = Instant::now();
        let r = invoke(
            &backend,
            test_spec(),
            tmp.as_path(),
            |_event| {},
            Some(cancel),
            None,
        );
        let elapsed = started.elapsed();
        let _ = flipper.join();

        let report = r.expect("spawn should succeed");
        assert!(
            !report.exit.success(),
            "killed child should not exit successfully"
        );
        // Streaming child sleeps for 5s after first line; if cancel
        // path were broken, invoke would wait the full 5s. Bound is
        // generous to absorb CI noise.
        assert!(
            elapsed < Duration::from_millis(1500),
            "streaming-cancel fast path should return well before the \
             child's natural 5s sleep elapses; actual = {:?}",
            elapsed
        );
    }

    /// Explicit synchronisation guarantee: the cancel watcher is joined
    /// (not detached) before `invoke` returns, so the watcher cannot
    /// outlive its child's PID slot. Since `invoke` performs a strict
    /// `watcher_handle.join()` synchronously before returning, "invoke
    /// returned" implies "watcher joined".
    #[test]
    fn watcher_joins_before_invoke_returns() {
        let backend = TestBackend(TestChild::Finite);
        let tmp = std::env::temp_dir();
        let started = Instant::now();
        let r = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None);
        let elapsed = started.elapsed();
        let report = r.expect("spawn should succeed");
        assert!(report.exit.success());
        // Bounded by CANCEL_WATCHER_POLL_INTERVAL (100ms) plus child
        // exec time. 2s gives ample slack.
        assert!(
            elapsed < Duration::from_secs(2),
            "watcher join contributes at most one poll interval; \
             total elapsed = {:?}",
            elapsed
        );
    }

    /// Concurrent double-kill safety: both the main-loop fast path and
    /// the watcher thread may attempt to kill the child at the same
    /// time. The contract is that this is harmless — `process_kill`'s
    /// idempotent behaviour (treat already-exited as Ok) plus the
    /// main-loop's `let _ = child.kill()` ignore-result discipline keeps
    /// `invoke` returning `Ok(InvokeReport)` even when both paths fire.
    ///
    /// This drives a streaming-then-sleep child: the main loop will
    /// read `line1` then immediately observe `cancel == true` and call
    /// `child.kill()`, while the watcher's 100ms poll will also observe
    /// the flag and call `kill_child_by_id`. The two races; both must
    /// be safe.
    #[test]
    fn concurrent_main_and_watcher_kill_is_safe() {
        let backend = TestBackend(TestChild::StreamingThenSleep);
        let cancel = Arc::new(AtomicBool::new(true));
        let tmp = std::env::temp_dir();
        let r = invoke(
            &backend,
            test_spec(),
            tmp.as_path(),
            |_event| {},
            Some(cancel),
            None,
        );
        let report = r.expect("invoke should return Ok even with concurrent kills");
        assert!(!report.exit.success(), "child was killed, not natural exit");
    }

    /// No-cancel-flag normal completion: child emits 3 lines and exits.
    /// Verifies `invoke` returns cleanly. Once the watcher thread lands,
    /// this also doubles as a smoke test that the watcher does not block
    /// `invoke`'s return when `cancel` is `None` AND when the `done` flag
    /// signals completion.
    #[test]
    fn watcher_thread_does_not_leak_on_normal_completion() {
        let backend = TestBackend(TestChild::Finite);
        let tmp = std::env::temp_dir();

        let started = Instant::now();
        let r = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None);
        let elapsed = started.elapsed();

        let report = r.expect("spawn should succeed");
        assert!(report.exit.success(), "finite child should exit success");
        assert!(!report.timed_out, "no timeout was set → timed_out is false");
        assert_eq!(
            report.sandbox_denial_count, 0,
            "TestBackend emits no ToolResult → no denials"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "invoke should return promptly after child exits; actual = {:?}",
            elapsed
        );
    }

    // === run-outcome-lifecycle-integrity Part A: wall-clock timeout ===

    /// Timeout fires: a silent 30s child with a short `timeout` SHALL be
    /// terminated by the watcher's third branch. `invoke` returns far before
    /// the child's natural 30s exit, with `timed_out == true` and a
    /// non-success exit. (Verification target for the `verb-library`
    /// `Run Wall-Clock Timeout Safety Net` "Timeout fires" scenario.)
    #[test]
    fn timeout_fires_terminates_tree_and_sets_timed_out() {
        let backend = TestBackend(TestChild::Silent);
        let tmp = std::env::temp_dir();
        let started = Instant::now();
        let r = invoke(
            &backend,
            test_spec(),
            tmp.as_path(),
            |_event| {},
            None,
            Some(Duration::from_millis(200)),
        );
        let elapsed = started.elapsed();
        let report = r.expect("spawn should succeed on a host with sleep/powershell");
        assert!(report.timed_out, "watcher should have flagged timed_out");
        assert!(
            !report.exit.success(),
            "timed-out child was killed, not a clean exit (exit={:?})",
            report.exit
        );
        // Generous bound: 200ms limit + poll interval + PowerShell startup
        // slack on Windows; still far below the child's natural 30s sleep.
        assert!(
            elapsed < Duration::from_secs(3),
            "invoke should return shortly after the timeout, well before the \
             child's natural 30s sleep; actual = {:?}",
            elapsed
        );
    }

    // === run-outcome-lifecycle-integrity Part B: denial accumulation ===

    /// Backend that maps the `Finite` child's three lines onto ToolResults
    /// exercising every denial-counting branch:
    /// - `line1` → failed result WITH a denial marker  → counted
    /// - `line2` → failed result WITHOUT a marker (grep-no-match) → NOT counted
    /// - `line3` → SUCCESS result whose text contains a marker → NOT counted
    struct DenialBackend;

    impl AgentBackend for DenialBackend {
        fn build_command(&self, _spec: &SpawnSpec) -> Command {
            finite_child_command()
        }
        fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent> {
            if line.contains("line1") {
                vec![StreamEvent::ToolResult {
                    output: "Set-Content : PermissionDenied ... UnauthorizedAccessError".into(),
                    is_error: true,
                }]
            } else if line.contains("line2") {
                vec![StreamEvent::ToolResult {
                    output: "no matches found".into(),
                    is_error: true,
                }]
            } else if line.contains("line3") {
                vec![StreamEvent::ToolResult {
                    output: "Access is denied".into(),
                    is_error: false,
                }]
            } else {
                Vec::new()
            }
        }
        fn extract_session_id(&self, _line: &str) -> Option<String> {
            None
        }
    }

    /// Only failed results carrying a marker are counted: one denial
    /// (`line1`), the grep-no-match (`line2`) and the success-with-marker
    /// (`line3`) are both excluded → `sandbox_denial_count == 1`.
    #[test]
    fn invoke_counts_only_failed_results_with_denial_marker() {
        let backend = DenialBackend;
        let tmp = std::env::temp_dir();
        let r = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None);
        let report = r.expect("spawn should succeed");
        assert_eq!(
            report.sandbox_denial_count, 1,
            "exactly the failed+marker result counts; got {}",
            report.sandbox_denial_count
        );
    }

    // === agent-run-integrity (vertical A): stderr-only denial accumulation ===

    /// Backend whose child writes a denial marker to STDERR and exits 0 with
    /// NO stdout `ToolResult`. Proves the stderr classification path counts a
    /// denial the stdout path can never see.
    struct StderrDenialBackend;

    impl AgentBackend for StderrDenialBackend {
        fn build_command(&self, _spec: &SpawnSpec) -> Command {
            stderr_denial_then_exit_zero_command()
        }
        fn parse_stream_line(&self, _line: &str) -> Vec<StreamEvent> {
            // Child emits nothing on stdout → no ToolResult events at all.
            Vec::new()
        }
        fn extract_session_id(&self, _line: &str) -> Option<String> {
            None
        }
    }

    #[cfg(unix)]
    fn stderr_denial_then_exit_zero_command() -> Command {
        let mut c = Command::new("sh");
        // Emit a curated denial marker on stderr, exit 0, no stdout.
        c.args(["-c", "echo 'cp: x: Permission denied' 1>&2; exit 0"]);
        c
    }

    #[cfg(windows)]
    fn stderr_denial_then_exit_zero_command() -> Command {
        let mut c = Command::new("cmd");
        // `1>&2` redirects the echo to stderr; the process exits 0.
        c.args(["/c", "echo Access is denied. 1>&2"]);
        c
    }

    /// A denial that appears ONLY on the child's stderr (top-level exit 0,
    /// no stdout ToolResult) is counted. Verification target for the A
    /// vertical's `agent::invoke` stderr-classification contract.
    #[test]
    fn invoke_counts_stderr_only_denial_with_exit_zero() {
        let backend = StderrDenialBackend;
        let tmp = std::env::temp_dir();
        let report = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None)
            .expect("spawn should succeed");
        assert!(
            report.exit.success(),
            "child exits 0 (denial does not change the top-level exit); exit={:?}",
            report.exit
        );
        assert!(
            report.sandbox_denial_count >= 1,
            "stderr-only denial must be counted; got {}",
            report.sandbox_denial_count
        );
    }

    /// A backend declaring `Cumulative` emits two running-total `Usage` events
    /// (100 then 250). `invoke` must report 250 (latest), NOT 350 (sum) — the
    /// provider-agnostic loop honors the declared semantics end-to-end.
    struct CumulativeUsageBackend;

    impl AgentBackend for CumulativeUsageBackend {
        fn build_command(&self, _spec: &SpawnSpec) -> Command {
            finite_child_command()
        }
        fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent> {
            if line.contains("line1") {
                vec![StreamEvent::Usage(TokenUsage {
                    input_tokens: 100,
                    output_tokens: 40,
                    ..Default::default()
                })]
            } else if line.contains("line2") {
                vec![StreamEvent::Usage(TokenUsage {
                    input_tokens: 250,
                    output_tokens: 90,
                    ..Default::default()
                })]
            } else {
                Vec::new()
            }
        }
        fn extract_session_id(&self, _line: &str) -> Option<String> {
            None
        }
        fn token_usage_semantics(&self) -> crate::log::TokenUsageSemantics {
            crate::log::TokenUsageSemantics::Cumulative
        }
    }

    #[test]
    fn invoke_cumulative_backend_reports_latest_usage_not_sum() {
        let backend = CumulativeUsageBackend;
        let tmp = std::env::temp_dir();
        let report = invoke(&backend, test_spec(), tmp.as_path(), |_event| {}, None, None)
            .expect("spawn should succeed");
        assert_eq!(
            report.accumulated_tokens.input_tokens, 250,
            "cumulative: latest snapshot wins, not the sum"
        );
        assert_ne!(
            report.accumulated_tokens.input_tokens, 350,
            "must not double-count cumulative usage"
        );
        assert_eq!(report.accumulated_tokens.output_tokens, 90);
    }
}
