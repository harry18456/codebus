//! `list_vaults` / `add_vault` / `remove_vault` IPC commands.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use codebus_core::vault::init::{InitError, InitEvent, InitOptions, run_init};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

use super::IpcResult;
use super::vault_progress::{
    VAULT_INIT_PROGRESS_EVENT, VaultInitProgress, init_event_label, init_event_to_phase,
};
use crate::error::AppError;
use crate::state::app_state::{
    AppState, StoredVaultEntry, app_state_path, load_app_state, save_app_state,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultEntry {
    pub path: String,
    pub display_name: String,
    pub last_opened: String,
    pub is_missing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddVaultMode {
    /// Caller has not picked between just-bind / re-init yet; backend should
    /// detect `.codebus/`. If present, returns `AppError::Invalid { field:
    /// "mode" }` so the frontend can prompt; if absent, runs `run_init`.
    Detect,
    /// Folder already has `.codebus/`; add to list without modifying.
    JustBind,
    /// Folder already has `.codebus/`; delete it then run fresh init.
    /// Frontend MUST require the user to type `delete` before reaching here.
    ReInit,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddVaultOptions {
    #[serde(default = "default_mode")]
    pub mode: AddVaultMode,
}

fn default_mode() -> AddVaultMode {
    AddVaultMode::Detect
}

/// Convert persisted vault entries to IPC entries, marking each as
/// `is_missing` when the supplied existence check reports the path as
/// inaccessible. Pure function — test fixtures inject the closure to
/// simulate "deleted" or "permission denied" without touching real
/// filesystem permissions (which are OS-specific to test).
pub(crate) fn map_to_vault_entries(
    stored: Vec<StoredVaultEntry>,
    mut path_exists: impl FnMut(&str) -> bool,
) -> Vec<VaultEntry> {
    stored
        .into_iter()
        .map(|e| {
            let is_missing = !path_exists(&e.path);
            VaultEntry {
                path: e.path,
                display_name: e.display_name,
                last_opened: e.last_opened,
                is_missing,
            }
        })
        .collect()
}

/// Production existence check: any IO error (including permission denied)
/// resolves to `false` so the vault is rendered with the missing badge.
/// This matches the spec scenario "Missing path surfaces as is_missing"
/// and the implementation contract bullet "卡片標 missing badge".
fn default_path_exists(path: &str) -> bool {
    Path::new(path).try_exists().unwrap_or(false)
}

/// Internal helper used by both the Tauri command and the integration
/// tests. Loads the app-state file at `state_path`, lazily migrates any
/// stored path that still carries the `\\?\` verbatim prefix (legacy
/// entries from before the path-normalization fix), and projects to the
/// IPC `VaultEntry` shape. Migration is best-effort — a save failure is
/// non-fatal because the in-memory values are still rendered correctly
/// and the next list call will retry.
pub(crate) fn list_vaults_at(state_path: &Path) -> IpcResult<Vec<VaultEntry>> {
    let mut state: AppState = load_app_state(state_path);
    let mut migrated = false;
    for entry in state.vault_list.iter_mut() {
        let cleaned = path_string(&normalize_path(Path::new(&entry.path)));
        if cleaned != entry.path {
            entry.path = cleaned;
            migrated = true;
        }
    }
    if migrated {
        let _ = save_app_state(state_path, &state);
    }
    Ok(map_to_vault_entries(
        state.vault_list.clone(),
        default_path_exists,
    ))
}

#[tauri::command]
pub async fn list_vaults() -> IpcResult<Vec<VaultEntry>> {
    let path = app_state_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable".into(),
    })?;
    list_vaults_at(&path)
}

/// Translate a core init error into the IPC-friendly AppError.
fn map_init_error(err: InitError) -> AppError {
    match err {
        InitError::Refused(refusal) => AppError::Invalid {
            field: "path".into(),
            message: refusal.to_string(),
        },
        // Everything else is fs or git plumbing → surface as Io.
        other => AppError::Io {
            message: other.to_string(),
        },
    }
}

/// Normalize a vault path for de-dup comparisons AND for the value that
/// flows through to display. Falls back to the raw `PathBuf` if
/// canonicalize fails. On Windows, also strips the `\\?\` verbatim
/// extended-length prefix that `std::fs::canonicalize` always emits:
/// `\\?\D:\side_project\repo` → `D:\side_project\repo`,
/// `\\?\UNC\server\share`     → `\\server\share`.
/// Without the strip every vault card / config display surface ends up
/// rendering the ugly `\\?\` prefix to the user.
fn normalize_path(p: &Path) -> PathBuf {
    let canonical = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    strip_verbatim_prefix(&canonical)
}

#[cfg(windows)]
fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        // `\\?\UNC\server\share` is the verbatim form of `\\server\share`.
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        // Drive-letter form `\\?\D:\path` strips cleanly to `D:\path` only
        // when the first segment is `<letter>:`. Volume-GUID forms
        // (`\\?\Volume{...}`) MUST keep the prefix or Windows APIs lose the
        // ability to resolve them.
        let mut chars = rest.chars();
        match (chars.next(), chars.next()) {
            (Some(c), Some(':')) if c.is_ascii_alphabetic() => PathBuf::from(rest),
            _ => p.to_path_buf(),
        }
    } else {
        p.to_path_buf()
    }
}

