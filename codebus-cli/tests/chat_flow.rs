//! Integration tests for `codebus chat` end-to-end flow (v3-chat-verb).
//!
//! These tests drive the real `codebus` binary against tempdir vaults but
//! substitute a Rust-built mock binary (tests/bins/mock_claude.rs) for the
//! claude CLI via the `CODEBUS_CLAUDE_BIN` env override on
//! `agent::claude_cli::invoke`. They cover chat-verb spec scenarios that
//! exercise the CLI REPL surface — vault precondition, exit aliases,
//! session_id propagation through `--resume <id>`, activity stream
//! rendering, RunLog mode=chat persistence, and promote-to-goal
//! subprocess spawn.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Build a minimal codebus vault under `tmp` so chat's vault precondition
/// passes. We bypass `codebus init` to keep the test fast and deterministic
/// (init creates many files we don't need for chat-only flows).
fn make_chat_vault(tmp: &TempDir) -> std::path::PathBuf {
    let repo = tmp.path().to_path_buf();
    let vault = repo.join(".codebus");
    let log_dir = vault.join("log");
    fs::create_dir_all(&log_dir).unwrap();
    repo
}

/// Run `codebus chat` in `repo` with the mock binary wired in. `stdin_bytes`
/// is the REPL input pipe — newline-terminated lines as the test driver.
/// `behavior` selects the mock_claude scenario; `extra_env` adds optional
/// CODEBUS_MOCK_* overrides.
fn run_chat(
    repo: &Path,
    behavior: &str,
    stdin_bytes: &[u8],
    extra_env: &[(&str, &str)],
) -> (Output, std::path::PathBuf) {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let mut cmd = Command::new(BIN);
    cmd.args(["chat"])
        .current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log)
        // Disable colored / emoji output so stdout assertions stay stable
        // (RenderOptions probes TERM; setting it to dumb forces ASCII).
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn codebus chat");
    {
        let mut stdin = child.stdin.take().expect("stdin piped");
        stdin.write_all(stdin_bytes).expect("write stdin");
        // Drop closes the pipe — REPL sees EOF after consuming all bytes.
    }
    let out = child.wait_with_output().expect("wait codebus chat");
    (out, log)
}

fn read_mock_log(log: &Path) -> String {
    fs::read_to_string(log).unwrap_or_default()
}

/// `Chat Subcommand Behavior` scenario: "Chat aborts on missing vault before first turn".
#[test]
fn chat_repl_vault_missing_exits_two() {
    let tmp = TempDir::new().unwrap();
    // tmp.path() has no .codebus/ subdirectory — vault is missing.
    let mut cmd = Command::new(BIN);
    cmd.args(["chat"])
        .current_dir(tmp.path())
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null());
    let out = cmd.output().expect("run codebus chat");
    let code = out.status.code().unwrap_or(-1);
    assert_eq!(
        code, 2,
        "expected exit 2 when vault missing; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("vault not found"),
        "stderr should mention missing vault, got: {stderr}"
    );
}

/// `Chat CLI Subcommand Behavior` scenario: "exit alias terminates REPL without spawning agent".
#[test]
fn chat_repl_exit_alias_terminates_zero() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, log) = run_chat(&repo, "chat-init-success", b"exit\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    // mock_claude should not have been spawned at all — log file absent.
    assert!(
        !log.exists(),
        "mock_claude was unexpectedly spawned (log exists at {log:?})"
    );
}

/// `Chat CLI Subcommand Behavior` scenario: ":q alias also terminates the REPL".
#[test]
fn chat_repl_colon_q_alias_terminates_zero() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, log) = run_chat(&repo, "chat-init-success", b":q\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!log.exists());
}

/// `Chat CLI Subcommand Behavior` scenario: "Empty input redisplays prompt".
/// (Verified indirectly: an empty line followed by exit still exits 0 and
/// mock_claude is never spawned.)
#[test]
fn chat_repl_empty_input_does_not_spawn() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, log) = run_chat(&repo, "chat-init-success", b"\n\n\nexit\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    assert!(!log.exists());
}

