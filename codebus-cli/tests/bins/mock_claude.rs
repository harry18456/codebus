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
//!     `success-stream-json`       emit canonical 5-line stream-json + exit 0
//!     `failure-stream-json`       emit partial stream + exit 1
//!     `chat-init-success`         emit init(session_id) + simple result, exit 0
//!     `chat-emit-promote`         emit init + assistant text with marker + result
//!     `chat-multi-tool`           emit init + 2 tool_use events + result
//!   CODEBUS_MOCK_SESSION_ID  — session_id value for `chat-*` behaviors
//!                              (defaults to `mock-session-0001` when unset)
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
    let behavior = env::var("CODEBUS_MOCK_BEHAVIOR").unwrap_or_else(|_| "success-noop".to_string());
    let args: Vec<String> = env::args().skip(1).collect();
    let cwd: PathBuf = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if let Some(path) = log_path.as_deref() {
        let mut log = String::new();
        log.push_str(&format!("cwd={}\n", cwd.display()));
        for a in &args {
            log.push_str(&format!("arg={a}\n"));
        }
        // Dump the env vars codebus is expected to scope-inject for the
        // azure profile. `claude-code-endpoint-profiles` change uses this
        // to assert `Command::envs` actually carries the 3 vars without
        // leaking to the parent shell. Missing var → omit line.
        for key in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_API_KEY",
            "CLAUDE_CODE_DISABLE_ADVISOR_TOOL",
        ] {
            if let Ok(v) = env::var(key) {
                log.push_str(&format!("env_{key}={v}\n"));
            }
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

        // v3-run-log: emit 5 stream-json lines covering the full event taxonomy
        // (system → assistant text → assistant tool_use → user tool_result →
        // result with usage). Lets integration tests verify the parse + render
        // + RunLog-write pipeline end-to-end against a deterministic stream.
        "success-stream-json" => {
            emit_stream_json_success();
            ExitCode::SUCCESS
        }

        // Same as success-stream-json but truncated mid-flow (no result event)
        // and exits non-zero. Tests assert that the verb still writes a RunLog
        // entry with zero tokens (Usage event was never emitted).
        "failure-stream-json" => {
            emit_stream_json_partial();
            ExitCode::from(1)
        }

        // v3-chat-verb: emit `{type:system,subtype:init,session_id:...}`
        // as the first line so chat-verb's `sniff_init_session_id` populates
        // `InvokeReport.session_id` deterministically. Followed by a tiny
        // assistant text + result.
        "chat-init-success" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"hello back"}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb: same as chat-init-success but the assistant text
        // begins with the promote-suggestion line marker so chat-verb emits
        // a `VerbLifecycleEvent::PromoteSuggestion { reason }` event.
        "chat-emit-promote" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"[CODEBUS_PROMOTE_SUGGESTION] mock topic worth promoting\n\nBody of the response."}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":5,"output_tokens":3}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb: emit init + two tool_use events so chat-verb's
        // activity stream render prints two `→ Tool ...` lines.
        "chat-multi-tool" => {
            let sid = session_id();
            println!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Glob","input":{{"pattern":"wiki/modules/*.md"}}}}]}}}}"#
            );
            println!(
                r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"results","is_error":false}}]}}}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"wiki/modules/uv-lib.md"}}}}]}}}}"#
            );
            println!(
                r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"file contents","is_error":false}}]}}}}"#
            );
            println!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"summary"}}]}}}}"#
            );
            println!(
                r#"{{"type":"result","usage":{{"input_tokens":10,"output_tokens":4}}}}"#
            );
            ExitCode::SUCCESS
        }

        // v3-chat-verb cancel test: emit init (flushed) so the parent can
        // capture session_id, then trickle further events with deliberate
        // sleeps between each so a caller-flipped cancel flag fires AFTER
        // the init line is processed but BEFORE the final result line.
        // Each emit is explicitly flushed so piped stdout buffering does
        // not defeat the timing window.
        "chat-trickle-cancel" => {
            let sid = session_id();
            emit_flushed(&format!(
                r#"{{"type":"system","subtype":"init","session_id":"{sid}","tools":["Read","Glob","Grep"]}}"#
            ));
            // Sleep so the cancel-flipping test thread has time to flip
            // the AtomicBool while the parent is blocked reading the
            // next line from the pipe.
            std::thread::sleep(std::time::Duration::from_millis(800));
            emit_flushed(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"x"}}]}}"#,
            );
            std::thread::sleep(std::time::Duration::from_millis(800));
            emit_flushed(
                r#"{"type":"result","usage":{"input_tokens":1,"output_tokens":1}}"#,
            );
            ExitCode::SUCCESS
        }

        other => {
            eprintln!("mock-claude: unknown behavior `{other}`");
            ExitCode::from(2)
        }
    }
}

fn session_id() -> String {
    env::var("CODEBUS_MOCK_SESSION_ID").unwrap_or_else(|_| "mock-session-0001".to_string())
}

/// Print one line to stdout and flush immediately. Used by trickle
/// behaviors so the parent (which reads via `BufReader::lines()` on a
/// piped child stdout) sees each line as it's emitted, not at process
/// exit. Default `println!` is fine for one-shot dumps but unreliable
/// when the test cares about between-line timing.
fn emit_flushed(line: &str) {
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{line}");
    let _ = lock.flush();
}

fn emit_stream_json_success() {
    println!(r#"{{"type":"system","subtype":"init"}}"#);
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"思考中..."}}]}}}}"#
    );
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"/x"}}}}]}}}}"#
    );
    println!(
        r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"file contents","is_error":false}}]}}}}"#
    );
    println!(
        r#"{{"type":"result","usage":{{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}}}"#
    );
}

fn emit_stream_json_partial() {
    println!(r#"{{"type":"system","subtype":"init"}}"#);
    println!(
        r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"about to fail"}}]}}}}"#
    );
    // No result event → no Usage → RunLog tokens stay zero.
}

fn write_test_page(rel_path: &str, name: &str) -> std::io::Result<()> {
    let path = PathBuf::from(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!("---\nname: {name}\n---\n\nbody from mock-claude\n");
    fs::write(&path, body)
}