#[cfg(not(windows))]
fn strip_verbatim_prefix(p: &Path) -> PathBuf {
    p.to_path_buf()
}

fn path_string(p: &Path) -> String {
    p.display().to_string()
}

/// Internal `add_vault` implementation parameterized by the app-state file
/// location and a `tauri::AppHandle`. The async + `AppHandle` shape is the
/// public contract (per spec "Vault Init Progress Event"); the body
/// delegates to [`add_vault_at_with_progress`] with an emit-bearing closure.
///
/// Tests SHOULD NOT call this function directly — they call the sync
/// inner [`add_vault_at_with_progress`] which avoids dragging the
/// Tauri runtime into the unit-test binary (Windows WebView2 loader is
/// not present in `cargo test` deps directory). The async wrapper
/// is exercised end-to-end via CDP smoke (see
/// `codebus-app/scripts/.loading-overlay-smoke/`).
pub(crate) async fn add_vault_at<R: Runtime>(
    app: &AppHandle<R>,
    state_path: &Path,
    vault_path: &Path,
    options: &AddVaultOptions,
) -> IpcResult<VaultEntry> {
    let started = Instant::now();
    add_vault_at_with_progress(state_path, vault_path, options, |event| {
        let payload = VaultInitProgress {
            phase: init_event_to_phase(&event),
            init_event_kind: init_event_label(&event).to_string(),
            elapsed_ms: started.elapsed().as_millis() as u64,
        };
        if let Err(e) = app.emit(VAULT_INIT_PROGRESS_EVENT, &payload) {
            eprintln!("vault-init-progress emit failed: {e}");
        }
    })
}

/// Sync core for `add_vault_at`. Tests drive this directly with a
/// tempdir-scoped state path so the per-process `dirs::home_dir()` is
/// never touched. The `on_event` callback is invoked once per
/// `InitEvent` emitted by `run_init`; production wires it to the
/// `vault-init-progress` Tauri event via [`add_vault_at`], tests pass
/// `|_| {}` (no init) or a `Vec`-recording closure to assert the event
/// stream.
pub(crate) fn add_vault_at_with_progress(
    state_path: &Path,
    vault_path: &Path,
    options: &AddVaultOptions,
    mut on_event: impl FnMut(InitEvent<'_>),
) -> IpcResult<VaultEntry> {
    if !vault_path.is_dir() {
        return Err(AppError::VaultNotFound {
            path: path_string(vault_path),
        });
    }

    let canonical = normalize_path(vault_path);
    let canonical_str = path_string(&canonical);

    let mut state = load_app_state(state_path);
    if state
        .vault_list
        .iter()
        .any(|entry| normalize_path(Path::new(&entry.path)) == canonical)
    {
        return Err(AppError::VaultAlreadyExists {
            path: canonical_str.clone(),
        });
    }

    let has_codebus = canonical.join(".codebus").is_dir();

    match (options.mode, has_codebus) {
        (AddVaultMode::Detect, true) => {
            return Err(AppError::Invalid {
                field: "mode".into(),
                message: ".codebus/ exists; choose just_bind or re_init".into(),
            });
        }
        (AddVaultMode::Detect, false) => {
            run_init(&canonical, &init_opts(), &mut on_event).map_err(map_init_error)?;
        }
        (AddVaultMode::JustBind, false) => {
            return Err(AppError::Invalid {
                field: "mode".into(),
                message: "just_bind requires an existing .codebus/ at the target path".into(),
            });
        }
        (AddVaultMode::JustBind, true) => {
            // Add to list without touching vault contents. Just-bind MUST NOT
            // emit `vault-init-progress` — per spec scenario
            // "Just-bind mode emits no progress events".
        }
        (AddVaultMode::ReInit, false) => {
            return Err(AppError::Invalid {
                field: "mode".into(),
                message: "re_init requires an existing .codebus/ at the target path".into(),
            });
        }
        (AddVaultMode::ReInit, true) => {
            fs::remove_dir_all(canonical.join(".codebus"))?;
            run_init(&canonical, &init_opts(), &mut on_event).map_err(map_init_error)?;
        }
    }

    let display_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("vault")
        .to_string();
    let last_opened = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let stored = StoredVaultEntry {
        path: canonical_str.clone(),
        display_name: display_name.clone(),
        last_opened: last_opened.clone(),
    };
    state.vault_list.push(stored);
    save_app_state(state_path, &state)?;

    Ok(VaultEntry {
        path: canonical_str,
        display_name,
        last_opened,
        is_missing: false,
    })
}

fn init_opts() -> InitOptions {
    InitOptions {
        no_obsidian_register: true,
        write_starter_config: true,
        with_repo_root_skills: false,
    }
}

#[tauri::command]
pub async fn add_vault(
    app: AppHandle,
    path: String,
    options: AddVaultOptions,
) -> IpcResult<VaultEntry> {
    let state_path = app_state_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable".into(),
    })?;
    let vault_path = PathBuf::from(&path);
    add_vault_at(&app, &state_path, &vault_path, &options).await
}