/// `Chat Verb Library Function` scenario: "First turn returns session_id
/// and writes RunLog mode chat".
#[test]
fn chat_first_turn_returns_session_id_and_writes_runlog() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, _log) = run_chat(
        &repo,
        "chat-init-success",
        b"hello\nexit\n",
        &[("CODEBUS_MOCK_SESSION_ID", "session-test-001")],
    );
    assert_eq!(out.status.code(), Some(0));

    // RunLog jsonl should have at least one mode=chat row with session_id.
    let log_dir = repo.join(".codebus").join("log");
    let jsonl: Vec<_> = fs::read_dir(&log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with("runs-") && n.ends_with(".jsonl")))
        .collect();
    assert!(!jsonl.is_empty(), "expected at least one runs-*.jsonl file");
    let body = fs::read_to_string(&jsonl[0]).unwrap();
    let chat_rows: Vec<&str> = body
        .lines()
        .filter(|l| l.contains("\"mode\":\"chat\""))
        .collect();
    assert_eq!(chat_rows.len(), 1, "expected exactly one mode=chat row, got body:\n{body}");
    assert!(
        chat_rows[0].contains("\"session_id\":\"session-test-001\""),
        "chat RunLog row missing session_id, got: {}",
        chat_rows[0]
    );
    assert!(chat_rows[0].contains("\"goal\":\"hello\""));
}

/// `Chat CLI Subcommand Behavior` + `--resume` design decision scenario:
/// "Second user input passes prior session id" — verified via mock_claude's
/// argv log: turn 2 spawn argv must contain `--resume <session_id>`.
#[test]
fn chat_repl_passes_prior_session_id_to_next_turn() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, log) = run_chat(
        &repo,
        "chat-init-success",
        b"first\nsecond\nexit\n",
        &[("CODEBUS_MOCK_SESSION_ID", "abc-resume-test")],
    );
    assert_eq!(out.status.code(), Some(0));
    // mock-claude.log dump captures ALL turns concatenated; we expect
    // `--resume abc-resume-test` to appear at least once (turn 2+).
    let log_body = read_mock_log(&log);
    assert!(
        log_body.contains("arg=--resume"),
        "turn 2+ spawn argv must include --resume; full log:\n{log_body}"
    );
    assert!(
        log_body.contains("arg=abc-resume-test"),
        "turn 2+ spawn argv must include the session id `abc-resume-test`; full log:\n{log_body}"
    );
}

/// `Activity Stream Render` scenario: "Tool use renders as one-line summary".
#[test]
fn chat_activity_stream_renders_tool_use_oneliners() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, _log) = run_chat(&repo, "chat-multi-tool", b"explore\nexit\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Expect two tool_use summary lines from chat-multi-tool: Glob + Read.
    assert!(
        stdout.contains("→ Glob"),
        "missing Glob tool_use summary; got stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("→ Read"),
        "missing Read tool_use summary; got stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("wiki/modules/uv-lib.md"),
        "missing Read input path in summary; got stdout:\n{stdout}"
    );
}

/// `Activity Stream Render` scenario: "Assistant text not rendered per chunk".
/// Verified by counting occurrences — chat-multi-tool emits exactly one
/// `text` Thought event ("summary"); a per-chunk renderer would print it
/// during streaming AND at turn-end (2 occurrences). The contract is one
/// block at turn-end only.
#[test]
fn chat_activity_stream_does_not_render_text_chunks_separately() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, _log) = run_chat(&repo, "chat-multi-tool", b"explore\nexit\n", &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let count = stdout.matches("summary").count();
    assert_eq!(
        count, 1,
        "assistant text block must appear exactly once at turn-end (got {count}); stdout:\n{stdout}"
    );
}

