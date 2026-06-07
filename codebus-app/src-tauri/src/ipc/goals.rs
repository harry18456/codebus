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
use codebus_core::log::sink::{InterruptReason, RunLog, TokenUsage};
use codebus_core::config::{default_config_path, load_goal_config, load_lifecycle_config};
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
    /// Why the run did not reach the success path. Mirrors
    /// `codebus_core::log::sink::RunLog::interrupt_reason`. Virtual
    /// interrupted entries synthesized from orphan events jsonl files
    /// SHALL set this to `Some(InterruptReason::AppClose)` per the
    /// interrupted-state-formalize spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_reason: Option<InterruptReason>,
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

// ---- content-verify config resolution -------------------------------------

/// goal-content-verify D6: resolve `goal.content_verify` from the shared
/// `goal.*` config using the same core loader the CLI uses. A missing
/// path, missing file/section/field, or any load error conservatively
/// resolves to `false` — a config error MUST NOT silently enable extra
/// verify/repair spawns. Never reads the app-only `app.*` namespace.
pub(crate) fn resolve_goal_content_verify(cfg_path: Option<&Path>) -> bool {
    match cfg_path {
        Some(p) => load_goal_config(p)
            .map(|c| c.content_verify)
            .unwrap_or(false),
        None => false,
    }
}

/// run-outcome-lifecycle-integrity: resolve the per-run wall-clock timeout
/// (`lifecycle.run_timeout_secs`) for injection into a `run_*` verb. A
/// missing file / section / field, a `0` value, OR a load error all resolve
/// to `None` (no limit). Shared by the goal / chat / quiz IPC handlers.
pub(crate) fn resolve_run_timeout(cfg_path: Option<&Path>) -> Option<std::time::Duration> {
    let p = cfg_path?;
    load_lifecycle_config(p)
        .ok()?
        .run_timeout_secs
        .map(std::time::Duration::from_secs)
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
    // GUI parity with the CLI: resolve `goal.content_verify` from the
    // same shared core loader (conservative false on any error).
    let content_verify = resolve_goal_content_verify(default_config_path().as_deref());
    let run_timeout = resolve_run_timeout(default_config_path().as_deref());
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
        content_verify,
        move |repo, options, mut on_event, cancel, started_at| {
            run_goal(
                repo,
                options,
                |e| on_event(e),
                cancel,
                run_timeout,
                Some(started_at),
            )
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
    content_verify: bool,
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
            String,
        ) -> Result<GoalReport, VerbError>
        + Send
        + 'static,
{
    // Spec: app-workspace § One Active Goal Run At A Time + § Cross-Vault
    // Goal Spawn Permitted — backend layer. Chat turns (keyed `chat-<slug>`)
    // SHALL NOT block goal spawn — chat is read-only and cannot conflict
    // with goal's writes, so the two can coexist in `active_runs`. Other
    // vaults' goal entries SHALL NOT block either — only another goal-mode
    // entry under the SAME vault blocks.
    let vault_str = vault_path.to_string_lossy();
    if active_runs.has_goal_run_for_vault(&vault_str) {
        return Err(AppError::Invalid {
            field: "active_runs".into(),
            message: "another goal run is already active".into(),
        });
    }

    // Single wall-clock sample threaded through every consumer. The colon
    // RFC 3339 form (`started_at`) goes down into `run_goal` (events.jsonl
    // filename slug + RunLog.started_at); the slug form (`:`→`-`) is the
    // `RunId` returned to the frontend AND the `active_runs` key AND the
    // `goal-stream` / `goal-terminal` payload id. Because all of these
    // derive from ONE sample, the frontend's `RunId` is byte-identical to
    // the persisted run's identity — a completed run stays reachable by
    // `get_run_detail`, never lost to a second `Utc::now()` drift. Millis
    // precision also keeps two same-second spawns distinct. See spec
    // `app-workspace § Interrupted Run Detection — Single-Source Run Id
    // Invariant`.
    let started_at = goal_started_at_now();
    let run_id = slug_started_at(&started_at);

    let cancel = Arc::new(AtomicBool::new(false));
    active_runs.insert(&vault_str, run_id.clone(), cancel.clone());

    let active_runs_thread = active_runs.clone();
    let run_id_thread = run_id.clone();
    let started_at_thread = started_at.clone();
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
                // goal-content-verify D6: resolved from the shared
                // `goal.*` config by the caller (CLI parity).
                content_verify,
            };

            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let _ = runner(
                    &vault_path,
                    options,
                    on_event,
                    Some(cancel_thread),
                    started_at_thread,
                );
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
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    mode_filter: ModeFilter,
) -> IpcResult<Vec<RunLogSummary>> {
    let log_dir = Path::new(&vault_path).join(".codebus").join("log");
    list_runs_impl(&log_dir, mode_filter, &runtime.active_runs)
}

