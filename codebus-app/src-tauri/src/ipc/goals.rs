//! Goal lifecycle IPC commands.
//!
//! Spec touchpoints (v3-app-workspace-goal):
//! - `app-workspace § Tauri IPC Commands for Goal Lifecycle and Wiki Read`
//! - `app-workspace § Interrupted Run Detection`
//! - `app-workspace § One Active Goal Run At A Time`
//! - design `IPC 6 個 commands、Tauri event 1 個 channel`
//! - design `Active runs 狀態存 AppState.active_runs`
//! - design `RunId 用 timestamp slug，不引入 UUID`
//! - design `Interrupted run 偵測 — events 有 / RunLog 無`
//!
//! Surface:
//! - `spawn_goal(vault_path, goal_text)` — start a background `run_goal`
//!   thread, emit each `VerbEvent` to the `goal-stream` Tauri event with
//!   `{ run_id, event }` payload, return the new `RunId` (= `started_at`
//!   slug per the design decision).
//! - `cancel_goal(run_id)` — idempotent cooperative cancel.
//! - `list_runs(vault_path, mode_filter)` — read `runs-*.jsonl`, filter
//!   by mode, merge interrupted virtual entries from orphan
//!   `events-*.jsonl` files, return sorted descending.
//! - `get_run_detail(vault_path, run_id)` — load the matching summary
//!   (real or virtual) plus a one-shot tail-replay of the corresponding
//!   `events-*.jsonl`.
//!
//! Cross-thread isolation:
//! - The goal thread is spawned via `std::thread::Builder::spawn`; its
//!   body is wrapped in `catch_unwind` so a verb panic does not poison
//!   `active_runs` cleanup or kill the Tauri host process.
//! - The cleanup path (`active_runs.remove`) ALWAYS runs regardless of
//!   verb outcome (Ok / Err / panic).
//!
//! Testability:
//! - `spawn_goal_with_runner` is the inner helper Tauri command and
//!   unit tests share; tests pass a stub `runner` closure to avoid the
//!   need for a real `claude` binary in CI.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;

use codebus_core::log::events::sink::EventEnvelope;
use codebus_core::log::sink::{RunLog, TokenUsage};
use codebus_core::verb::error::VerbError;
use codebus_core::verb::event::VerbBanner;
use codebus_core::verb::goal::{run_goal, GoalOptions, GoalReport};
use codebus_core::verb::VerbEvent;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use super::IpcResult;
use crate::error::AppError;
use crate::state::active_runs::ActiveRuns;
use crate::state::app_state::AppRuntimeState;

/// Filter selector for `list_runs`. Serialized as a tagged enum so the
/// frontend writes `{ kind: "goal" }` / `{ kind: "all" }`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModeFilter {
    Goal,
    All,
}

/// IPC-shaped projection of one `RunLog` row plus the derived `run_id`
/// (= `started_at` slug). Virtual interrupted entries also project into
/// this shape, with `outcome == "interrupted"` and zero token / lint
/// counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLogSummary {
    pub run_id: String,
    pub mode: String,
    pub goal: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    pub tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// Full run detail: the summary plus an in-order replay of the events
/// file. v1 reads the whole events file synchronously (typical run is
/// < 200 events / 500 KB, per the design Risks section).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetail {
    pub summary: RunLogSummary,
    pub events: Vec<EventEnvelope>,
}

/// Payload pushed to the `goal-stream` Tauri event channel per
/// `VerbEvent` emitted by the running goal thread.
#[derive(Debug, Clone, Serialize)]
pub struct GoalStreamPayload {
    pub run_id: String,
    pub event: VerbEvent,
}

/// Payload pushed to the `goal-terminal` Tauri event channel exactly
/// once per spawn, after the background thread exits regardless of
/// outcome (success / failure / cancel / panic). Frontend uses this
/// to flip `useGoalsStore.activeRun` back to `null` and refresh the
/// `runs` list from disk so the newly-written RunLog row is picked up.
#[derive(Debug, Clone, Serialize)]
pub struct GoalTerminalPayload {
    pub run_id: String,
}