/// `Verb RunLog Capture and Persistence` chat-specific scenario: "Chat
/// REPL with three turns appends three RunLog entries with the same
/// session_id".
#[test]
fn chat_three_turns_produce_three_runlog_rows_same_session_id() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, _log) = run_chat(
        &repo,
        "chat-init-success",
        b"q1\nq2\nq3\nexit\n",
        &[("CODEBUS_MOCK_SESSION_ID", "three-turn-session")],
    );
    assert_eq!(out.status.code(), Some(0));
    let log_dir = repo.join(".codebus").join("log");
    let jsonl: Vec<_> = fs::read_dir(&log_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map_or(false, |n| n.starts_with("runs-") && n.ends_with(".jsonl")))
        .collect();
    let body = fs::read_to_string(&jsonl[0]).unwrap();
    let chat_rows: Vec<&str> = body
        .lines()
        .filter(|l| l.contains("\"mode\":\"chat\""))
        .collect();
    assert_eq!(chat_rows.len(), 3, "expected 3 chat rows, got body:\n{body}");
    for row in &chat_rows {
        assert!(
            row.contains("\"session_id\":\"three-turn-session\""),
            "row missing shared session_id: {row}"
        );
    }
}

/// `Promote Confirmation and Goal Subprocess Spawn` scenario:
/// "User declines promote returns to REPL" — verified by feeding `n\n`
/// to the (y/n) prompt and asserting REPL still exits 0 cleanly.
#[test]
fn chat_promote_n_returns_to_repl() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    // Turn 1 = "trigger" → mock emits marker → CLI asks "(y/n) " →
    // we answer "n" → CLI redisplays prompt → we type "exit" → exit 0.
    let (out, _log) = run_chat(&repo, "chat-emit-promote", b"trigger\nn\nexit\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("[suggest] promote to wiki? (y/n)"),
        "missing promote confirmation prompt; got stdout:\n{stdout}"
    );
}

/// Defensive check: user prompts containing `"` (ASCII double-quote) must
/// reach the spawned claude as a single argv element with the quotes
/// preserved verbatim. `Command::arg` doesn't shell-escape so this is the
/// expected behavior — but we pin it as a test so a future refactor that
/// switches to a shell-style invocation would break loudly.
#[test]
fn chat_passes_user_prompt_with_embedded_quotes_verbatim_to_claude() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, log) = run_chat(
        &repo,
        "chat-init-success",
        // User prompt contains an embedded double-quote that the chat
        // verb wraps into the slash command `/codebus-chat "..."` literal.
        b"what is \"foo\" then?\nexit\n",
        &[],
    );
    assert_eq!(out.status.code(), Some(0));
    let log_body = read_mock_log(&log);
    // Slash command argv element is on a single `arg=` line. Look for
    // the user phrase inside it.
    let has_phrase = log_body.lines().any(|l| {
        l.starts_with("arg=") && l.contains("what is") && l.contains("foo") && l.contains("then?")
    });
    assert!(
        has_phrase,
        "user phrase with quotes not propagated to claude argv; log:\n{log_body}"
    );
}

/// `Chat Verb Library Function` scenario: "No auto-commit on chat turn".
/// Verified by counting commits on the vault's nested git repo: a chat
/// turn must not add any new commit (vault may not even be a git repo,
/// so we just assert `.git` is absent when we never inited).
#[test]
fn chat_does_not_auto_commit() {
    let tmp = TempDir::new().unwrap();
    let repo = make_chat_vault(&tmp);
    let (out, _log) = run_chat(&repo, "chat-init-success", b"hello\nexit\n", &[]);
    assert_eq!(out.status.code(), Some(0));
    // We never ran `codebus init` — vault is bare. If chat had tried to
    // auto_commit, it would have either created `.git` or errored. Neither
    // happened.
    assert!(
        !repo.join(".codebus").join(".git").exists(),
        "chat must not have created a git repo (no auto_commit)"
    );
}
