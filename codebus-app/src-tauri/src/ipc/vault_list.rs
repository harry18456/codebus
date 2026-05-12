//! `list_vaults` / `add_vault` / `remove_vault` IPC commands.

use std::fs;
use std::path::{Path, PathBuf};

use codebus_core::vault::init::{InitError, InitOptions, run_init};
use serde::{Deserialize, Serialize};

use super::IpcResult;
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
/// location. Tests drive this directly with a tempdir-scoped state path so
/// the per-process `dirs::home_dir()` is never touched.
pub(crate) fn add_vault_at(
    state_path: &Path,
    vault_path: &Path,
    options: &AddVaultOptions,
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
            run_init(&canonical, &init_opts(), |_| {}).map_err(map_init_error)?;
        }
        (AddVaultMode::JustBind, false) => {
            return Err(AppError::Invalid {
                field: "mode".into(),
                message: "just_bind requires an existing .codebus/ at the target path".into(),
            });
        }
        (AddVaultMode::JustBind, true) => {
            // Add to list without touching vault contents.
        }
        (AddVaultMode::ReInit, false) => {
            return Err(AppError::Invalid {
                field: "mode".into(),
                message: "re_init requires an existing .codebus/ at the target path".into(),
            });
        }
        (AddVaultMode::ReInit, true) => {
            fs::remove_dir_all(canonical.join(".codebus"))?;
            run_init(&canonical, &init_opts(), |_| {}).map_err(map_init_error)?;
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
    }
}

#[tauri::command]
pub async fn add_vault(path: String, options: AddVaultOptions) -> IpcResult<VaultEntry> {
    let state_path = app_state_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable".into(),
    })?;
    let vault_path = PathBuf::from(&path);
    add_vault_at(&state_path, &vault_path, &options)
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

    #[test]
    fn add_vault_detect_on_fresh_dir_runs_init_and_appends() {
        let home = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        let state_path = home.path().join(".codebus").join("app-state.json");
        let opts = AddVaultOptions {
            mode: AddVaultMode::Detect,
        };

        let entry = with_codebus_home(home.path(), || {
            add_vault_at(&state_path, repo.path(), &opts).expect("fresh init should succeed")
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
            add_vault_at(&state_path, repo.path(), &opts).expect_err("must require mode")
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
            add_vault_at(&state_path, repo.path(), &opts).expect("just_bind should succeed")
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
            add_vault_at(&state_path, &missing, &opts).expect_err("must reject missing path")
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
            add_vault_at(&state_path, repo.path(), &opts).expect("first add ok");
            let err =
                add_vault_at(&state_path, repo.path(), &opts).expect_err("second add must fail");
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
            add_vault_at(&state_path, repo.path(), &opts).expect("bind ok");

            remove_vault_at(&state_path, repo.path()).expect("remove ok");

            let state = load_app_state(&state_path);
            assert!(state.vault_list.is_empty(), "state must be unbound");
            assert!(
                codebus_dir.is_dir(),
                ".codebus/ MUST survive remove_vault per spec 'Remove unbinds without deletion'"
            );
        });
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