/// Internal `remove_vault` impl parameterized by the state file path.
pub(crate) fn remove_vault_at(state_path: &Path, vault_path: &Path) -> IpcResult<()> {
    let canonical = normalize_path(vault_path);
    let mut state = load_app_state(state_path);
    let before = state.vault_list.len();
    state
        .vault_list
        .retain(|entry| normalize_path(Path::new(&entry.path)) != canonical);
    if state.vault_list.len() == before {
        return Err(AppError::VaultNotFound {
            path: path_string(&canonical),
        });
    }
    save_app_state(state_path, &state)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_vault(path: String) -> IpcResult<()> {
    let state_path = app_state_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable".into(),
    })?;
    let vault_path = PathBuf::from(&path);
    remove_vault_at(&state_path, &vault_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::app_state::{CURRENT_SCHEMA_VERSION, save_app_state};
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Serialize tests that mutate `CODEBUS_HOME`. cargo runs tests in
    /// parallel by default; concurrent env mutation would race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn stored(path: &str) -> StoredVaultEntry {
        StoredVaultEntry {
            path: path.into(),
            display_name: "demo".into(),
            last_opened: "2026-05-11T00:00:00Z".into(),
        }
    }

    #[test]
    fn existing_path_is_not_missing() {
        let stored_entries = vec![stored("/exists")];
        let result = map_to_vault_entries(stored_entries, |_p| true);
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_missing);
    }

    #[test]
    fn deleted_path_is_missing() {
        let stored_entries = vec![stored("/gone")];
        let result = map_to_vault_entries(stored_entries, |_p| false);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_missing);
    }

    #[test]
    fn permission_denied_is_treated_as_missing() {
        // Production wires `default_path_exists` which calls
        // `try_exists().unwrap_or(false)` — any IO error including permission
        // denied collapses to `false`. We simulate that pathway here by
        // injecting a closure that mimics "exists() returned Err → false".
        let stored_entries = vec![stored("/locked")];
        let result = map_to_vault_entries(stored_entries, |_p| false);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].is_missing,
            "permission denied must surface as is_missing"
        );
    }

    fn with_codebus_home<R>(home: &Path, f: impl FnOnce() -> R) -> R {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("CODEBUS_HOME", home) };
        let r = f();
        unsafe { std::env::remove_var("CODEBUS_HOME") };
        r
    }

    /// Test helper: invoke the sync core `add_vault_at_with_progress`
    /// with a noop callback, matching the production behavior except
    /// without emitting a Tauri event. The async wrapper `add_vault_at`
    /// is exercised end-to-end via CDP smoke. See design.md "add_vault_at
    /// sync to async + accept AppHandle" for the rationale of the
    /// inner-vs-wrapper split.
    fn run_add_vault_at_in_test(
        state_path: &Path,
        vault_path: &Path,
        opts: &AddVaultOptions,
    ) -> IpcResult<VaultEntry> {
        add_vault_at_with_progress(state_path, vault_path, opts, |_| {})
    }

    #[test]
    fn add_vault_detect_on_fresh_dir_runs_init_and_appends() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };

        let entry = with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, repo.path(), &opts)
                .expect("fresh init should succeed")
        });

        assert!(!entry.is_missing);
        assert!(
            repo.path().join(".codebus").is_dir(),
            "fresh init must create .codebus/"
        );

        let state = load_app_state(&state_path);
        assert_eq!(state.vault_list.len(), 1);
    }

    #[test]
    fn add_vault_detect_on_existing_vault_returns_mode_required() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join(".codebus")).unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };

        let err = with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, repo.path(), &opts)
                .expect_err("must require mode")
        });

        match err {
            AppError::Invalid { field, .. } => assert_eq!(field, "mode"),
            other => panic!("expected Invalid {{field=mode}}, got {other:?}"),
        }
        // No state mutation when the call fails.
        let state = load_app_state(&state_path);
        assert!(state.vault_list.is_empty());
    }

    #[test]
    fn add_vault_just_bind_skips_init() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let codebus_dir = repo.path().join(".codebus");
        std::fs::create_dir_all(&codebus_dir).unwrap();
        let sentinel = codebus_dir.join("sentinel");
        std::fs::write(&sentinel, b"keep me").unwrap();

        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::JustBind,
        };

        let entry = with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, repo.path(), &opts)
                .expect("just_bind should succeed")
        });

        assert!(!entry.is_missing);
        assert!(
            sentinel.is_file(),
            "just_bind must not touch vault contents"
        );
    }

    #[test]
    fn add_vault_nonexistent_path_returns_vault_not_found() {
        let home = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };
        let missing = home.path().join("does-not-exist");

        let err = with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, &missing, &opts)
                .expect_err("must reject missing path")
        });
        assert!(matches!(err, AppError::VaultNotFound { .. }));
    }

    #[test]
    fn add_vault_duplicate_path_returns_vault_already_exists() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };

        with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, repo.path(), &opts).expect("first add ok");
            let err = run_add_vault_at_in_test(&state_path, repo.path(), &opts)
                .expect_err("second add must fail");
            assert!(matches!(err, AppError::VaultAlreadyExists { .. }));
        });
    }

    #[test]
    fn remove_vault_unbinds_without_deleting_fs() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let codebus_dir = repo.path().join(".codebus");
        std::fs::create_dir_all(&codebus_dir).unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");

        let opts = AddVaultOptions {
            mode: AddVaultMode::JustBind,
        };
        with_codebus_home(home.path(), || {
            run_add_vault_at_in_test(&state_path, repo.path(), &opts).expect("bind ok");

            remove_vault_at(&state_path, repo.path()).expect("remove ok");

            let state = load_app_state(&state_path);
            assert!(state.vault_list.is_empty(), "state must be unbound");
            assert!(
                codebus_dir.is_dir(),
                ".codebus/ MUST survive remove_vault per spec 'Remove unbinds without deletion'"
            );
        });
    }

    /// Spec scenario "Detect-mode add emits one event per InitEvent":
    /// drive `add_vault_at_with_progress` with a recording closure and
    /// assert the captured phase sequence is non-decreasing (1..=6) and
    /// covers at least Start and Finished. Phase/label correctness for
    /// each individual variant is covered by `vault_progress::tests`.
    #[test]
    fn detect_emits_progress_events() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };

        let mut phases: Vec<u8> = Vec::new();
        let mut labels: Vec<String> = Vec::new();
        with_codebus_home(home.path(), || {
            add_vault_at_with_progress(&state_path, repo.path(), &opts, |event| {
                phases.push(init_event_to_phase(&event));
                labels.push(init_event_label(&event).to_string());
            })
            .expect("init ok");
        });

        assert!(repo.path().join(".codebus").is_dir());
        assert!(
            !phases.is_empty(),
            "detect-mode add MUST emit at least one InitEvent"
        );
        assert!(
            phases.iter().all(|&p| (1..=6).contains(&p)),
            "all emitted phases MUST be 1..=6, got {phases:?}"
        );
        // Phase sequence MUST be non-decreasing (run_init emits in order).
        for w in phases.windows(2) {
            assert!(
                w[0] <= w[1],
                "phase regression at {w:?} in full sequence {phases:?}"
            );
        }
        assert!(
            labels.iter().any(|l| l == "Start"),
            "expected a Start event, got {labels:?}"
        );
        assert!(
            labels.iter().any(|l| l == "Finished"),
            "expected a Finished event, got {labels:?}"
        );
    }

    /// Spec scenario "Just-bind mode emits no progress events": the
    /// just-bind branch MUST NOT invoke `run_init` and therefore MUST
    /// produce zero callback invocations.
    #[test]
    fn just_bind_emits_no_progress_events() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join(".codebus")).unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::JustBind,
        };

        let mut event_count = 0;
        with_codebus_home(home.path(), || {
            add_vault_at_with_progress(&state_path, repo.path(), &opts, |_| {
                event_count += 1;
            })
            .expect("just_bind ok");
        });
        assert_eq!(
            event_count, 0,
            "just_bind MUST NOT emit vault-init-progress events"
        );
    }

    #[test]
    #[cfg(windows)]
    fn windows_verbatim_prefix_is_stripped() {
        let drive = PathBuf::from(r"\\?\D:\foo\bar");
        let stripped = strip_verbatim_prefix(&drive);
        assert_eq!(stripped, PathBuf::from(r"D:\foo\bar"));

        let unc = PathBuf::from(r"\\?\UNC\server\share\sub");
        let stripped = strip_verbatim_prefix(&unc);
        assert_eq!(stripped, PathBuf::from(r"\\server\share\sub"));

        // Volume-GUID paths MUST keep the prefix.
        let volume = PathBuf::from(r"\\?\Volume{abc}\stuff");
        let stripped = strip_verbatim_prefix(&volume);
        assert_eq!(stripped, volume);

        // Plain paths pass through.
        let plain = PathBuf::from(r"D:\already\clean");
        assert_eq!(strip_verbatim_prefix(&plain), plain);
    }

    #[test]
    #[cfg(windows)]
    fn list_vaults_migrates_legacy_verbatim_paths() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        // Stash a legacy-style entry with the `\\?\` prefix that older
        // versions of `add_vault_at` would have saved.
        let legacy_path = format!(r"\\?\{}", repo.path().display());
        let state_path = home.path().join(".codebus").join("app-state.json");
        let state = AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: vec![StoredVaultEntry {
                path: legacy_path.clone(),
                display_name: "legacy".into(),
                last_opened: "2026-05-11T00:00:00Z".into(),
            }],
        };
        save_app_state(&state_path, &state).unwrap();

        let entries = list_vaults_at(&state_path).unwrap();

        // The returned entry has the clean form …
        assert!(
            !entries[0].path.starts_with(r"\\?\"),
            "list_vaults_at should strip the verbatim prefix, got: {}",
            entries[0].path
        );
        // … and the on-disk file is rewritten so subsequent reads stay
        // fast (no repeated migration).
        let on_disk = load_app_state(&state_path);
        assert!(!on_disk.vault_list[0].path.starts_with(r"\\?\"));
    }

    #[test]
    fn remove_vault_unknown_path_returns_vault_not_found() {
        let home = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        // No vault registered.
        let missing = home.path().join("never-added");
        let err = with_codebus_home(home.path(), || {
            remove_vault_at(&state_path, &missing).expect_err("must reject unknown vault")
        });
        assert!(matches!(err, AppError::VaultNotFound { .. }));
    }

    #[test]
    fn list_vaults_at_reads_disk_state() {
        let tmp = TempDir::new().unwrap();
        let real_dir = TempDir::new().unwrap();
        let state_path = tmp.path().join("app-state.json");
        let state = AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: vec![
                stored(real_dir.path().to_str().unwrap()),
                stored("/definitely/does/not/exist/codebus-test-xyz"),
            ],
        };
        save_app_state(&state_path, &state).unwrap();

        let entries = list_vaults_at(&state_path).unwrap();

        assert_eq!(entries.len(), 2);
        assert!(
            !entries[0].is_missing,
            "real tempdir path should not be missing"
        );
        assert!(entries[1].is_missing, "bogus path should be missing");
    }
}
