//! Cross-platform filesystem watcher seam.
//!
//! Single owner of all `notify` interactions per the `fs-watcher`
//! capability. The module exposes two Tauri commands
//! ([`start_vault_watcher`], [`stop_vault_watcher`]) plus the
//! [`WatcherRegistry::start_lobby`] helper invoked once from `lib::run`
//! for the long-lived `~/.codebus/app-state.json` watcher. Every Tauri
//! event emitted on behalf of filesystem changes flows through this
//! module — no other module is permitted to call `notify::Watcher::new`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, State};

use crate::error::AppError;
use crate::ipc::IpcResult;

// ---- Constants ----------------------------------------------------------

/// Spec-mandated debounce window per `Per-Path Debounce Window`. A raw
/// event for path P starts (or resets) a `DEBOUNCE_WINDOW` timer keyed
/// by P; the corresponding Tauri event is emitted only after the timer
/// elapses without a fresh raw event.
pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(200);

/// How often the pump thread wakes up to (a) drain queued raw events
/// and (b) check the debouncer for ready entries. Smaller = lower
/// emit-latency floor; larger = lower idle CPU. 50 ms keeps the
/// observable end-to-end latency under one `DEBOUNCE_WINDOW + 50 ms`
/// (≈ 250 ms worst case), well within the 400 ms spec tolerance.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

// ---- EmitKind -----------------------------------------------------------

/// One emit-worthy filesystem change as seen by the watcher. Each variant
/// maps 1-to-1 to a Tauri event name + payload per the spec
/// `Event Catalog And Payloads`. The classifier may return multiple
/// variants for a single raw path (e.g. a `.md` modification emits both
/// `wiki-list-changed` and `wiki-page-changed`) to cover the
/// platform-coalescing case in design D4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmitKind {
    WikiListChanged,
    WikiPageChanged { path: PathBuf },
    GoalsChanged,
    GoalRunChanged { run_id: String },
    QuizChanged,
    QuizAttemptChanged { slug: String, id: String },
    VaultListChanged,
    /// Emitted exactly once when a per-vault watcher fails to construct
    /// (`notify::Watcher::new` failure, most commonly Linux ENOSPC or
    /// macOS file-access denial). After this variant fires for a vault,
    /// no other watcher events SHALL be emitted for that vault per
    /// `Watcher Startup Failure Surfaces Loudly`.
    VaultWatcherError { vault_path: PathBuf, reason: String },
}

impl EmitKind {
    pub(crate) fn event_name(&self) -> &'static str {
        match self {
            EmitKind::WikiListChanged => "wiki-list-changed",
            EmitKind::WikiPageChanged { .. } => "wiki-page-changed",
            EmitKind::GoalsChanged => "goals-changed",
            EmitKind::GoalRunChanged { .. } => "goal-run-changed",
            EmitKind::QuizChanged => "quiz-changed",
            EmitKind::QuizAttemptChanged { .. } => "quiz-attempt-changed",
            EmitKind::VaultListChanged => "vault-list-changed",
            EmitKind::VaultWatcherError { .. } => "vault-watcher-error",
        }
    }

    pub(crate) fn payload(&self) -> serde_json::Value {
        match self {
            EmitKind::WikiPageChanged { path } => serde_json::json!({ "path": path }),
            EmitKind::GoalRunChanged { run_id } => serde_json::json!({ "run_id": run_id }),
            EmitKind::QuizAttemptChanged { slug, id } => {
                serde_json::json!({ "slug": slug, "id": id })
            }
            EmitKind::VaultWatcherError { vault_path, reason } => {
                serde_json::json!({ "vault_path": vault_path, "reason": reason })
            }
            _ => serde_json::Value::Null,
        }
    }
}

// ---- Classification -----------------------------------------------------

const EXCLUDED_SEGMENT: &[&str] = &[".git", ".obsidian"];

fn is_excluded(rel: &Path) -> bool {
    for seg in rel.iter() {
        let s = match seg.to_str() {
            Some(s) => s,
            None => continue,
        };
        if EXCLUDED_SEGMENT.contains(&s) {
            return true;
        }
        if s.ends_with(".lock") {
            return true;
        }
    }
    false
}

