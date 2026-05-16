//! `spawn_quiz_plan` / `spawn_quiz_generate` / `cancel_quiz` IPC commands
//! (v3-app-quiz task 5.2 — GUI plan-confirm-generate flow).
//!
//! Mirrors the `chats.rs` runner-injectable background-thread pattern.
//! The two spawns are SEPARATE commands so the GUI can interpose the
//! confirm gate (design D1 / app-workspace Quiz Tab Plan-Confirm-Generate
//! Flow): `spawn_quiz_plan` returns the planned scope (or no-match) via a
//! terminal event; the frontend shows it with confirm/revise controls and
//! only then calls `spawn_quiz_generate`. Each `VerbEvent` flows live on
//! the `quiz-stream` channel for the activity stream.
//!
//! Persistence (frontmatter injection / storage layout) is NOT done here
//! — `spawn_quiz_generate`'s terminal payload carries `quiz_md` +
//! `planned_pages` + `events_log` so the frontend drives the answering
//! view (task 5.4) and history persistence (task 5.5).

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;

use codebus_core::verb::quiz::{
    QuizGenerateOptions, QuizPlanOptions, QuizPlanOutcome, QuizPlanReport, QuizReport,
    QuizTrigger, persist_quiz, run_quiz_generate, run_quiz_plan,
};
use codebus_core::verb::{VerbError, VerbEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::IpcResult;
use crate::error::AppError;
use crate::state::active_runs::ActiveRuns;
use crate::state::app_state::AppRuntimeState;

/// Per-`VerbEvent` payload on the `quiz-stream` channel (shared by both
/// the plan and generate spawns; frontend correlates by `run_id`).
#[derive(Debug, Clone, Serialize)]
pub struct QuizStreamPayload {
    pub run_id: String,
    pub event: VerbEvent,
}

/// Terminal payload for `spawn_quiz_plan` on the `quiz-plan-terminal`
/// channel. The frontend acts on `result`: `Scope` → show the page list
/// with confirm/revise; `NoMatch` → show the reason and stop; otherwise
/// surface the failure.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuizPlanResult {
    Scope { pages: Vec<String> },
    NoMatch { reason: String },
    Failed { message: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuizPlanTerminalPayload {
    pub run_id: String,
    pub result: QuizPlanResult,
}

/// Terminal payload for `spawn_quiz_generate` on the
/// `quiz-generate-terminal` channel. On success the frontend receives
/// the fence-stripped `quiz_md` (for the answering view), the
/// `planned_pages`, and `events_log` (for history persistence).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuizGenerateResult {
    Succeeded {
        quiz_md: String,
        planned_pages: Vec<String>,
        events_log: Option<String>,
        /// Persisted attempt path (design D4/D7). `None` if the write
        /// failed (non-fatal — the frontend still has `quiz_md` for the
        /// answering view; history just lacks this attempt).
        quiz_file: Option<String>,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

/// Frontend-supplied trigger provenance for `spawn_quiz_generate`,
/// deserialised from the IPC arg and mapped to the core [`QuizTrigger`]
/// for persistence (design D4/D7 slug + frontmatter).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuizTriggerArg {
    AiPlanned { topic: String },
    WikiPreview { target_page: String },
}

impl QuizTriggerArg {
    fn to_core(&self) -> QuizTrigger {
        match self {
            QuizTriggerArg::AiPlanned { topic } => QuizTrigger::AiPlanned {
                topic: topic.clone(),
            },
            QuizTriggerArg::WikiPreview { target_page } => QuizTrigger::WikiPreview {
                target_page: target_page.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuizGenerateTerminalPayload {
    pub run_id: String,
    pub result: QuizGenerateResult,
}

fn quiz_run_id(prefix: &str) -> String {
    let started_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    format!("quiz-{prefix}-{}", started_at.replace(':', "-"))
}

// ---- spawn_quiz_plan ------------------------------------------------------

#[tauri::command]
pub fn spawn_quiz_plan(
    app: AppHandle,
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    topic: String,
) -> IpcResult<String> {
    let active_runs = runtime.active_runs.clone();
    let app_stream = app.clone();
    let app_terminal = app.clone();
    spawn_quiz_plan_with_runner(
        active_runs,
        PathBuf::from(vault_path),
        topic,
        move |payload| {
            let _ = app_stream.emit("quiz-stream", payload);
        },
        move |payload| {
            let _ = app_terminal.emit("quiz-plan-terminal", payload);
        },
        |repo, options, mut on_event, cancel| {
            run_quiz_plan(repo, options, |e| on_event(e), cancel)
        },
    )
}

#[allow(clippy::type_complexity)]
pub(crate) fn spawn_quiz_plan_with_runner<E, T, F>(
    active_runs: Arc<ActiveRuns>,
    vault_path: PathBuf,
    topic: String,
    emit: E,
    emit_terminal: T,
    runner: F,
) -> Result<String, AppError>
where
    E: Fn(QuizStreamPayload) + Send + 'static,
    T: Fn(QuizPlanTerminalPayload) + Send + 'static,
    F: FnOnce(
            &Path,
            QuizPlanOptions,
            Box<dyn FnMut(VerbEvent) + Send>,
            Option<Arc<AtomicBool>>,
        ) -> Result<QuizPlanReport, VerbError>
        + Send
        + 'static,
{
    let run_id = quiz_run_id("plan");
    let cancel = Arc::new(AtomicBool::new(false));
    active_runs.insert(run_id.clone(), cancel.clone());

    let active_runs_thread = active_runs.clone();
    let run_id_thread = run_id.clone();
    let cancel_thread = cancel.clone();

    thread::Builder::new()
        .name(format!("quiz-plan-{run_id}"))
        .spawn(move || {
            let run_id_event = run_id_thread.clone();
            let emit_for_thread = emit;
            let on_event: Box<dyn FnMut(VerbEvent) + Send> = Box::new(move |event: VerbEvent| {
                emit_for_thread(QuizStreamPayload {
                    run_id: run_id_event.clone(),
                    event,
                });
            });

            let options = QuizPlanOptions { topic };
            let result: std::thread::Result<Result<QuizPlanReport, VerbError>> =
                std::panic::catch_unwind(AssertUnwindSafe(|| {
                    runner(&vault_path, options, on_event, Some(cancel_thread))
                }));

            let plan_result = match result {
                Ok(Ok(report)) => match report.outcome {
                    QuizPlanOutcome::Scope(pages) => QuizPlanResult::Scope { pages },
                    QuizPlanOutcome::NoMatch(reason) => QuizPlanResult::NoMatch { reason },
                },
                Ok(Err(VerbError::Cancelled)) => QuizPlanResult::Cancelled,
                Ok(Err(e)) => QuizPlanResult::Failed {
                    message: format!("{e:?}"),
                },
                Err(_) => QuizPlanResult::Failed {
                    message: "quiz plan thread panicked".into(),
                },
            };

            active_runs_thread.remove(&run_id_thread);
            emit_terminal(QuizPlanTerminalPayload {
                run_id: run_id_thread.clone(),
                result: plan_result,
            });
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn quiz plan thread: {e}"),
        })?;

    Ok(run_id)
}

// ---- spawn_quiz_generate --------------------------------------------------

#[tauri::command]
pub fn spawn_quiz_generate(
    app: AppHandle,
    runtime: State<'_, AppRuntimeState>,
    vault_path: String,
    pages: Vec<String>,
    question_count: u8,
    trigger: QuizTriggerArg,
) -> IpcResult<String> {
    let active_runs = runtime.active_runs.clone();
    let app_stream = app.clone();
    let app_terminal = app.clone();
    spawn_quiz_generate_with_runner(
        active_runs,
        PathBuf::from(vault_path),
        pages,
        question_count,
        trigger,
        move |payload| {
            let _ = app_stream.emit("quiz-stream", payload);
        },
        move |payload| {
            let _ = app_terminal.emit("quiz-generate-terminal", payload);
        },
        |repo, options, mut on_event, cancel| {
            run_quiz_generate(repo, options, |e| on_event(e), cancel)
        },
    )
}

#[allow(clippy::type_complexity)]
pub(crate) fn spawn_quiz_generate_with_runner<E, T, F>(
    active_runs: Arc<ActiveRuns>,
    vault_path: PathBuf,
    pages: Vec<String>,
    question_count: u8,
    trigger: QuizTriggerArg,
    emit: E,
    emit_terminal: T,
    runner: F,
) -> Result<String, AppError>
where
    E: Fn(QuizStreamPayload) + Send + 'static,
    T: Fn(QuizGenerateTerminalPayload) + Send + 'static,
    F: FnOnce(
            &Path,
            QuizGenerateOptions,
            Box<dyn FnMut(VerbEvent) + Send>,
            Option<Arc<AtomicBool>>,
        ) -> Result<QuizReport, VerbError>
        + Send
        + 'static,
{
    let run_id = quiz_run_id("generate");
    let cancel = Arc::new(AtomicBool::new(false));
    active_runs.insert(run_id.clone(), cancel.clone());

    let active_runs_thread = active_runs.clone();
    let run_id_thread = run_id.clone();
    let cancel_thread = cancel.clone();

    thread::Builder::new()
        .name(format!("quiz-generate-{run_id}"))
        .spawn(move || {
            let run_id_event = run_id_thread.clone();
            let emit_for_thread = emit;
            let on_event: Box<dyn FnMut(VerbEvent) + Send> = Box::new(move |event: VerbEvent| {
                emit_for_thread(QuizStreamPayload {
                    run_id: run_id_event.clone(),
                    event,
                });
            });

            let options = QuizGenerateOptions {
                pages,
                question_count,
            };
            let result: std::thread::Result<Result<QuizReport, VerbError>> =
                std::panic::catch_unwind(AssertUnwindSafe(|| {
                    runner(&vault_path, options, on_event, Some(cancel_thread))
                }));

            let gen_result = match result {
                Ok(Ok(report)) => {
                    // Persist with caller-injected frontmatter (design
                    // D4/D7) via the shared core helper. Write failure is
                    // non-fatal: still surface the body for answering.
                    let quiz_file = persist_quiz(
                        &vault_path,
                        &trigger.to_core(),
                        &report,
                    )
                    .ok()
                    .map(|p| p.display().to_string());
                    QuizGenerateResult::Succeeded {
                        quiz_md: report.quiz_md,
                        planned_pages: report.planned_pages,
                        events_log: report.events_log,
                        quiz_file,
                    }
                }
                Ok(Err(VerbError::Cancelled)) => QuizGenerateResult::Cancelled,
                Ok(Err(e)) => QuizGenerateResult::Failed {
                    message: format!("{e:?}"),
                },
                Err(_) => QuizGenerateResult::Failed {
                    message: "quiz generate thread panicked".into(),
                },
            };

            active_runs_thread.remove(&run_id_thread);
            emit_terminal(QuizGenerateTerminalPayload {
                run_id: run_id_thread.clone(),
                result: gen_result,
            });
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn quiz generate thread: {e}"),
        })?;

    Ok(run_id)
}

// ---- cancel_quiz ----------------------------------------------------------

#[tauri::command]
pub fn cancel_quiz(runtime: State<'_, AppRuntimeState>, run_id: String) -> IpcResult<()> {
    cancel_quiz_impl(&runtime.active_runs, &run_id)
}

/// Flip the cancel flag for `run_id` when present; idempotent no-op when
/// missing (a cancel racing with thread completion still resolves Ok).
pub(crate) fn cancel_quiz_impl(
    active_runs: &ActiveRuns,
    run_id: &str,
) -> Result<(), AppError> {
    if let Some(flag) = active_runs.get(run_id) {
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

// ---- quiz history (task 5.5, spec app-workspace Quiz History List) --------

/// One persisted quiz attempt's metadata, parsed from its frontmatter
/// (design D4/D7). The frontend groups these by `slug` (page/topic).
#[derive(Debug, Clone, Serialize)]
pub struct QuizAttemptMeta {
    pub slug: String,
    pub quiz_id: String,
    pub trigger: String,
    pub topic: Option<String>,
    pub target_page: Option<String>,
    pub events_log: Option<String>,
    pub path: String,
}

/// Extract a `key: value` frontmatter scalar (quoted or bare). Returns
/// `None` for `null` / empty so optional fields collapse cleanly.
fn fm_value(line: &str, key: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(&format!("{key}:"))?.trim();
    if rest == "null" || rest.is_empty() {
        return None;
    }
    Some(rest.trim_matches('"').to_string())
}

/// Scan `<vault>/.codebus/quiz/<slug>/*.md`, parse each attempt's
/// frontmatter, return newest-first (by `quiz_id` timestamp). A missing
/// quiz directory is not an error — it yields an empty list.
#[tauri::command]
pub fn list_quiz_attempts(vault_path: String) -> IpcResult<Vec<QuizAttemptMeta>> {
    let quiz_dir = Path::new(&vault_path).join(".codebus").join("quiz");
    list_quiz_attempts_impl(&quiz_dir)
}

pub(crate) fn list_quiz_attempts_impl(
    quiz_dir: &Path,
) -> Result<Vec<QuizAttemptMeta>, AppError> {
    if !quiz_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<QuizAttemptMeta> = Vec::new();
    for slug_entry in std::fs::read_dir(quiz_dir)? {
        let slug_entry = slug_entry?;
        if !slug_entry.path().is_dir() {
            continue;
        }
        let slug = slug_entry.file_name().to_string_lossy().to_string();
        for md in std::fs::read_dir(slug_entry.path())? {
            let p = md?.path();
            if p.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let body = std::fs::read_to_string(&p).unwrap_or_default();
            let mut lines = body.lines();
            if lines.next().map(str::trim) != Some("---") {
                continue; // not a well-formed attempt file
            }
            let mut quiz_id = String::new();
            let mut trigger = String::new();
            let mut topic = None;
            let mut target_page = None;
            let mut events_log = None;
            for line in lines {
                if line.trim() == "---" {
                    break;
                }
                if let Some(v) = fm_value(line, "quiz_id") {
                    quiz_id = v;
                } else if let Some(v) = fm_value(line, "trigger") {
                    trigger = v;
                } else if let Some(v) = fm_value(line, "topic") {
                    topic = Some(v);
                } else if let Some(v) = fm_value(line, "target_page") {
                    target_page = Some(v);
                } else if let Some(v) = fm_value(line, "events_log") {
                    events_log = Some(v);
                }
            }
            out.push(QuizAttemptMeta {
                slug: slug.clone(),
                quiz_id,
                trigger,
                topic,
                target_page,
                events_log,
                path: p.display().to_string(),
            });
        }
    }
    out.sort_by(|a, b| b.quiz_id.cmp(&a.quiz_id));
    Ok(out)
}

/// Read a persisted quiz attempt's markdown. The path MUST resolve
/// under the vault's `.codebus/quiz/` tree (path-traversal guard).
#[tauri::command]
pub fn read_quiz_attempt(vault_path: String, path: String) -> IpcResult<String> {
    let quiz_root = Path::new(&vault_path).join(".codebus").join("quiz");
    let target = Path::new(&path);
    if !target.starts_with(&quiz_root) {
        return Err(AppError::Invalid {
            field: "path".into(),
            message: "path is outside the vault quiz directory".into(),
        });
    }
    Ok(std::fs::read_to_string(target)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn collect<P: Send + 'static>() -> (Arc<Mutex<Vec<P>>>, impl Fn(P) + Send + 'static) {
        let store = Arc::new(Mutex::new(Vec::new()));
        let s = store.clone();
        (store, move |p: P| s.lock().unwrap().push(p))
    }

    #[test]
    fn plan_runner_scope_emits_scope_terminal() {
        let active = Arc::new(ActiveRuns::default());
        let (events, emit) = collect::<QuizStreamPayload>();
        let (terms, emit_t) = collect::<QuizPlanTerminalPayload>();

        let run_id = spawn_quiz_plan_with_runner(
            active.clone(),
            PathBuf::from("/tmp/v"),
            "auth".into(),
            emit,
            emit_t,
            |_repo, _opts, mut on_event, _cancel| {
                on_event(VerbEvent::Lifecycle(
                    codebus_core::verb::VerbLifecycleEvent::QuizScopePlanned {
                        pages: vec!["wiki/a.md".into()],
                    },
                ));
                Ok(QuizPlanReport {
                    outcome: QuizPlanOutcome::Scope(vec!["wiki/a.md".into()]),
                    accumulated_tokens: Default::default(),
                    started_at: "t0".into(),
                    finished_at: "t1".into(),
                    agent_exit_code: Some(0),
                })
            },
        )
        .expect("spawn");
        assert!(run_id.starts_with("quiz-plan-"));

        // Join: wait for the terminal payload (thread is detached, poll).
        for _ in 0..200 {
            if !terms.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let t = terms.lock().unwrap();
        assert_eq!(t.len(), 1);
        match &t[0].result {
            QuizPlanResult::Scope { pages } => assert_eq!(pages, &vec!["wiki/a.md".to_string()]),
            other => panic!("expected Scope, got {other:?}"),
        }
        assert!(
            !events.lock().unwrap().is_empty(),
            "QuizScopePlanned event must stream"
        );
        assert!(active.get(&run_id).is_none(), "run id removed after thread");
    }

    #[test]
    fn generate_runner_success_emits_quiz_md_terminal() {
        let active = Arc::new(ActiveRuns::default());
        let (_events, emit) = collect::<QuizStreamPayload>();
        let (terms, emit_t) = collect::<QuizGenerateTerminalPayload>();

        let run_id = spawn_quiz_generate_with_runner(
            active.clone(),
            PathBuf::from("/tmp/v"),
            vec!["wiki/a.md".into()],
            5,
            QuizTriggerArg::AiPlanned {
                topic: "auth".into(),
            },
            emit,
            emit_t,
            |_repo, _opts, _on_event, _cancel| {
                Ok(QuizReport {
                    quiz_md: "## Q1. x\n## Answer: A".into(),
                    planned_pages: vec!["wiki/a.md".into()],
                    accumulated_tokens: Default::default(),
                    started_at: "t0".into(),
                    finished_at: "t1".into(),
                    agent_exit_code: Some(0),
                    events_log: Some("/v/.codebus/log/events-x.jsonl".into()),
                })
            },
        )
        .expect("spawn");
        assert!(run_id.starts_with("quiz-generate-"));

        for _ in 0..200 {
            if !terms.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let t = terms.lock().unwrap();
        assert_eq!(t.len(), 1);
        match &t[0].result {
            QuizGenerateResult::Succeeded {
                quiz_md,
                planned_pages,
                events_log,
                quiz_file: _,
            } => {
                assert!(quiz_md.contains("## Q1."));
                assert_eq!(planned_pages, &vec!["wiki/a.md".to_string()]);
                assert!(events_log.is_some());
            }
            other => panic!("expected Succeeded, got {other:?}"),
        }
    }

    #[test]
    fn cancel_quiz_idempotent_on_unknown_run() {
        let active = ActiveRuns::default();
        assert!(cancel_quiz_impl(&active, "quiz-plan-nope").is_ok());
    }

    // --- task 5.5 history scan ---

    #[test]
    fn list_quiz_attempts_missing_dir_is_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let got = list_quiz_attempts_impl(&tmp.path().join("nope")).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn list_quiz_attempts_groups_and_sorts_newest_first() {
        let tmp = tempfile::TempDir::new().unwrap();
        let slug_dir = tmp.path().join("auth-abcd1234");
        std::fs::create_dir_all(&slug_dir).unwrap();
        let attempt = |id: &str| {
            format!(
                "---\nquiz_id: {id}\ntrigger: ai_planned\ntopic: \"auth\"\n\
                 target_page: null\nplanned_pages:\n  - wiki/a.md\n\
                 events_log: \"/v/.codebus/log/events-{id}.jsonl\"\n---\n\n## Q1. x\n"
            )
        };
        std::fs::write(
            slug_dir.join("2026-05-16T10-00-00Z.md"),
            attempt("2026-05-16T10-00-00Z"),
        )
        .unwrap();
        std::fs::write(
            slug_dir.join("2026-05-16T11-00-00Z.md"),
            attempt("2026-05-16T11-00-00Z"),
        )
        .unwrap();

        let got = list_quiz_attempts_impl(tmp.path()).unwrap();
        assert_eq!(got.len(), 2, "retry → two attempts under one slug");
        // newest (later quiz_id) first
        assert_eq!(got[0].quiz_id, "2026-05-16T11-00-00Z");
        assert_eq!(got[1].quiz_id, "2026-05-16T10-00-00Z");
        assert_eq!(got[0].slug, "auth-abcd1234");
        assert_eq!(got[0].trigger, "ai_planned");
        assert_eq!(got[0].topic.as_deref(), Some("auth"));
        assert_eq!(got[0].target_page, None);
        assert!(got[0].events_log.is_some());
    }

    #[test]
    fn read_quiz_attempt_rejects_path_outside_quiz_dir() {
        let r = read_quiz_attempt("/v".into(), "/etc/passwd".into());
        assert!(matches!(
            r,
            Err(AppError::Invalid { ref field, .. }) if field == "path"
        ));
    }
}