// ---- spawn_goal -----------------------------------------------------------

/// Tauri command wrapper. The real cross-thread orchestration lives in
/// [`spawn_goal_with_runner`] so tests can inject a stub `runner` that
/// avoids the need for a real `claude` binary.
#[tauri::command]
pub fn spawn_goal(
    app: AppHandle,
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    goal_text: String,
) -> IpcResult<String> {
    let active_runs = runtime.active_runs.clone();
    let app_for_stream = app.clone();
    let app_for_terminal = app.clone();
    spawn_goal_with_runner(
        active_runs,
        PathBuf::from(vault_path),
        goal_text,
        move |payload| {
            // `emit` may fail when the main window is gone (e.g., user
            // closed the app mid-run). It is correct to ignore the
            // error — the thread will still clean up its active_runs
            // entry on completion.
            let _ = app_for_stream.emit("goal-stream", payload);
        },
        move |payload| {
            let _ = app_for_terminal.emit("goal-terminal", payload);
        },
        |repo, options, mut on_event, cancel| {
            run_goal(repo, options, |e| on_event(e), cancel)
        },
    )
}

/// Inner helper for `spawn_goal`. Performs the one-active-run check,
/// allocates the `RunId` + cancel flag, registers the cancel flag in
/// `active_runs`, and spawns a background thread that:
/// 1. invokes `runner(...)` (a stand-in for `run_goal`), wired so each
///    `VerbEvent` it emits flows through `emit` as a `GoalStreamPayload`.
/// 2. on thread completion (success / failure / panic), removes the
///    entry from `active_runs`.
///
/// Returns the new `RunId` synchronously so the frontend can switch to
/// the Running detail view before any `goal-stream` event has arrived.
pub(crate) fn spawn_goal_with_runner<E, T, F>(
    active_runs: Arc<ActiveRuns>,
    vault_path: PathBuf,
    goal_text: String,
    emit: E,
    emit_terminal: T,
    runner: F,
) -> Result<String, AppError>
where
    E: Fn(GoalStreamPayload) + Send + 'static,
    T: Fn(GoalTerminalPayload) + Send + 'static,
    F: FnOnce(
            &Path,
            GoalOptions,
            Box<dyn FnMut(VerbEvent) + Send>,
            Option<Arc<AtomicBool>>,
        ) -> Result<GoalReport, VerbError>
        + Send
        + 'static,
{
    // Spec: app-workspace § One Active Goal Run At A Time — backend layer.
    // Chat turns (keyed `chat-<slug>`) SHALL NOT block goal spawn — chat
    // is read-only and cannot conflict with goal's writes, so the two
    // can coexist in `active_runs`. Only another goal-mode entry blocks.
    if active_runs.has_goal_run() {
        return Err(AppError::Invalid {
            field: "active_runs".into(),
            message: "another goal run is already active".into(),
        });
    }

    // RunId = started_at slug (per-second precision matches what
    // `run_goal` captures internally and what `EventsJsonlSink::new`
    // uses to derive the events file name).
    let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let run_id = started_at.replace(':', "-");

    let cancel = Arc::new(AtomicBool::new(false));
    active_runs.insert(run_id.clone(), cancel.clone());

    let active_runs_thread = active_runs.clone();
    let run_id_thread = run_id.clone();
    let cancel_thread = cancel.clone();

    thread::Builder::new()
        .name(format!("goal-{run_id}"))
        .spawn(move || {
            let run_id_for_event = run_id_thread.clone();
            let emit_for_thread = emit;
            let on_event: Box<dyn FnMut(VerbEvent) + Send> =
                Box::new(move |event: VerbEvent| {
                    emit_for_thread(GoalStreamPayload {
                        run_id: run_id_for_event.clone(),
                        event,
                    });
                });

            let options = GoalOptions {
                text: goal_text,
                force_resync: false,
                no_fix: false,
                // GUI does NOT touch Obsidian registry (the foundation
                // already left this opt-in to the user via Settings).
                no_obsidian_register: true,
            };

            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let _ = runner(&vault_path, options, on_event, Some(cancel_thread));
            }));

            active_runs_thread.remove(&run_id_thread);
            // Notify frontend that the run has fully terminated so it
            // can clear `activeRun` and re-read RunLog from disk.
            emit_terminal(GoalTerminalPayload {
                run_id: run_id_thread.clone(),
            });
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn goal thread: {e}"),
        })?;

    Ok(run_id)
}