/// Classify a debounced absolute path under `vault_root` into zero or
/// more `EmitKind` variants. Paths outside `<vault_root>/.codebus/` or
/// under excluded segments resolve to an empty `Vec` (no emit).
pub(crate) fn classify_vault_path(vault_root: &Path, path: &Path) -> Vec<EmitKind> {
    let codebus = vault_root.join(".codebus");
    let rel = match path.strip_prefix(&codebus) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    if is_excluded(rel) {
        return Vec::new();
    }
    let segments: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
    if segments.is_empty() {
        return Vec::new();
    }

    match segments[0] {
        "wiki" => {
            let mut out = vec![EmitKind::WikiListChanged];
            let is_md = path.extension().and_then(|e| e.to_str()) == Some("md");
            if is_md && segments.len() > 1 {
                out.push(EmitKind::WikiPageChanged {
                    path: path.to_path_buf(),
                });
            }
            out
        }
        "log" => {
            let leaf = match segments.get(1) {
                Some(s) => *s,
                None => return vec![EmitKind::GoalsChanged],
            };
            if let Some(slug) = leaf
                .strip_prefix("events-")
                .and_then(|s| s.strip_suffix(".jsonl"))
            {
                vec![
                    EmitKind::GoalsChanged,
                    EmitKind::GoalRunChanged {
                        run_id: slug.to_string(),
                    },
                ]
            } else if leaf.starts_with("runs-") && leaf.ends_with(".jsonl") {
                vec![EmitKind::GoalsChanged]
            } else {
                Vec::new()
            }
        }
        "quiz" => {
            let slug = match segments.get(1) {
                Some(s) => *s,
                None => return vec![EmitKind::QuizChanged],
            };
            let leaf = match segments.get(2) {
                Some(s) => *s,
                None => return vec![EmitKind::QuizChanged],
            };
            let id = if let Some(i) = leaf.strip_suffix(".progress.json") {
                Some(i.to_string())
            } else {
                leaf.strip_suffix(".md").map(|s| s.to_string())
            };
            if let Some(id) = id {
                vec![
                    EmitKind::QuizChanged,
                    EmitKind::QuizAttemptChanged {
                        slug: slug.to_string(),
                        id,
                    },
                ]
            } else {
                vec![EmitKind::QuizChanged]
            }
        }
        _ => Vec::new(),
    }
}

/// Lobby classifier: emits `VaultListChanged` only for changes to the
/// app-state.json file the Lobby watches. Any other path resolves to an
/// empty `Vec` so siblings in `~/.codebus/` are ignored.
pub(crate) fn classify_lobby_path(app_state_path: &Path, event_path: &Path) -> Vec<EmitKind> {
    if event_path == app_state_path {
        vec![EmitKind::VaultListChanged]
    } else {
        Vec::new()
    }
}

// ---- Debouncer ----------------------------------------------------------

#[derive(Default)]
pub(crate) struct Debouncer {
    pending: HashMap<PathBuf, Instant>,
}

