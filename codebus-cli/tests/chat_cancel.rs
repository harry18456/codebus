//! Library-level cancel signal verification for `verb::chat::run_chat_turn`.
//!
//! This is the only direct test of the cancel polling path that
//! v3-chat-verb introduces at the CLI (mid-turn Ctrl+C → flip
//! `AtomicBool` → invoke kills child → `Err(VerbError::Cancelled)`).
//! Mock_claude scenarios drive timing: `chat-trickle-cancel` emits the
//! init line, then sleeps for 800ms before the next line, giving the
//! test thread a deterministic window to flip the cancel flag while the
//! parent's `BufReader::lines()` is blocked waiting for more input.
//!
//! NOTE: this test mutates process-wide env (`CODEBUS_CLAUDE_BIN`,
//! `CODEBUS_HOME`, `CODEBUS_MOCK_*`) because `agent::invoke` reads them
//! at spawn time. Cargo runs each `tests/*.rs` file in its own test
//! binary, so the mutation is safe AS LONG AS this file contains only
//! ONE test function (cargo parallelizes test fns WITHIN a binary).
//! If you add a second env-mutating test here, switch to `--test-threads=1`
//! or guard with a `Mutex` instead.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use codebus_core::verb::VerbError;
use codebus_core::verb::chat::{ChatTurnOptions, run_chat_turn};
use tempfile::TempDir;

const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

#[test]
fn run_chat_turn_observes_cancel_and_returns_cancelled() {
    // 1. Build minimal vault so chat-verb's vault precondition passes.
    let tmp = TempDir::new().expect("tempdir");
    let repo = tmp.path().to_path_buf();
    let vault = repo.join(".codebus");
    std::fs::create_dir_all(vault.join("log")).expect("create vault log dir");

    // Isolated CODEBUS_HOME so this test never reads the user's real config.
    let home = TempDir::new().expect("isolated codebus home");

    // 2. Mutate process-wide env (safe because this file has ONE test —
    // see the file-level NOTE for the contract).
    // SAFETY: single-threaded test, no concurrent readers.
    unsafe {
        std::env::set_var("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE);
        std::env::set_var("CODEBUS_HOME", home.path());
        std::env::set_var("CODEBUS_MOCK_BEHAVIOR", "chat-trickle-cancel");
        std::env::set_var(
            "CODEBUS_MOCK_SESSION_ID",
            "cancel-test-session-id-0001",
        );
        // No CODEBUS_MOCK_LOG — not needed for cancel path verification.
    }

    // 3. Spawn run_chat_turn on a worker thread so the main thread can
    // flip the cancel flag mid-stream.
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_worker = cancel.clone();
    let repo_for_worker = repo.clone();
    let start = Instant::now();
    let handle: thread::JoinHandle<Result<_, VerbError>> = thread::spawn(move || {
        run_chat_turn(
            &repo_for_worker,
            ChatTurnOptions {
                text: "test".into(),
                session_id: None,
            },
            |_event| {
                // Silently consume events; the test only cares about
                // the eventual Err(Cancelled) return path.
            },
            Some(cancel_for_worker),
            None,
        )
    });

    // 4. Wait long enough for the worker to consume the init line and
    // start blocking on the next line (mock sleeps 800ms). 250ms gives
    // a comfortable margin without blowing test wall time.
    thread::sleep(Duration::from_millis(250));
    cancel.store(true, Ordering::Relaxed);

    // 5. Join with a generous timeout — mock_claude's full trickle takes
    // ~1.6s if uninterrupted; with the kill-on-cancel path the worker
    // should return well under 2s.
    let timeout = Duration::from_secs(5);
    let poll = Duration::from_millis(50);
    while !handle.is_finished() && start.elapsed() < timeout {
        thread::sleep(poll);
    }
    assert!(
        handle.is_finished(),
        "run_chat_turn did not return within {timeout:?} after cancel"
    );

    let result = handle.join().expect("worker thread panicked");

    // 6. Restore process env BEFORE asserting (so even on assert panic,
    // other tests running in different binaries see a clean env).
    // SAFETY: still single-threaded — no concurrent readers.
    unsafe {
        std::env::remove_var("CODEBUS_CLAUDE_BIN");
        std::env::remove_var("CODEBUS_HOME");
        std::env::remove_var("CODEBUS_MOCK_BEHAVIOR");
        std::env::remove_var("CODEBUS_MOCK_SESSION_ID");
    }

    // 7. Verify spec scenario:
    //    "Cancel mid-turn returns VerbError Cancelled"
    //    (chat-verb spec § Chat Verb Library Function)
    match result {
        Err(VerbError::Cancelled) => {
            // expected
        }
        Err(other) => panic!(
            "expected Err(VerbError::Cancelled), got Err({other:?}); \
             the cancel signal path is wired up but didn't fire as expected"
        ),
        Ok(report) => panic!(
            "expected Err(VerbError::Cancelled) but run_chat_turn returned Ok({report:?}); \
             cancel flag was flipped 250ms after spawn but the cancel poll never observed it. \
             Likely causes: (a) mock_claude finished before cancel flipped (trickle timing too \
             fast), (b) BufReader read returned EOF before flag check, (c) cancel polling logic \
             regressed in agent::invoke"
        ),
    }

    // 8. Verify a cancelled RunLog row was persisted to the vault.
    // Per chat-verb spec: cancel path writes RunLog with
    // outcome="cancelled" + session_id=Some(<init_session_id>) BEFORE
    // returning Err(VerbError::Cancelled).
    let log_dir = vault.join("log");
    let jsonl_paths: Vec<_> = std::fs::read_dir(&log_dir)
        .expect("read log dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with("runs-") && n.ends_with(".jsonl"))
        })
        .collect();
    assert!(
        !jsonl_paths.is_empty(),
        "no runs-*.jsonl file produced — cancel path failed to persist RunLog"
    );
    let body = std::fs::read_to_string(&jsonl_paths[0]).expect("read jsonl");
    let cancelled_rows: Vec<&str> = body
        .lines()
        .filter(|l| l.contains("\"outcome\":\"cancelled\"") && l.contains("\"mode\":\"chat\""))
        .collect();
    assert_eq!(
        cancelled_rows.len(),
        1,
        "expected exactly one cancelled chat RunLog row, got body:\n{body}"
    );
    assert!(
        cancelled_rows[0].contains("\"session_id\":\"cancel-test-session-id-0001\""),
        "cancelled RunLog row missing init-event session_id, got: {}",
        cancelled_rows[0]
    );
}