// ---- cancel_goal ----------------------------------------------------------

/// Tauri command wrapper for cancel. Delegates to the helper so tests
/// can drive the cancel path without constructing a `tauri::State`.
#[tauri::command]
pub fn cancel_goal(runtime: State<'_, AppRuntimeState>, run_id: String) -> IpcResult<()> {
    cancel_goal_impl(&runtime.active_runs, &run_id)
}

/// Helper: flip the cancel flag for `run_id` when present, otherwise
/// no-op. The idempotent "no-op when missing" branch matches the spec
/// scenario `cancel_goal idempotent on unknown run` — a `cancel_goal`
/// call racing with thread completion must still resolve `Ok(())`.
pub(crate) fn cancel_goal_impl(active_runs: &ActiveRuns, run_id: &str) -> Result<(), AppError> {
    if let Some(flag) = active_runs.get(run_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

// ---- list_runs ------------------------------------------------------------

/// Tauri command wrapper. The directory layout is fixed:
/// `<vault>/.codebus/log/{runs-*.jsonl, events-*.jsonl}`.
#[tauri::command]
pub async fn list_runs(
    vault_path: String,
    mode_filter: ModeFilter,
) -> IpcResult<Vec<RunLogSummary>> {
    let log_dir = Path::new(&vault_path).join(".codebus").join("log");
    list_runs_impl(&log_dir, mode_filter)
}

/// Implementation-side `list_runs`. Scans `log_dir` for both
/// `runs-*.jsonl` (per-run summaries) and `events-*.jsonl` (per-event
/// timelines). Real rows are projected as `RunLogSummary`; orphan
/// events files (no matching `RunLog` row) are synthesized as virtual
/// `outcome="interrupted"` entries (per design
/// `Interrupted run 偵測 — events 有 / RunLog 無`).
///
/// Sorted by `started_at` descending so the freshest run appears first.
pub(crate) fn list_runs_impl(
    log_dir: &Path,
    mode_filter: ModeFilter,
) -> Result<Vec<RunLogSummary>, AppError> {
    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut runs_rows_by_slug: HashMap<String, RunLog> = HashMap::new();
    let mut events_slugs: HashSet<String> = HashSet::new();

    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        if name.starts_with("runs-") && name.ends_with(".jsonl") {
            let body = match fs::read_to_string(&path) {
                Ok(b) => b,
                Err(_) => continue,
            };
            for line in body.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<RunLog>(line) {
                    Ok(rl) => {
                        let slug = rl.started_at.replace(':', "-");
                        // If a later row repeats the same started_at,
                        // keep the most recent (last write wins).
                        runs_rows_by_slug.insert(slug, rl);
                    }
                    Err(_) => continue,
                }
            }
        } else if name.starts_with("events-") && name.ends_with(".jsonl") {
            let slug = name
                .strip_prefix("events-")
                .and_then(|s| s.strip_suffix(".jsonl"))
                .map(|s| s.to_string());
            if let Some(s) = slug {
                events_slugs.insert(s);
            }
        }
    }

    let mut summaries: Vec<RunLogSummary> = runs_rows_by_slug
        .iter()
        .map(|(slug, rl)| run_log_to_summary(slug.clone(), rl.clone()))
        .collect();

    // Synthesize interrupted entries for events files that lack a
    // matching RunLog row. Spec: app-workspace § Interrupted Run
    // Detection. Mode is "goal" (chat / query / fix interrupted
    // detection is out of v1 scope per the same requirement).
    for slug in &events_slugs {
        if runs_rows_by_slug.contains_key(slug) {
            continue;
        }
        let events_file = log_dir.join(format!("events-{slug}.jsonl"));
        let goal_text = first_goal_text_in_events(&events_file).unwrap_or_default();
        let started_at = unslug_started_at(slug);
        summaries.push(RunLogSummary {
            run_id: slug.clone(),
            mode: "goal".into(),
            goal: goal_text,
            model: None,
            effort: None,
            started_at,
            finished_at: String::new(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "interrupted".into(),
            session_id: None,
        });
    }

    summaries.retain(|s| match mode_filter {
        ModeFilter::All => true,
        ModeFilter::Goal => s.mode == "goal",
    });
    summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Ok(summaries)
}

fn run_log_to_summary(run_id: String, rl: RunLog) -> RunLogSummary {
    RunLogSummary {
        run_id,
        mode: rl.mode,
        goal: rl.goal,
        model: rl.model,
        effort: rl.effort,
        started_at: rl.started_at,
        finished_at: rl.finished_at,
        tokens: rl.tokens,
        wiki_changed: rl.wiki_changed,
        lint_error_count: rl.lint_error_count,
        lint_warn_count: rl.lint_warn_count,
        outcome: rl.outcome,
        session_id: rl.session_id,
    }
}

/// Convert a slug like `"2026-05-13T03-00-00Z"` back to the RFC 3339
/// form `"2026-05-13T03:00:00Z"`. Only the two dashes immediately
/// following `T` are reverted to colons; the date portion `YYYY-MM-DD`
/// is left intact.
fn unslug_started_at(slug: &str) -> String {
    if let Some(t_pos) = slug.find('T') {
        let prefix = &slug[..=t_pos];
        let rest = &slug[t_pos + 1..];
        let mut result = String::with_capacity(rest.len());
        let mut dashes_left: u8 = 2;
        for c in rest.chars() {
            if c == '-' && dashes_left > 0 {
                result.push(':');
                dashes_left -= 1;
            } else {
                result.push(c);
            }
        }
        format!("{prefix}{result}")
    } else {
        slug.to_string()
    }
}

/// Scan the first ~10 events of an events-*.jsonl file for a
/// `VerbBanner::Goal { goal_text }` payload. Returns the embedded
/// goal text when found, otherwise `None`. Used by the interrupted
/// virtual entry to recover the original goal so the GUI row still
/// shows a useful label.
fn first_goal_text_in_events(events_file: &Path) -> Option<String> {
    let body = fs::read_to_string(events_file).ok()?;
    for line in body.lines().take(10) {
        let envelope: EventEnvelope = match serde_json::from_str(line) {
            Ok(env) => env,
            Err(_) => continue,
        };
        if let VerbEvent::Banner(VerbBanner::Goal { goal_text }) = envelope.event {
            return Some(goal_text);
        }
    }
    None
}

// ---- get_run_detail -------------------------------------------------------

#[tauri::command]
pub async fn get_run_detail(vault_path: String, run_id: String) -> IpcResult<RunDetail> {
    let log_dir = Path::new(&vault_path).join(".codebus").join("log");
    get_run_detail_impl(&log_dir, &run_id)
}

pub(crate) fn get_run_detail_impl(
    log_dir: &Path,
    run_id: &str,
) -> Result<RunDetail, AppError> {
    let summaries = list_runs_impl(log_dir, ModeFilter::All)?;
    let summary = summaries
        .into_iter()
        .find(|s| s.run_id == run_id)
        .ok_or_else(|| AppError::Invalid {
            field: "run_id".into(),
            message: format!("no run found for id `{run_id}`"),
        })?;

    let events_file = log_dir.join(format!("events-{run_id}.jsonl"));
    let events = read_events_jsonl(&events_file)?;

    Ok(RunDetail { summary, events })
}

fn read_events_jsonl(path: &Path) -> Result<Vec<EventEnvelope>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = fs::read_to_string(path)?;
    let mut events = Vec::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<EventEnvelope>(line) {
            Ok(env) => events.push(env),
            // Skip malformed lines silently — the events file is a
            // best-effort log and partial corruption shouldn't sink
            // the whole timeline view.
            Err(_) => continue,
        }
    }
    Ok(events)
}

