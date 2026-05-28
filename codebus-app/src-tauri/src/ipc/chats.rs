//! Chat-turn IPC commands.
//!
//! Spec touchpoints (v3-app-chat-cmdk):
//! - `app-workspace § Tauri IPC Commands for Chat Turn Lifecycle`
//! - `app-workspace § One Active Goal Run At A Time` (chat coexists with goal)
//! - design `IPC Surface — Mirror goals.rs Pattern, Separate chat-stream Channel`
//! - design `Concurrency — Chat Turn 與 Goal Run 可同時存在於 active_runs`
//!
//! Surface:
//! - `spawn_chat_turn(vault_path, text, session_id)` — start a background
//!   `run_chat_turn` thread, emit each `VerbEvent` to the `chat-stream`
//!   Tauri event with `{ run_id, event }` payload, return the new `RunId`
//!   formatted as `chat-<started_at slug>` so callers can distinguish
//!   chat turns from goal runs in the same `active_runs` map.
//! - `cancel_chat_turn(run_id)` — idempotent cooperative cancel; preserves
//!   the chat session_id so the next turn can `--resume <id>`.
//!
//! Concurrency:
//! - Two chat turns in the same vault MUST NOT coexist; `spawn_chat_turn`
//!   rejects with `AppError::Invalid { field: "active_runs", message: "...
//!   another chat turn is already active in this session" }` when
//!   `active_runs.has_chat_turn_for_vault(vault)` returns true.
//! - A chat turn CAN coexist with an active goal run because chat is
//!   read-only (`CHAT_TOOLSET` excludes Write/Edit at the binary layer).
//!
//! Testability:
//! - `spawn_chat_turn_with_runner` is the inner helper Tauri command and
//!   unit tests share; tests pass a stub `runner` closure to avoid the
//!   need for a real `claude` binary in CI.

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;

use codebus_core::verb::chat::{run_chat_turn, ChatTurnOptions, ChatTurnReport};
use codebus_core::verb::error::VerbError;
use codebus_core::verb::VerbEvent;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::IpcResult;
use crate::error::AppError;
use crate::state::active_runs::ActiveRuns;
use crate::state::app_state::AppRuntimeState;

/// Payload pushed to the `chat-stream` Tauri event channel per
/// `VerbEvent` emitted by the running chat-turn thread. Mirrors
/// `GoalStreamPayload` shape — frontend listeners SHALL distinguish
/// chat vs goal flows by channel name (`chat-stream` vs `goal-stream`)
/// rather than by inspecting the `run_id` prefix.
#[derive(Debug, Clone, Serialize)]
pub struct ChatStreamPayload {
    pub run_id: String,
    pub event: VerbEvent,
}

/// Payload pushed to the `chat-terminal` Tauri event channel exactly
/// once per chat turn, after the background thread exits regardless of
/// outcome (success / failure / cancel / panic). Frontend uses this to
/// flip `useChatStore.activeTurn` back to `null`, finalize the turn in
/// the transcript, AND record the claude `session_id` so the next
/// `spawn_chat_turn` call can pass it back for `--resume <id>`.
///
/// `session_id` is `None` on terminal paths that never reached the
/// init phase (e.g., spawn failure before stream-json's init event);
/// frontend treats `None` as "session not advanced" and keeps any
/// previously known sessionId.
#[derive(Debug, Clone, Serialize)]
pub struct ChatTerminalPayload {
    pub run_id: String,
    pub session_id: Option<String>,
    pub outcome: ChatTurnOutcome,
}

/// Coarse outcome classification surfaced to the frontend. Matches the
/// outcomes the chat-verb library returns: `succeeded` when `run_chat_turn`
/// returns `Ok(report)`, `cancelled` on `Err(VerbError::Cancelled)`,
/// `failed` for any other `Err` AND any panic intercepted by
/// `catch_unwind`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurnOutcome {
    Succeeded,
    Cancelled,
    Failed,
}

// ---- spawn_chat_turn ------------------------------------------------------