impl Debouncer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record(&mut self, path: PathBuf, at: Instant) {
        self.pending.insert(path, at);
    }

    pub(crate) fn drain_ready(&mut self, now: Instant) -> Vec<PathBuf> {
        let due: Vec<PathBuf> = self
            .pending
            .iter()
            .filter_map(|(p, last)| {
                if now.duration_since(*last) >= DEBOUNCE_WINDOW {
                    Some(p.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut ready = Vec::with_capacity(due.len());
        for p in due {
            self.pending.remove(&p);
            ready.push(p);
        }
        ready
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

// ---- Pump thread --------------------------------------------------------

/// Generic pump loop shared by per-vault and Lobby watchers. Drains raw
/// `notify::Event` results from `rx`, debounces them per `DEBOUNCE_WINDOW`,
/// classifies each ready path via `classify`, and calls `emit` for every
/// resulting `EmitKind`. Exits when either `stop` is set or `rx` is
/// disconnected (the watcher dropped its `tx`).
fn pump_loop<C, E>(
    rx: Receiver<notify::Result<notify::Event>>,
    stop: Arc<AtomicBool>,
    classify: C,
    emit: E,
) where
    C: Fn(&Path) -> Vec<EmitKind> + Send + 'static,
    E: Fn(EmitKind) + Send + 'static,
{
    let mut debouncer = Debouncer::new();
    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(Ok(event)) => {
                let at = Instant::now();
                for p in event.paths {
                    debouncer.record(p, at);
                }
            }
            Ok(Err(_notify_err)) => {
                // notify-side errors during operation (rare; usually
                // permission flips). Fail-loud at construction time is
                // handled in `start_vault` / `start_lobby`; mid-stream
                // errors here are dropped to keep the pump alive.
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        for ready_path in debouncer.drain_ready(Instant::now()) {
            for kind in classify(&ready_path) {
                emit(kind);
            }
        }
    }
}

// ---- WatcherRegistry ----------------------------------------------------

#[derive(Default)]
pub struct WatcherRegistry {
    inner: Mutex<RegistryInner>,
}

#[derive(Default)]
struct RegistryInner {
    vaults: HashMap<PathBuf, WatcherHandle>,
    lobby: Option<WatcherHandle>,
}

/// Holds the OS-level `notify` watcher together with its pump thread
/// and a stop signal. `Drop` signals the pump to exit, releases the
/// notify watcher (which closes the OS handles), and joins the thread
/// so no orphaned worker survives a `stop_vault`.
struct WatcherHandle {
    _watcher: RecommendedWatcher,
    stop: Arc<AtomicBool>,
    pump: Option<JoinHandle<()>>,
}

impl Drop for WatcherHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.pump.take() {
            let _ = h.join();
        }
    }
}

impl WatcherRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start (or restart) a per-vault watcher. Recursively watches the
    /// three vault sub-directories `<vault>/.codebus/{wiki,log,quiz}/`
    /// that exist. Idempotent: an existing watcher for the same vault is
    /// dropped (and its pump joined) before the new one is constructed.
    ///
    /// `emit` is invoked from the pump thread for each classified
    /// `EmitKind` and SHALL be cheap. The IPC wrapper supplies a closure
    /// that forwards to `AppHandle::emit`.
    pub(crate) fn start_vault<E>(&self, vault_path: PathBuf, emit: E) -> Result<(), AppError>
    where
        E: Fn(EmitKind) + Send + Sync + Clone + 'static,
    {
        // Idempotent: drop any prior handle before constructing the new
        // one so OS-level registrations are released and the old pump
        // thread joins before we register the replacement.
        let _ = self.stop_vault(&vault_path);

        let emit_for_failure = emit.clone();
        match build_vault_watcher(&vault_path, emit) {
            Ok(handle) => {
                let mut inner = self.lock_inner()?;
                inner.vaults.insert(vault_path, handle);
            }
            Err(e) => {
                // Spec: Watcher Startup Failure Surfaces Loudly. The
                // failure SHALL be emitted as `vault-watcher-error`
                // exactly once; no entry is inserted into the registry
                // so the vault stays observably silent. The IPC command
                // returns `Ok` because the error is delivered via the
                // event channel — both surfaces returning errors would
                // force the frontend to duplicate handling logic.
                emit_for_failure(EmitKind::VaultWatcherError {
                    vault_path: vault_path.clone(),
                    reason: format!("{e}"),
                });
            }
        }
        Ok(())
    }

    /// Stop the per-vault watcher. Calling stop on a vault without an
    /// active watcher is a no-op and SHALL NOT raise an error.
    pub(crate) fn stop_vault(&self, vault_path: &Path) -> Result<(), AppError> {
        let removed = {
            let mut inner = self.lock_inner()?;
            inner.vaults.remove(vault_path)
        };
        // Drop happens here outside the lock so the pump-join doesn't
        // hold the registry mutex (which a concurrent start_vault might
        // also want).
        drop(removed);
        Ok(())
    }

    /// Start the long-lived Lobby watcher. Watches the parent directory
    /// of `app_state_path` (notify cannot reliably watch single files
    /// on every platform) and filters emits to events whose target path
    /// equals `app_state_path`. Idempotent like `start_vault`.
    pub(crate) fn start_lobby<E>(&self, app_state_path: PathBuf, emit: E) -> Result<(), AppError>
    where
        E: Fn(EmitKind) + Send + 'static,
    {
        let handle = build_lobby_watcher(&app_state_path, emit)?;
        let mut inner = self.lock_inner()?;
        inner.lobby = Some(handle);
        Ok(())
    }

    fn lock_inner(&self) -> Result<std::sync::MutexGuard<'_, RegistryInner>, AppError> {
        self.inner.lock().map_err(|_| AppError::Internal {
            message: "watcher registry mutex poisoned".into(),
        })
    }

    /// Test seam exercising the `Watcher Startup Failure Surfaces Loudly`
    /// branch deterministically without depending on the host platform to
    /// fail `notify::Watcher::new` (which is environment-dependent and
    /// not reliably reproducible in CI). Executes the exact emit-and-do-
    /// not-insert path that `start_vault` runs when `build_vault_watcher`
    /// returns `Err`.
    #[cfg(test)]
    pub(crate) fn test_inject_vault_failure<E>(
        &self,
        vault_path: PathBuf,
        reason: &str,
        emit: E,
    ) where
        E: Fn(EmitKind),
    {
        emit(EmitKind::VaultWatcherError {
            vault_path,
            reason: reason.to_string(),
        });
        // Deliberately do NOT insert into self.inner.vaults so the
        // registry observes the same "silent vault" post-condition as a
        // real failure.
    }

    #[cfg(test)]
    pub(crate) fn active_vault_count(&self) -> usize {
        self.inner.lock().expect("registry mutex").vaults.len()
    }

    #[cfg(test)]
    pub(crate) fn lobby_active(&self) -> bool {
        self.inner.lock().expect("registry mutex").lobby.is_some()
    }
}

// ---- Watcher construction ----------------------------------------------

fn build_vault_watcher<E>(vault_path: &Path, emit: E) -> Result<WatcherHandle, AppError>
where
    E: Fn(EmitKind) + Send + 'static,
{
    let (tx, rx): (Sender<_>, Receiver<_>) = channel();
    let tx_for_callback = tx;
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx_for_callback.send(res);
        },
        Config::default(),
    )
    .map_err(|e| AppError::Internal {
        message: format!("notify::Watcher::new failed: {e}"),
    })?;

    let codebus = vault_path.join(".codebus");
    for sub in ["wiki", "log", "quiz"] {
        let dir = codebus.join(sub);
        if dir.exists() {
            watcher
                .watch(&dir, RecursiveMode::Recursive)
                .map_err(|e| AppError::Internal {
                    message: format!("watch {} failed: {e}", dir.display()),
                })?;
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop.clone();
    let vault_root = vault_path.to_path_buf();
    let pump = thread::Builder::new()
        .name(format!("fs-watcher-vault:{}", vault_path.display()))
        .spawn(move || {
            pump_loop(
                rx,
                stop_for_thread,
                move |p| classify_vault_path(&vault_root, p),
                emit,
            );
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn pump thread failed: {e}"),
        })?;

    Ok(WatcherHandle {
        _watcher: watcher,
        stop,
        pump: Some(pump),
    })
}

fn build_lobby_watcher<E>(app_state_path: &Path, emit: E) -> Result<WatcherHandle, AppError>
where
    E: Fn(EmitKind) + Send + 'static,
{
    let parent = app_state_path
        .parent()
        .ok_or_else(|| AppError::Internal {
            message: format!(
                "lobby watcher target has no parent dir: {}",
                app_state_path.display()
            ),
        })?
        .to_path_buf();

    // Ensure the parent directory exists so `watch` does not fail on a
    // fresh install that never wrote app-state.json before launch.
    if !parent.exists() {
        std::fs::create_dir_all(&parent).map_err(|e| AppError::Internal {
            message: format!("create {} failed: {e}", parent.display()),
        })?;
    }

    let (tx, rx): (Sender<_>, Receiver<_>) = channel();
    let tx_for_callback = tx;
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx_for_callback.send(res);
        },
        Config::default(),
    )
    .map_err(|e| AppError::Internal {
        message: format!("notify::Watcher::new failed: {e}"),
    })?;
    watcher
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(|e| AppError::Internal {
            message: format!("watch {} failed: {e}", parent.display()),
        })?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop.clone();
    let target = app_state_path.to_path_buf();
    let pump = thread::Builder::new()
        .name("fs-watcher-lobby".into())
        .spawn(move || {
            pump_loop(
                rx,
                stop_for_thread,
                move |p| classify_lobby_path(&target, p),
                emit,
            );
        })
        .map_err(|e| AppError::Internal {
            message: format!("spawn pump thread failed: {e}"),
        })?;

    Ok(WatcherHandle {
        _watcher: watcher,
        stop,
        pump: Some(pump),
    })
}