/// Implementation-side `list_runs`. Scans `log_dir` for both
/// `runs-*.jsonl` (per-run summaries) and `events-*.jsonl` (per-event
/// timelines). Real rows are projected as `RunLogSummary`; orphan
/// events files (no matching `RunLog` row) are projected as either
/// `outcome="running"` (when the slug is currently in `active_runs`,
/// per Decision 6 of `vault-switch-goal-regression`) or
/// `outcome="interrupted"` (the legacy synthesis, used when the events
/// file is genuinely abandoned — process exited mid-run, no live
/// `active_runs` entry to claim it).
///
/// Sorted by `started_at` descending so the freshest run appears first.
pub(crate) fn list_runs_impl(
    log_dir: &Path,
    mode_filter: ModeFilter,
    active_runs: &ActiveRuns,
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
    // Detection — detection is goal-only. An events file is identified as
    // a goal run only when it carries a `VerbBanner::Goal` event; non-goal
    // verbs (chat / query / fix / quiz) never emit that banner. Skipping
    // unidentified files keeps in-progress non-goal runs (events written
    // before their terminal RunLog row) out of the Goals list, where they
    // would otherwise surface as empty-goal `interrupted` rows.
    for slug in &events_slugs {
        if runs_rows_by_slug.contains_key(slug) {
            continue;
        }
        let events_file = log_dir.join(format!("events-{slug}.jsonl"));
        let goal_text = match first_goal_text_in_events(&events_file) {
            Some(text) => text,
            None => continue,
        };
        let started_at = unslug_started_at(slug);
        // Decision 6 of vault-switch-goal-regression: a slug currently
        // present in `active_runs` is an IN-FLIGHT goal whose RunLog
        // has not been written yet — surface it as `running` so the
        // GUI matches backend reality and users don't think the goal
        // was interrupted just because they navigated away briefly.
        // Only when the slug is truly orphaned (no live entry) do we
        // synthesize the legacy `interrupted` projection.
        let is_live = active_runs.get(slug).is_some();
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
            outcome: if is_live { "running" } else { "interrupted" }.into(),
            session_id: None,
            interrupt_reason: if is_live {
                None
            } else {
                Some(InterruptReason::AppClose)
            },
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
        interrupt_reason: rl.interrupt_reason,
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
pub async fn get_run_detail(
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    run_id: String,
) -> IpcResult<RunDetail> {
    let log_dir = Path::new(&vault_path).join(".codebus").join("log");
    get_run_detail_impl(&log_dir, &run_id, &runtime.active_runs)
}

pub(crate) fn get_run_detail_impl(
    log_dir: &Path,
    run_id: &str,
    active_runs: &ActiveRuns,
) -> Result<RunDetail, AppError> {
    let summaries = list_runs_impl(log_dir, ModeFilter::All, active_runs)?;
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

pub(crate) fn read_events_jsonl(path: &Path) -> Result<Vec<EventEnvelope>, AppError> {
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

/// Sample the wall-clock once as an RFC 3339 UTC string at millisecond
/// precision (colon time separators). This is the SINGLE source for a
/// goal run's identity: the colon form threads into `run_goal` as
/// `run_started_at` (events.jsonl filename slug + RunLog.started_at) and
/// the slug form (`slug_started_at`) is the `RunId` returned to the
/// frontend. Millisecond precision keeps two same-second spawns distinct.
pub(crate) fn goal_started_at_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Slug an RFC 3339 started_at string into a `RunId` / events-file slug by
/// replacing `:` with `-`. Pairs with `goal_started_at_now` so the RunId
/// and the persisted run derive from the SAME sample.
pub(crate) fn slug_started_at(started_at: &str) -> String {
    started_at.replace(':', "-")
}

// ---- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::log::events::sink::EventEnvelope;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    /// Regression (goal-run-id-unify-stuck-rundetail): the `RunId` returned
    /// by `spawn_goal` and the `run_started_at` threaded into `run_goal`
    /// MUST originate from the SAME single wall-clock sample, so the
    /// frontend's `RunId` is byte-identical (after slugging) to the
    /// events.jsonl filename slug + `RunLog.started_at` the verb persists.
    ///
    /// This replaces the former `goal_run_id_precision_matches_verb_run_started_at_slug`
    /// test, which only compared two INDEPENDENT samples by byte LENGTH —
    /// it passed even though the values drifted by milliseconds, which is
    /// exactly the bug that left a completed run unreachable by
    /// `get_run_detail` and hung the GUI on "載入中…". See app-workspace
    /// § Interrupted Run Detection — Single-Source Run Id Invariant.
    #[test]
    fn spawn_goal_runid_matches_started_at_threaded_into_run_goal() {
        let runtime = AppRuntimeState::new();
        let (tx, rx) = mpsc::sync_channel::<String>(1);
        let temp = tempfile::TempDir::new().unwrap();
        // Capturing runner records the started_at the IPC layer threads
        // down into run_goal.
        let runner = move |_repo: &Path,
                           _opts: GoalOptions,
                           _on_event: Box<dyn FnMut(VerbEvent) + Send>,
                           _cancel: Option<Arc<AtomicBool>>,
                           started_at: String| {
            let _ = tx.send(started_at);
            Ok(fake_report())
        };
        let run_id = spawn_goal_with_runner(
            runtime.active_runs.clone(),
            temp.path().to_path_buf(),
            "drift check".into(),
            |_p| {},
            |_t| {},
            false,
            runner,
        )
        .expect("spawn ok");
        let threaded_started_at = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("runner must receive run_started_at");
        assert_eq!(
            run_id,
            slug_started_at(&threaded_started_at),
            "the RunId returned to the frontend MUST equal the slug of the \
             started_at threaded into run_goal (single-source invariant); \
             run_id=`{run_id}` started_at=`{threaded_started_at}`"
        );
    }

    /// The single-sample RunId derivation (`slug_started_at` over
    /// `goal_started_at_now`) carries millisecond precision so two spawns
    /// within the same wall-clock second produce distinct ids. See spec
    /// `app-workspace § Tauri IPC Commands for Goal Lifecycle and Wiki Read`
    /// scenario "spawn_goal same-second calls yield distinct RunIds".
    #[test]
    fn goal_run_id_same_second_yields_distinct_ids() {
        let first = slug_started_at(&goal_started_at_now());
        std::thread::sleep(Duration::from_millis(2));
        let second = slug_started_at(&goal_started_at_now());
        assert_ne!(
            first, second,
            "two consecutive RunId derivations must differ; got {first} and {second}"
        );
        assert!(
            first.contains('.'),
            "RunId must include a `.fff` fractional component; got {first}"
        );
        assert!(
            second.contains('.'),
            "RunId must include a `.fff` fractional component; got {second}"
        );
    }

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
            content_review: None,
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
            sandbox_denial_count: 0,
            interrupt_reason: None,
        }
    }

    use std::sync::mpsc::SyncSender;

    /// Capturing runner: records the `GoalOptions` injected into
    /// `run_goal` so tests can assert config-resolved threading, then
    /// returns a fake successful report.
    fn capturing_runner(
        tx: SyncSender<GoalOptions>,
    ) -> impl FnOnce(
        &Path,
        GoalOptions,
        Box<dyn FnMut(VerbEvent) + Send>,
        Option<Arc<AtomicBool>>,
        String,
    ) -> Result<GoalReport, VerbError>
           + Send
           + 'static {
        move |_repo, opts, _on_event, _cancel, _started_at| {
            let _ = tx.send(opts);
            Ok(fake_report())
        }
    }

    fn write_goal_cfg(home: &Path, body: &str) {
        let cb = home.join(".codebus");
        fs::create_dir_all(&cb).unwrap();
        fs::write(cb.join("config.yaml"), body).unwrap();
    }

    /// goal-content-verify task 6.1 (design D6; spec app-workspace /
    /// Goal Content Verify GUI Wiring). Scenario: GUI resolves config
    /// and threads goal text — `goal.content_verify: true` → `run_goal`
    /// receives `content_verify == true` and the originating goal text.
    #[test]
    fn spawn_goal_threads_content_verify_true_and_goal_text() {
        let home = tempfile::TempDir::new().unwrap();
        write_goal_cfg(home.path(), "goal:\n  content_verify: true\n");
        let cv = resolve_goal_content_verify(Some(
            &home.path().join(".codebus").join("config.yaml"),
        ));
        assert!(cv, "true config must resolve to content_verify=true");

        let runtime = AppRuntimeState::new();
        let (tx, rx) = mpsc::sync_channel::<GoalOptions>(1);
        let temp = tempfile::TempDir::new().unwrap();
        spawn_goal_with_runner(
            runtime.active_runs.clone(),
            temp.path().to_path_buf(),
            "describe auth".into(),
            |_p| {},
            |_t| {},
            cv,
            capturing_runner(tx),
        )
        .expect("spawn ok");
        let opts = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("runner must receive GoalOptions");
        assert!(opts.content_verify, "content_verify must thread into run_goal");
        assert_eq!(
            opts.text, "describe auth",
            "originating goal text must thread into run_goal (off-goal check)"
        );
    }

    /// Scenario: GUI default-off — absent config → content_verify false.
    #[test]
    fn resolve_goal_content_verify_false_when_config_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!resolve_goal_content_verify(Some(
            &tmp.path().join("nonexistent.yaml")
        )));
        assert!(!resolve_goal_content_verify(None));
    }

    /// Scenario: GUI config load error is conservative — a malformed
    /// config resolves to `false` (do NOT silently enable extra spawns).
    #[test]
    fn resolve_goal_content_verify_false_on_load_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = tmp.path().join("config.yaml");
        fs::write(&p, "goal:\n  content_verify: not-a-bool\n").unwrap();
        assert!(
            !resolve_goal_content_verify(Some(&p)),
            "a malformed goal config must conservatively resolve to false"
        );
    }

    /// run-outcome-lifecycle-integrity: GUI resolves the per-run timeout from
    /// `lifecycle.run_timeout_secs`. Absent path/config → None; a positive
    /// value → Some(Duration); a malformed config conservatively → None.
    #[test]
    fn resolve_run_timeout_maps_config_to_duration() {
        use std::time::Duration;
        // No path / absent file → None.
        assert_eq!(resolve_run_timeout(None), None);
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(
            resolve_run_timeout(Some(&tmp.path().join("nonexistent.yaml"))),
            None
        );
        // Positive value → Some(Duration).
        let p = tmp.path().join("config.yaml");
        fs::write(&p, "lifecycle:\n  run_timeout_secs: 1800\n").unwrap();
        assert_eq!(resolve_run_timeout(Some(&p)), Some(Duration::from_secs(1800)));
        // Zero normalizes to None (no instant-kill foot-gun).
        fs::write(&p, "lifecycle:\n  run_timeout_secs: 0\n").unwrap();
        assert_eq!(resolve_run_timeout(Some(&p)), None);
        // Malformed config conservatively → None.
        fs::write(&p, "lifecycle:\n  run_timeout_secs: not-a-number\n").unwrap();
        assert_eq!(resolve_run_timeout(Some(&p)), None);
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
                           _cancel: Option<Arc<AtomicBool>>,
                           _started_at: String| {
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
            false,
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
        let temp = tempfile::TempDir::new().unwrap();
        let vault_str = temp.path().to_string_lossy().into_owned();
        // existing goal entry under THE SAME vault as the upcoming spawn,
        // so the per-vault guard SHALL still reject the second spawn.
        active_runs.insert(&vault_str, "existing".into(), Arc::new(AtomicBool::new(false)));

        let err = spawn_goal_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "x".into(),
            |_p| {},
            |_terminal| {},
            false,
            |_repo, _opts, _on_event, _cancel, _started_at| Ok(fake_report()),
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
        let temp = tempfile::TempDir::new().unwrap();
        let vault_str = temp.path().to_string_lossy().into_owned();
        // Simulate an active chat turn already running under the same vault
        // — per-vault scope still excludes chat- prefix from goal counting.
        active_runs.insert(
            &vault_str,
            "chat-2026-05-14T10-20-30Z".into(),
            Arc::new(AtomicBool::new(false)),
        );

        let run_id = spawn_goal_with_runner(
            active_runs.clone(),
            temp.path().to_path_buf(),
            "concurrent goal".into(),
            |_p| {},
            |_terminal| {},
            false,
            |_repo, _opts, _on_event, _cancel, _started_at| Ok(fake_report()),
        )
        .expect("goal spawn MUST succeed when only a chat turn is active");

        // Both entries SHALL coexist in active_runs.
        assert!(
            active_runs.has_chat_turn_for_vault(&vault_str),
            "chat entry preserved"
        );
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
            active_runs.has_chat_turn_for_vault(&vault_str),
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
        runtime
            .active_runs
            .insert("/vault/test", "run-x".into(), flag.clone());

        cancel_goal_impl(&runtime.active_runs, "run-x").expect("ok");
        assert!(
            flag.load(std::sync::atomic::Ordering::Relaxed),
            "cancel flag must be set after cancel_goal"
        );
    }

    /// Spec: app-workspace § Cross-Vault Goal Spawn Permitted scenario 1.
    /// A goal active under vault A SHALL NOT block spawn_goal under
    /// vault B; both entries SHALL coexist in active_runs.
    #[test]
    fn spawn_goal_cross_vault_allowed() {
        let runtime = AppRuntimeState::new();
        let active_runs = runtime.active_runs.clone();

        // Pre-seed a goal-mode entry under vault A. We do NOT spin up a
        // real spawn thread for vault A — directly inserting captures the
        // "goal active under vault A" state without needing concurrent
        // runner threads. The pre-spawn guard for vault B reads the same
        // map so this is a valid representation of the spec scenario.
        let vault_a = "/vault/a";
        active_runs.insert(
            vault_a,
            "2026-05-28T10-00-00Z".into(),
            Arc::new(AtomicBool::new(false)),
        );
        assert!(
            active_runs.has_goal_run_for_vault(vault_a),
            "vault A precondition: has goal active"
        );

        // Spawn against vault B — SHALL succeed despite vault A entry.
        let temp_b = tempfile::TempDir::new().unwrap();
        let vault_b_str = temp_b.path().to_string_lossy().into_owned();
        assert_ne!(
            vault_a, vault_b_str,
            "test precondition: vault A and vault B paths differ"
        );

        let (start_tx, start_rx) = mpsc::sync_channel::<()>(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);

        let runner = move |_repo: &Path,
                           _opts: GoalOptions,
                           _on_event: Box<dyn FnMut(VerbEvent) + Send>,
                           _cancel: Option<Arc<AtomicBool>>,
                           _started_at: String| {
            start_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(fake_report())
        };

        let run_id_b = spawn_goal_with_runner(
            active_runs.clone(),
            temp_b.path().to_path_buf(),
            "vault B goal".into(),
            |_p| {},
            |_terminal| {},
            false,
            runner,
        )
        .expect("spawn_goal under vault B MUST succeed when only vault A has an active goal");

        start_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("runner SHALL reach start signal");

        // Both vault A and vault B goal entries SHALL coexist.
        assert!(
            active_runs.has_goal_run_for_vault(vault_a),
            "vault A goal entry SHALL persist"
        );
        assert!(
            active_runs.has_goal_run_for_vault(&vault_b_str),
            "vault B goal entry SHALL be present after spawn"
        );
        assert!(
            active_runs.get(&run_id_b).is_some(),
            "vault B run_id SHALL be retrievable from active_runs"
        );

        // Release runner so the test cleans up.
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while active_runs.get(&run_id_b).is_some() {
            if Instant::now() > deadline {
                panic!("vault B goal entry not removed after runner completed");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Vault A entry SHALL still be present after vault B's runner ends.
        assert!(
            active_runs.has_goal_run_for_vault(vault_a),
            "vault A entry SHALL outlive vault B's runner termination"
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

        let goal_only = list_runs_impl(&log_dir, ModeFilter::Goal, &ActiveRuns::new()).unwrap();
        assert_eq!(goal_only.len(), 3, "expected 3 goal rows: {goal_only:?}");
        assert!(goal_only.iter().all(|s| s.mode == "goal"));

        let all = list_runs_impl(&log_dir, ModeFilter::All, &ActiveRuns::new()).unwrap();
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

        let entries = list_runs_impl(&log_dir, ModeFilter::Goal, &ActiveRuns::new()).unwrap();
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
        let entries = list_runs_impl(&log_dir, ModeFilter::Goal, &ActiveRuns::new()).unwrap();
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

    /// vault-switch-goal-regression Decision 6: an orphan events file
    /// (no RunLog row yet) whose RunId is currently present in
    /// `active_runs` SHALL surface as `outcome="running"` not
    /// `"interrupted"`. This is the UI-lie fix — frontend `refreshRuns`
    /// after vault re-open MUST see the goal as still alive so the New
    /// Goal modal's Run button stays correctly enabled / disabled.
    #[test]
    fn list_runs_marks_in_flight_goal_as_running_when_active_runs_has_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();
        let slug = "2026-05-28T07-39-26Z";

        // Mimic an in-flight goal: events file has a Goal banner but no
        // RunLog row has been written yet.
        write_events_jsonl(
            &log_dir,
            slug,
            &[
                VerbEvent::Banner(VerbBanner::Start {
                    repo_path: PathBuf::from("/some/repo"),
                }),
                VerbEvent::Banner(VerbBanner::Goal {
                    goal_text: "smoke probe goal".into(),
                }),
            ],
        );

        // Without an active_runs entry: legacy synthesis kicks in.
        let entries_orphan = list_runs_impl(&log_dir, ModeFilter::Goal, &ActiveRuns::new()).unwrap();
        let orphan = entries_orphan
            .iter()
            .find(|s| s.run_id == slug)
            .expect("orphan events file SHALL still synthesize an entry");
        assert_eq!(
            orphan.outcome, "interrupted",
            "no active_runs entry → legacy `interrupted` synthesis"
        );
        assert_eq!(
            orphan.interrupt_reason,
            Some(InterruptReason::AppClose),
            "interrupted synthesis SHALL preserve AppClose reason"
        );

        // With matching active_runs entry: Decision 6 kicks in.
        let active = ActiveRuns::new();
        active.insert(
            "/some/repo",
            slug.into(),
            Arc::new(AtomicBool::new(false)),
        );
        let entries_live = list_runs_impl(&log_dir, ModeFilter::Goal, &active).unwrap();
        let live = entries_live
            .iter()
            .find(|s| s.run_id == slug)
            .expect("in-flight goal SHALL still appear in list");
        assert_eq!(
            live.outcome, "running",
            "active_runs has the slug → outcome SHALL be `running`, not `interrupted`"
        );
        assert!(
            live.interrupt_reason.is_none(),
            "`running` entry SHALL NOT carry an interrupt reason"
        );
        assert_eq!(live.goal, "smoke probe goal");
        assert_eq!(live.finished_at, "", "running entries have no finished_at");
    }

    /// Interrupted detection is goal-only: an orphan events file that does
    /// NOT carry a `VerbBanner::Goal` event (i.e. an in-progress or
    /// interrupted chat / query / fix / quiz run, whose terminal RunLog row
    /// has not been written yet) MUST NOT be synthesized into a virtual
    /// `interrupted` entry — neither under `Goal` nor `All` filtering. This
    /// is what stops such runs from transiently appearing in the Goals list
    /// with empty goal text.
    #[test]
    fn list_runs_skips_non_goal_orphan_events() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();

        // Non-goal verbs never emit `VerbBanner::Goal`. A `Start` banner
        // alone (no `Goal` banner) stands in for any such run's events.
        write_events_jsonl(
            &log_dir,
            "2026-05-13T04-00-00Z",
            &[VerbEvent::Banner(VerbBanner::Start {
                repo_path: PathBuf::from("/some/repo"),
            })],
        );

        for filter in [ModeFilter::Goal, ModeFilter::All] {
            let entries = list_runs_impl(&log_dir, filter, &ActiveRuns::new()).unwrap();
            assert!(
                !entries.iter().any(|s| s.run_id == "2026-05-13T04-00-00Z"),
                "non-goal orphan events file MUST NOT produce any entry \
                 (filter={filter:?}); got {entries:?}"
            );
        }
    }

    /// The goal-only judgement is positive for genuine goal runs: an orphan
    /// events file that DOES carry a `VerbBanner::Goal` event is still
    /// synthesized into a virtual `interrupted` entry with the goal text
    /// recovered from that banner. Complements
    /// `list_runs_skips_non_goal_orphan_events` (the negative case).
    #[test]
    fn list_runs_synthesizes_interrupted_only_for_goal_events() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_dir = tmp.path().to_path_buf();

        // A goal orphan (with Goal banner) and a non-goal orphan (without)
        // side by side: only the goal one becomes an interrupted entry.
        write_events_jsonl(
            &log_dir,
            "2026-05-13T05-00-00Z",
            &[
                VerbEvent::Banner(VerbBanner::Start {
                    repo_path: PathBuf::from("/some/repo"),
                }),
                VerbEvent::Banner(VerbBanner::Goal {
                    goal_text: "map the API surface".into(),
                }),
            ],
        );
        write_events_jsonl(
            &log_dir,
            "2026-05-13T06-00-00Z",
            &[VerbEvent::Banner(VerbBanner::Start {
                repo_path: PathBuf::from("/some/repo"),
            })],
        );

        let entries = list_runs_impl(&log_dir, ModeFilter::Goal, &ActiveRuns::new()).unwrap();
        let goal_entry = entries
            .iter()
            .find(|s| s.run_id == "2026-05-13T05-00-00Z")
            .expect("goal orphan events file MUST synthesize interrupted entry");
        assert_eq!(goal_entry.outcome, "interrupted");
        assert_eq!(goal_entry.mode, "goal");
        assert_eq!(goal_entry.goal, "map the API surface");
        assert!(
            !entries.iter().any(|s| s.run_id == "2026-05-13T06-00-00Z"),
            "non-goal orphan MUST be skipped even alongside a goal orphan; got {entries:?}"
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

        let detail = get_run_detail_impl(&log_dir, &slug, &ActiveRuns::new()).unwrap();
        assert_eq!(detail.events.len(), 5);
        assert_eq!(detail.summary.run_id, slug);
        assert_eq!(detail.summary.goal, "describe X");
        assert_eq!(detail.summary.outcome, "succeeded");
    }

    #[test]
    fn get_run_detail_returns_invalid_for_unknown_run_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = get_run_detail_impl(tmp.path(), "no-such-run", &ActiveRuns::new()).expect_err("must fail");
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
        let entries = list_runs_impl(&missing, ModeFilter::All, &ActiveRuns::new()).unwrap();
        assert!(entries.is_empty());
    }
}