/// Tauri command wrapper. The real cross-thread orchestration lives in
/// [`spawn_chat_turn_with_runner`] so tests can inject a stub `runner`
/// that avoids the need for a real `claude` binary.
#[tauri::command]
pub fn spawn_chat_turn(
    app: AppHandle,
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    text: String,
    session_id: Option<String>,
) -> IpcResult<String> {
    let active_runs = runtime.active_runs.clone();
    let app_for_stream = app.clone();
    let app_for_terminal = app.clone();
    spawn_chat_turn_with_runner(
        active_runs,
        PathBuf::from(vault_path),
        text,
        session_id,
        move |payload| {
            // emit MAY fail if the main window is gone (e.g., user closed
            // the app mid-turn). Same rationale as goals.rs: ignore — the
            // thread still cleans up its active_runs entry.
            let _ = app_for_stream.emit("chat-stream", payload);
        },
        move |payload| {
            let _ = app_for_terminal.emit("chat-terminal", payload);
        },
        |repo, options, mut on_event, cancel| {
            run_chat_turn(repo, options, |e| on_event(e), cancel)
        },
    )
}

/// Inner helper for `spawn_chat_turn`. Performs the one-active-chat-turn
/// check, allocates the `RunId` + cancel flag (`chat-<slug>` keyed),
/// registers the cancel flag in `active_runs`, and spawns a background
/// thread that:
/// 1. invokes `runner(...)` (a stand-in for `run_chat_turn`), wired so
///    each `VerbEvent` it emits flows through `emit` as a
///    `ChatStreamPayload`.
/// 2. on thread completion (success / failure / cancel / panic), removes
///    the entry from `active_runs` AND emits a `chat-terminal` event.
///
/// Returns the new `RunId` synchronously so the frontend can attach a
/// listener to `chat-stream` filtered by `run_id` before any event arrives.
pub(crate) fn spawn_chat_turn_with_runner<E, T, F>(
    active_runs: Arc<ActiveRuns>,
    vault_path: PathBuf,
    text: String,
    session_id: Option<String>,
    emit: E,
    emit_terminal: T,
    runner: F,
) -> Result<String, AppError>
where
    E: Fn(ChatStreamPayload) + Send + 'static,
    T: Fn(ChatTerminalPayload) + Send + 'static,
    F: FnOnce(
            &Path,
            ChatTurnOptions,
            Box<dyn FnMut(VerbEvent) + Send>,
            Option<Arc<AtomicBool>>,
        ) -> Result<ChatTurnReport, VerbError>
        + Send
        + 'static,
{
    // Spec: app-workspace § Tauri IPC Commands for Chat Turn Lifecycle +
    // § Cross-Vault Goal Spawn Permitted (symmetric vault scope for chat) —
    // one active chat turn per (session, vault). Goal-mode entries under
    // the same vault DO NOT block; chat entries under other vaults DO NOT
    // block either.
    let vault_str = vault_path.to_string_lossy();
    if active_runs.has_chat_turn_for_vault(&vault_str) {
        return Err(AppError::Invalid {
            field: "active_runs".into(),
            message: "another chat turn is already active in this session".into(),
        });
    }

    // RunId = `chat-<started_at slug>` per spec scenario "spawn_chat_turn
    // returns chat run id" (e.g., "chat-2026-05-14T10-20-30Z").
    let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let run_id = format!("chat-{}", started_at.replace(':', "-"));

    let cancel = Arc::new(AtomicBool::new(false));
    active_runs.insert(&vault_str, run_id.clone(), cancel.clone());

    let active_runs_thread = active_runs.clone();
    let run_id_thread = run_id.clone();
    let cancel_thread = cancel.clone();

    thread::Builder::new()
        .name(format!("chat-turn-{run_id}"))
        .spawn(move || {
            let run_id_for_event = run_id_thread.clone();
            let emit_for_thread = emit;
            let on_event: Box<dyn FnMut(VerbEvent) + Send> =
                Box::new(move |event: VerbEvent| {
                    emit_for_thread(ChatStreamPayload {
                        run_id: run_id_for_event.clone(),
                        event,
                    });
                });

            let options = ChatTurnOptions { text, session_id };

            // Capture the runner's outcome so the terminal payload can
            // surface session_id (for next-turn --resume) AND the coarse
            // outcome to the frontend.
            let result: std::thread::Result<Result<ChatTurnReport, VerbError>> =
                std::panic::catch_unwind(AssertUnwindSafe(|| {
                    runner(&vault_path, options, on_event, Some(cancel_thread))
                }));

            let (session_id, outcome) = match result {
                Ok(Ok(report)) => (Some(report.session_id), ChatTurnOutcome::Succeeded),
                Ok(Err(VerbError::Cancelled)) => (None, ChatTurnOutcome::Cancelled),
                Ok(Err(_)) => (None, ChatTurnOutcome::Failed),
                Err(_) => (None, ChatTurnOutcome::Failed),
            };

            active_runs_thread.remove(&run_id_thread);
            // Notify frontend that the turn has fully terminated so it can
            // flip `activeTurn` to null, finalize the transcript, AND
            // record session_id for the next turn's `--resume`.
            emit_terminal(ChatTerminalPayload {
                run_id: run_id_thread.clone(),
                session_id,
                outcome,
            });
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn chat thread: {e}"),
        })?;

    Ok(run_id)
}