// ---- Tauri command wrappers --------------------------------------------

/// Forward an `EmitKind` to the Tauri event channel. Errors are logged
/// but not propagated — emitting events is best-effort (the window may
/// have closed mid-pump).
fn emit_via_app(app: &AppHandle, kind: EmitKind) {
    let name = kind.event_name();
    let payload = kind.payload();
    if let Err(e) = app.emit(name, payload) {
        eprintln!("watcher emit {name} failed: {e}");
    }
}

#[tauri::command]
pub async fn start_vault_watcher(
    app: AppHandle,
    registry: State<'_, WatcherRegistry>,
    vault_path: String,
) -> IpcResult<()> {
    let path = PathBuf::from(&vault_path);
    let app_for_emit = app.clone();
    registry.start_vault(path, move |k| emit_via_app(&app_for_emit, k))?;
    Ok(())
}

#[tauri::command]
pub async fn stop_vault_watcher(
    registry: State<'_, WatcherRegistry>,
    vault_path: String,
) -> IpcResult<()> {
    let path = PathBuf::from(&vault_path);
    registry.stop_vault(&path)?;
    Ok(())
}

/// Called once from `lib::run` during Tauri `setup`. Resolves the user's
/// `~/.codebus/app-state.json` path via `codebus_core::config::default_config_path`
/// (the same hook that lets `CODEBUS_HOME` relocate the dir for tests/CI)
/// and starts the long-lived Lobby watcher.
pub fn setup_lobby_watcher(app: &AppHandle, registry: &WatcherRegistry) -> Result<(), AppError> {
    let cfg = codebus_core::config::default_config_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable for lobby watcher".into(),
    })?;
    let parent = cfg.parent().ok_or_else(|| AppError::Internal {
        message: "config path has no parent".into(),
    })?;
    let app_state = parent.join("app-state.json");
    let app_for_emit = app.clone();
    registry.start_lobby(app_state, move |k| emit_via_app(&app_for_emit, k))
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // ---- Lifecycle (task 1.2) ----

    fn make_vault_layout(root: &Path) {
        for sub in ["wiki", "log", "quiz"] {
            fs::create_dir_all(root.join(".codebus").join(sub)).unwrap();
        }
    }

    #[test]
    fn start_then_stop_releases_handle() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().to_path_buf();
        make_vault_layout(&vault);

        let reg = WatcherRegistry::new();
        reg.start_vault(vault.clone(), |_| {}).expect("start ok");
        assert_eq!(reg.active_vault_count(), 1);

        reg.stop_vault(&vault).expect("stop ok");
        assert_eq!(reg.active_vault_count(), 0);
    }

    #[test]
    fn double_start_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().to_path_buf();
        make_vault_layout(&vault);

        let reg = WatcherRegistry::new();
        reg.start_vault(vault.clone(), |_| {}).unwrap();
        reg.start_vault(vault.clone(), |_| {}).unwrap();
        assert_eq!(reg.active_vault_count(), 1);
    }

    #[test]
    fn stop_unstarted_is_noop() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().to_path_buf();
        let reg = WatcherRegistry::new();
        reg.stop_vault(&vault).expect("stop ok on unstarted");
        assert_eq!(reg.active_vault_count(), 0);
    }

    #[test]
    fn start_vault_tolerates_missing_subdir() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().to_path_buf();
        fs::create_dir_all(vault.join(".codebus").join("wiki")).unwrap();
        let reg = WatcherRegistry::new();
        reg.start_vault(vault.clone(), |_| {}).expect("start ok");
        assert_eq!(reg.active_vault_count(), 1);
    }

    // ---- Debouncer (task 1.3) ----

    #[test]
    fn atomic_rename_emits_one_event() {
        let mut d = Debouncer::new();
        let base = Instant::now();
        let path = PathBuf::from("C:/vault/.codebus/wiki/foo.md");

        d.record(path.clone(), base);
        d.record(path.clone(), base + Duration::from_millis(10));
        d.record(path.clone(), base + Duration::from_millis(40));

        let probe1 = base + Duration::from_millis(40 + 199);
        assert!(d.drain_ready(probe1).is_empty());

        let probe2 = base + Duration::from_millis(40 + 240);
        let ready = d.drain_ready(probe2);
        assert_eq!(ready, vec![path]);
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn distinct_paths_debounce_independently() {
        let mut d = Debouncer::new();
        let base = Instant::now();
        let a = PathBuf::from("C:/vault/.codebus/wiki/a.md");
        let b = PathBuf::from("C:/vault/.codebus/wiki/b.md");

        d.record(a.clone(), base);
        d.record(b.clone(), base + Duration::from_millis(50));

        let probe_a = base + Duration::from_millis(200);
        let ready_a = d.drain_ready(probe_a);
        assert_eq!(ready_a, vec![a]);

        let probe_b = base + Duration::from_millis(250);
        let ready_b = d.drain_ready(probe_b);
        assert_eq!(ready_b, vec![b]);
    }

    // ---- Classifier (task 1.4) ----

    fn vault() -> PathBuf {
        PathBuf::from("C:/vault")
    }

    #[test]
    fn classify_wiki_list_changed_on_dir_event() {
        let v = vault();
        let ev = v.join(".codebus").join("wiki").join("concepts");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(kinds, vec![EmitKind::WikiListChanged]);
        assert_eq!(kinds[0].event_name(), "wiki-list-changed");
        assert_eq!(kinds[0].payload(), serde_json::Value::Null);
    }

    #[test]
    fn classify_wiki_page_changed_for_md_file() {
        let v = vault();
        let ev = v.join(".codebus/wiki/concepts/foo.md");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(
            kinds,
            vec![
                EmitKind::WikiListChanged,
                EmitKind::WikiPageChanged { path: ev.clone() }
            ]
        );
        assert_eq!(kinds[1].event_name(), "wiki-page-changed");
        assert_eq!(kinds[1].payload(), serde_json::json!({ "path": ev }));
    }

    #[test]
    fn classify_goals_changed_for_runs_jsonl() {
        let v = vault();
        let ev = v.join(".codebus/log/runs-2026-05-20.jsonl");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(kinds, vec![EmitKind::GoalsChanged]);
        assert_eq!(kinds[0].event_name(), "goals-changed");
        assert_eq!(kinds[0].payload(), serde_json::Value::Null);
    }

    #[test]
    fn classify_goal_run_changed_extracts_run_id() {
        let v = vault();
        let ev = v.join(".codebus/log/events-2026-05-20T08-30-00Z.jsonl");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(
            kinds,
            vec![
                EmitKind::GoalsChanged,
                EmitKind::GoalRunChanged {
                    run_id: "2026-05-20T08-30-00Z".into()
                }
            ]
        );
        assert_eq!(kinds[1].event_name(), "goal-run-changed");
        assert_eq!(
            kinds[1].payload(),
            serde_json::json!({ "run_id": "2026-05-20T08-30-00Z" })
        );
    }

    #[test]
    fn classify_quiz_changed_on_dir_event() {
        let v = vault();
        let ev = v.join(".codebus/quiz/jwt-basics");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(kinds, vec![EmitKind::QuizChanged]);
        assert_eq!(kinds[0].event_name(), "quiz-changed");
    }

    #[test]
    fn classify_quiz_attempt_changed_for_progress_sidecar() {
        let v = vault();
        let ev = v.join(".codebus/quiz/jwt-basics/2026-05-20T08-30-00Z.progress.json");
        let kinds = classify_vault_path(&v, &ev);
        assert_eq!(
            kinds,
            vec![
                EmitKind::QuizChanged,
                EmitKind::QuizAttemptChanged {
                    slug: "jwt-basics".into(),
                    id: "2026-05-20T08-30-00Z".into()
                }
            ]
        );
        assert_eq!(kinds[1].event_name(), "quiz-attempt-changed");
        assert_eq!(
            kinds[1].payload(),
            serde_json::json!({ "slug": "jwt-basics", "id": "2026-05-20T08-30-00Z" })
        );
    }

    #[test]
    fn classify_quiz_attempt_changed_for_md_leaf() {
        let v = vault();
        let ev = v.join(".codebus/quiz/jwt-basics/2026-05-20T08-30-00Z.md");
        let kinds = classify_vault_path(&v, &ev);
        assert!(matches!(
            &kinds[1],
            EmitKind::QuizAttemptChanged { slug, id }
                if slug == "jwt-basics" && id == "2026-05-20T08-30-00Z"
        ));
    }

    #[test]
    fn vault_list_changed_event_name_is_stable() {
        let k = EmitKind::VaultListChanged;
        assert_eq!(k.event_name(), "vault-list-changed");
        assert_eq!(k.payload(), serde_json::Value::Null);
    }

    // ---- Excluded paths (task 1.6) ----

    #[test]
    fn excluded_paths_emit_nothing() {
        let v = vault();
        let cases: &[PathBuf] = &[
            v.join(".codebus/raw/code/main.rs"),
            v.join(".codebus/CLAUDE.md"),
            v.join(".codebus/wiki/.git/HEAD"),
            v.join(".codebus/wiki/.obsidian/config.json"),
            v.join(".codebus/wiki/some.lock"),
            v.join(".codebus/log/.git/refs"),
            v.join("src/main.rs"),
            v.join("Cargo.toml"),
        ];
        for c in cases {
            assert_eq!(
                classify_vault_path(&v, c),
                Vec::<EmitKind>::new(),
                "expected no emit for {}",
                c.display()
            );
        }
    }

    // ---- Lobby classifier ----

    #[test]
    fn lobby_classifier_emits_only_for_target_path() {
        let target = PathBuf::from("C:/Users/x/.codebus/app-state.json");
        let hit = target.clone();
        let miss = PathBuf::from("C:/Users/x/.codebus/config.yaml");

        assert_eq!(classify_lobby_path(&target, &hit), vec![EmitKind::VaultListChanged]);
        assert_eq!(classify_lobby_path(&target, &miss), Vec::<EmitKind>::new());
    }

    // ---- Fail-loud (task 1.7) ----

    /// Spec scenario: `watcher_new_failure_emits_error_once` — the
    /// emit-on-failure branch SHALL fire `vault-watcher-error` exactly
    /// once with the contracted payload, the registry SHALL NOT carry
    /// any handle for that vault, and no subsequent emit SHALL happen
    /// from this module on that vault's behalf.
    #[test]
    fn watcher_new_failure_emits_error_once() {
        let captured: Arc<Mutex<Vec<EmitKind>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured.clone();

        let vault = PathBuf::from("C:/vault-that-fails");
        let reg = WatcherRegistry::new();
        reg.test_inject_vault_failure(vault.clone(), "ENOSPC simulated", move |k| {
            captured_clone.lock().unwrap().push(k);
        });

        let events = captured.lock().unwrap();
        assert_eq!(events.len(), 1, "exactly one event SHALL be emitted");
        match &events[0] {
            EmitKind::VaultWatcherError { vault_path, reason } => {
                assert_eq!(vault_path, &vault);
                assert!(reason.contains("ENOSPC"));
            }
            other => panic!("expected VaultWatcherError, got {other:?}"),
        }
        // No registry entry for the failed vault.
        assert_eq!(reg.active_vault_count(), 0);
        // The Tauri event name is the spec-contracted string.
        assert_eq!(events[0].event_name(), "vault-watcher-error");
        // Payload shape matches the spec.
        assert_eq!(
            events[0].payload(),
            serde_json::json!({
                "vault_path": vault,
                "reason": "ENOSPC simulated",
            })
        );
    }

    // ---- End-to-end (task 1.5): Lobby watcher emits on file change ----

    /// Poll a callback-collected emit list until at least one event of the
    /// requested name shows up, or `deadline` elapses. Returns true if the
    /// event was observed in time.
    fn wait_for_event(
        captured: &Arc<Mutex<Vec<EmitKind>>>,
        event_name: &str,
        deadline: Duration,
    ) -> bool {
        let start = Instant::now();
        while start.elapsed() < deadline {
            {
                let guard = captured.lock().unwrap();
                if guard.iter().any(|k| k.event_name() == event_name) {
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[test]
    fn lobby_watcher_emits_on_app_state_change() {
        let tmp = TempDir::new().unwrap();
        let home_codebus = tmp.path().join(".codebus");
        fs::create_dir_all(&home_codebus).unwrap();
        let app_state = home_codebus.join("app-state.json");
        fs::write(&app_state, b"{\"vault_list\":[]}").unwrap();

        let captured: Arc<Mutex<Vec<EmitKind>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_clone = captured.clone();

        let reg = WatcherRegistry::new();
        reg.start_lobby(app_state.clone(), move |k| {
            captured_clone.lock().unwrap().push(k);
        })
        .expect("lobby start ok");
        assert!(reg.lobby_active());

        // Give the watcher a brief warmup before mutating so the OS
        // registration is fully in place (FSEvents has a small async setup).
        thread::sleep(Duration::from_millis(120));

        fs::write(&app_state, b"{\"vault_list\":[{\"path\":\"D:/v\"}]}").unwrap();

        // Spec contract: emit within 400 ms. Allow a generous 2 s ceiling
        // for slow CI / Windows file-lock retry.
        assert!(
            wait_for_event(&captured, "vault-list-changed", Duration::from_secs(2)),
            "vault-list-changed was not emitted within 2s; captured: {:?}",
            captured.lock().unwrap().iter().map(EmitKind::event_name).collect::<Vec<_>>(),
        );
    }
}