// ---- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::log::events::sink::EventEnvelope;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    fn fake_report() -> GoalReport {
        GoalReport {
            accumulated_tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            started_at: "2026-05-13T00:00:00Z".into(),
            finished_at: "2026-05-13T00:00:01Z".into(),
            agent_exit_code: Some(0),
            fix_post_lint_issues_remain: false,
        }
    }

    fn run_log(started_at: &str, mode: &str, outcome: &str, goal: &str) -> RunLog {
        RunLog {
            goal: goal.into(),
            mode: mode.into(),
            model: None,
            effort: None,
            started_at: started_at.into(),
            finished_at: started_at.into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: outcome.into(),
            session_id: None,
        }
    }

    /// Task 3.1 acceptance: spawn_goal returns a run id AND the
    /// active_runs map contains that key while the verb thread is alive.
    #[test]
    fn spawn_goal_returns_run_id_and_registers_active_run() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();

        let (start_tx, start_rx) = mpsc::sync_channel::<()>(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);

        let runner = move |_repo: &Path,
                           _opts: GoalOptions,
                           _on_event: Box<dyn FnMut(VerbEvent) + Send>,
                           _cancel: Option<Arc<AtomicBool>>| {
            start_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(fake_report())
        };

        let (terminal_tx, terminal_rx) = mpsc::sync_channel::<String>(1);
        let temp = tempfile::TempDir::new().unwrap();
        let run_id = spawn_goal_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "test goal".into(),
            |_payload| {},
            move |payload| {
                let _ = terminal_tx.send(payload.run_id);
            },
            runner,
        )
        .expect("spawn ok");

        start_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("runner thread should reach start signal");
        assert!(
            active_runs.get(&run_id).is_some(),
            "active_runs MUST contain run_id while the verb thread is alive"
        );

        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while active_runs.get(&run_id).is_some() {
            if Instant::now() > deadline {
                panic!("active_runs entry not removed after runner completed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(active_runs.is_empty());
        // Terminal emit fires exactly once after thread exit.
        let terminal_id = terminal_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("emit_terminal must fire after runner completes");
        assert_eq!(terminal_id, run_id);
    }

    /// Spec: app-workspace § One Active Goal Run At A Time — backend.
    #[test]
    fn spawn_goal_rejects_when_another_run_is_active() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();
        active_runs.insert("existing".into(), Arc::new(AtomicBool::new(false)));

        let temp = tempfile::TempDir::new().unwrap();
        let err = spawn_goal_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "x".into(),
            |_p| {},
            |_terminal| {},
            |_repo, _opts, _on_event, _cancel| Ok(fake_report()),
        )
        .expect_err("second spawn must be rejected");
        match err {
            AppError::Invalid { field, message } => {
                assert_eq!(field, "active_runs");
                assert!(
                    message.contains("already active"),
                    "message should mention 'already active': {message}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    /// Spec: app-workspace § One Active Goal Run At A Time —
    /// "Chat turn does not block concurrent goal spawn" scenario.
    /// Chat runs (key prefix `chat-`) MUST NOT register as goal runs.
    #[test]
    fn spawn_goal_succeeds_with_concurrent_chat_turn() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();
        // Simulate an active chat turn already running.
        active_runs.insert(
            "chat-2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );

        let temp = tempfile::TempDir::new().unwrap();
        let run_id = spawn_goal_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "concurrent goal".into(),
            |_p| {},
            |_terminal| {},
            |_repo, _opts, _on_event, _cancel| Ok(fake_report()),
        )
        .expect("goal spawn MUST succeed when only a chat turn is active");

        // Both entries SHALL coexist in active_runs.
        assert!(active_runs.has_chat_turn(), "chat entry preserved");
        // Wait for the runner thread to complete so the goal entry gets
        // cleaned up; meanwhile the chat entry stays.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while active_runs.get(&run_id).is_some() {
            if std::time::Instant::now() > deadline {
                panic!("goal entry not removed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            active_runs.has_chat_turn(),
            "chat entry SHALL persist after goal completes"
        );
    }

    /// Task 3.2 acceptance: cancel_goal is idempotent on unknown run.
    #[test]
    fn cancel_goal_idempotent_on_unknown_run() {
        let runtime = AppRuntimeState::new();
        cancel_goal_impl(&runtime.active_runs, "nonexistent")
            .expect("must succeed idempotently when run id unknown");
    }

    /// cancel_goal flips the stored cancel flag for an active run.
    #[test]
    fn cancel_goal_flips_existing_flag() {
        let runtime = AppRuntimeState::new();
        let flag = Arc::new(AtomicBool::new(false));
        runtime.active_runs.insert("run-x".into(), flag.clone());

        cancel_goal_impl(&runtime.active_runs, "run-x").expect("ok");
        assert!(
            flag.load(std::sync::atomic::Ordering::Relaxed),
            "cancel flag must be set after cancel_goal"
        );
    }

    fn write_runs_jsonl(dir: &Path, name: &str, rows: &[RunLog]) {
        let mut body = String::new();
        for r in rows {
            body.push_str(&serde_json::to_string(r).unwrap());
            body.push('\n');
        }
        fs::write(dir.join(name), body).unwrap();
    }

    fn write_events_jsonl(dir: &Path, slug: &str, events: &[VerbEvent]) {
        let mut body = String::new();
        for e in events {
            let envelope = EventEnvelope {
                ts: "2026-05-13T00:00:00Z".into(),
                event: e.clone(),
            };
            body.push_str(&serde_json::to_string(&envelope).unwrap());
            body.push('\n');
        }
        fs::write(dir.join(format!("events-{slug}.jsonl")), body).unwrap();
    }

    /// Task 3.3 acceptance: list_runs filters by mode.
    #[test]
    fn list_runs_filters_by_mode() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();

        write_runs_jsonl(
            &log_dir,
            "runs-2026-05-13.jsonl",
            &[
                run_log("2026-05-13T10:00:00Z", "goal", "succeeded", "g1"),
                run_log("2026-05-13T11:00:00Z", "goal", "succeeded", "g2"),
                run_log("2026-05-13T12:00:00Z", "goal", "succeeded", "g3"),
                run_log("2026-05-13T13:00:00Z", "chat", "succeeded", "c1"),
                run_log("2026-05-13T14:00:00Z", "chat", "succeeded", "c2"),
                run_log("2026-05-13T15:00:00Z", "fix", "succeeded", "f1"),
            ],
        );

        let goal_only = list_runs_impl(&log_dir, ModeFilter::Goal).unwrap();
        assert_eq!(goal_only.len(), 3, "expected 3 goal rows: {goal_only:?}");
        assert!(goal_only.iter().all(|s| s.mode == "goal"));

        let all = list_runs_impl(&log_dir, ModeFilter::All).unwrap();
        assert_eq!(all.len(), 6, "All filter should return 6 rows");
        // Sort descending check
        let starts: Vec<&str> = all.iter().map(|s| s.started_at.as_str()).collect();
        let mut sorted = starts.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(starts, sorted, "list_runs must sort descending");
    }

    /// Task 3.4 acceptance: orphan events file synthesizes a virtual
    /// interrupted entry; once a matching RunLog row appears, the
    /// virtual entry disappears.
    #[test]
    fn list_runs_synthesizes_interrupted_virtual_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();

        write_events_jsonl(
            &log_dir,
            "2026-05-13T03-00-00Z",
            &[
                VerbEvent::Banner(VerbBanner::Start {
                    repo_path: PathBuf::from("/some/repo"),
                }),
                VerbEvent::Banner(VerbBanner::Goal {
                    goal_text: "describe auth flow".into(),
                }),
            ],
        );

        let entries = list_runs_impl(&log_dir, ModeFilter::Goal).unwrap();
        let virt = entries
            .iter()
            .find(|s| s.outcome == "interrupted")
            .expect("orphan events file must synthesize interrupted entry");
        assert_eq!(virt.run_id, "2026-05-13T03-00-00Z");
        assert_eq!(virt.started_at, "2026-05-13T03:00:00Z");
        assert_eq!(virt.mode, "goal");
        assert_eq!(virt.goal, "describe auth flow");

        // Now add a RunLog row with the same started_at; virtual entry
        // must disappear (real row supersedes).
        write_runs_jsonl(
            &log_dir,
            "runs-2026-05-13.jsonl",
            &[run_log(
                "2026-05-13T03:00:00Z",
                "goal",
                "cancelled",
                "describe auth flow",
            )],
        );
        let entries = list_runs_impl(&log_dir, ModeFilter::Goal).unwrap();
        let same_slug = entries
            .iter()
            .find(|s| s.run_id == "2026-05-13T03-00-00Z")
            .unwrap();
        assert_eq!(
            same_slug.outcome, "cancelled",
            "real RunLog row must supersede virtual interrupted"
        );
        assert!(
            !entries.iter().any(|s| s.outcome == "interrupted"),
            "no interrupted entry should remain once RunLog row exists"
        );
    }

    /// Task 3.5 acceptance: get_run_detail replays the events file
    /// alongside the matching RunLog summary.
    #[test]
    fn get_run_detail_replays_events_jsonl() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();

        let started = "2026-05-13T10:00:00Z";
        let slug = started.replace(':', "-");
        write_runs_jsonl(
            &log_dir,
            "runs-2026-05-13.jsonl",
            &[run_log(started, "goal", "succeeded", "describe X")],
        );
        write_events_jsonl(
            &log_dir,
            &slug,
            &[
                VerbEvent::Banner(VerbBanner::Start {
                    repo_path: PathBuf::from("/repo"),
                }),
                VerbEvent::Banner(VerbBanner::Goal {
                    goal_text: "describe X".into(),
                }),
                VerbEvent::Banner(VerbBanner::SyncStart),
                VerbEvent::Banner(VerbBanner::SyncDone {
                    files: 12,
                    mib: 0.4,
                    elapsed_ms: 80,
                }),
                VerbEvent::Banner(VerbBanner::Done {
                    wiki_path: PathBuf::from("/repo/.codebus/wiki"),
                }),
            ],
        );

        let detail = get_run_detail_impl(&log_dir, &slug).unwrap();
        assert_eq!(detail.events.len(), 5);
        assert_eq!(detail.summary.run_id, slug);
        assert_eq!(detail.summary.goal, "describe X");
        assert_eq!(detail.summary.outcome, "succeeded");
    }

    #[test]
    fn get_run_detail_returns_invalid_for_unknown_run_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = get_run_detail_impl(tmp.path(), "no-such-run").expect_err("must fail");
        match err {
            AppError::Invalid { field, .. } => assert_eq!(field, "run_id"),
            other => panic!("expected Invalid{{field=run_id}}, got {other:?}"),
        }
    }

    #[test]
    fn unslug_started_at_inverts_colon_slug() {
        assert_eq!(
            unslug_started_at("2026-05-13T14-56-21Z"),
            "2026-05-13T14:56:21Z"
        );
        assert_eq!(
            unslug_started_at("2026-05-13T03-00-00Z"),
            "2026-05-13T03:00:00Z"
        );
    }

    #[test]
    fn list_runs_returns_empty_for_missing_log_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let entries = list_runs_impl(&missing, ModeFilter::All).unwrap();
        assert!(entries.is_empty());
    }
}