// ---- cancel_chat_turn -----------------------------------------------------

/// Tauri command wrapper for cancel. Delegates to the helper so tests
/// can drive the cancel path without constructing a `tauri::State`.
#[tauri::command]
pub fn cancel_chat_turn(runtime: State<'_, AppRuntimeState>, run_id: String) -> IpcResult<()> {
    cancel_chat_turn_impl(&runtime.active_runs, &run_id)
}

/// Helper: flip the cancel flag for `run_id` when present, otherwise
/// no-op. The idempotent "no-op when missing" branch matches the spec
/// scenario `cancel_chat_turn idempotent on unknown run` — a cancel
/// call racing with thread completion MUST still resolve `Ok(())`.
pub(crate) fn cancel_chat_turn_impl(
    active_runs: &ActiveRuns,
    run_id: &str,
) -> Result<(), AppError> {
    if let Some(flag) = active_runs.get(run_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::log::sink::TokenUsage;
    use std::sync::mpsc;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    fn fake_report() -> ChatTurnReport {
        ChatTurnReport {
            session_id: "test-session-id".into(),
            accumulated_tokens: TokenUsage::default(),
            started_at: "2026-05-14T10:20:30Z".into(),
            finished_at: "2026-05-14T10:20:31Z".into(),
            agent_exit_code: Some(0),
        }
    }

    /// Spec scenario: "spawn_chat_turn returns chat run id".
    #[test]
    fn spawn_chat_turn_returns_chat_prefixed_run_id() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();

        let (terminal_tx, terminal_rx) = mpsc::sync_channel::<String>(1);
        let temp = tempfile::TempDir::new().unwrap();
        let run_id = spawn_chat_turn_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "hello".into(),
            None,
            |_payload| {},
            move |payload| {
                let _ = terminal_tx.send(payload.run_id);
            },
            |_repo, _opts, _on_event, _cancel| Ok(fake_report()),
        )
        .expect("spawn ok");

        assert!(
            run_id.starts_with("chat-"),
            "run_id MUST be `chat-` prefixed; got {run_id}"
        );
        let vault_str = temp.path().to_string_lossy().into_owned();
        assert!(
            active_runs.has_chat_turn_for_vault(&vault_str),
            "active_runs SHALL contain the chat entry while the thread runs"
        );
        // Wait for terminal emit.
        let terminal_id = terminal_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("emit_terminal SHALL fire after runner completes");
        assert_eq!(terminal_id, run_id);
        // Active_runs cleared.
        let deadline = Instant::now() + Duration::from_secs(2);
        while active_runs.has_chat_turn_for_vault(&vault_str) {
            if Instant::now() > deadline {
                panic!("active_runs entry not removed after runner completed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Spec scenario: "spawn_chat_turn rejects when chat turn already active".
    #[test]
    fn spawn_chat_turn_rejects_when_chat_already_active() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();
        let temp = tempfile::TempDir::new().unwrap();
        let vault_str = temp.path().to_string_lossy().into_owned();
        active_runs.insert(
            &vault_str,
            "chat-existing".into(),
            Arc::new(AtomicBool::new(false)),
        );

        let err = spawn_chat_turn_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "x".into(),
            None,
            |_p| {},
            |_terminal| {},
            |_repo, _opts, _on_event, _cancel| Ok(fake_report()),
        )
        .expect_err("second chat spawn MUST be rejected");
        match err {
            AppError::Invalid { field, message } => {
                assert_eq!(field, "active_runs");
                assert!(
                    message.contains("chat turn is already active"),
                    "message should mention `chat turn is already active`: {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    /// Spec scenario: "chat-stream events forwarded with run_id payload".
    #[test]
    fn chat_stream_events_carry_run_id_payload() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();

        // Capture all stream payloads emitted by the helper.
        let captured: Arc<Mutex<Vec<ChatStreamPayload>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured.clone();

        let temp = tempfile::TempDir::new().unwrap();
        let run_id = spawn_chat_turn_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "test".into(),
            None,
            move |payload| {
                captured_clone.lock().unwrap().push(payload);
            },
            |_terminal| {},
            |_repo, _opts, mut on_event, _cancel| {
                // Emit one synthetic lifecycle event so the test has
                // something observable to assert about payload shape.
                use codebus_core::verb::event::VerbLifecycleEvent;
                on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
                    verb: codebus_core::config::Verb::Chat,
                }));
                Ok(fake_report())
            },
        )
        .expect("spawn ok");

        // Wait until the runner thread reaches completion (active_runs
        // cleanup is the most reliable signal).
        let vault_str = temp.path().to_string_lossy().into_owned();
        let deadline = Instant::now() + Duration::from_secs(2);
        while active_runs.has_chat_turn_for_vault(&vault_str) {
            if Instant::now() > deadline {
                panic!("runner thread did not finish");
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let payloads = captured.lock().unwrap();
        assert!(
            !payloads.is_empty(),
            "at least one ChatStreamPayload SHALL be emitted"
        );
        for p in payloads.iter() {
            assert_eq!(p.run_id, run_id, "every payload SHALL carry the spawn run_id");
        }
    }

    /// Spec scenario: "cancel_chat_turn idempotent on unknown run".
    #[test]
    fn cancel_chat_turn_idempotent_on_unknown_run() {
        let runtime = AppRuntimeState::new();
        cancel_chat_turn_impl(&runtime.active_runs, "chat-nonexistent")
            .expect("MUST succeed idempotently when run id unknown");
    }

    /// cancel_chat_turn flips the stored cancel flag for an active run.
    /// This is the GUI-side support for the spec scenario "Cancelled chat
    /// turn preserves session id for next turn" — the cancel flag flip
    /// is the backend side; session_id preservation is asserted at the
    /// store layer (see `useChatStore` tests in store/chat.test.ts).
    #[test]
    fn cancel_chat_turn_flips_existing_flag() {
        let runtime = AppRuntimeState::new();
        let flag = Arc::new(AtomicBool::new(false));
        runtime
            .active_runs
            .insert("/vault/test", "chat-run-x".into(), flag.clone());

        cancel_chat_turn_impl(&runtime.active_runs, "chat-run-x").expect("ok");
        assert!(
            flag.load(std::sync::atomic::Ordering::Relaxed),
            "cancel flag MUST be set after cancel_chat_turn"
        );
    }

    /// Spec scenario: "Chat turn does not block concurrent goal spawn"
    /// — chat-only side of the assertion. (The goal-spawn side is asserted
    /// in goals.rs `spawn_goal_succeeds_with_concurrent_chat_turn`.)
    /// Here we confirm that spawning a chat turn does not register as a
    /// goal run, so the symmetric goal spawn that runs concurrently is
    /// not rejected by `has_goal_run_for_vault`.
    #[test]
    fn chat_turn_does_not_register_as_goal_run() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();

        let (start_tx, start_rx) = mpsc::sync_channel::<()>(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);

        let runner = move |_repo: &Path,
                           _opts: ChatTurnOptions,
                           _on_event: Box<dyn FnMut(VerbEvent) + Send>,
                           _cancel: Option<Arc<AtomicBool>>| {
            start_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(fake_report())
        };

        let temp = tempfile::TempDir::new().unwrap();
        let _run_id = spawn_chat_turn_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "x".into(),
            None,
            |_p| {},
            |_terminal| {},
            runner,
        )
        .expect("chat spawn ok");

        start_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("runner SHALL reach start signal");

        // Critical assertion: chat is active, but `has_goal_run_for_vault`
        // is false so a concurrent `spawn_goal` under the same vault would
        // succeed.
        let vault_str = temp.path().to_string_lossy().into_owned();
        assert!(active_runs.has_chat_turn_for_vault(&vault_str));
        assert!(
            !active_runs.has_goal_run_for_vault(&vault_str),
            "chat entry MUST NOT register as a goal run"
        );

        release_tx.send(()).unwrap();
        // Wait for cleanup.
        let deadline = Instant::now() + Duration::from_secs(2);
        while active_runs.has_chat_turn_for_vault(&vault_str) {
            if Instant::now() > deadline {
                panic!("chat entry not removed after runner completed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
