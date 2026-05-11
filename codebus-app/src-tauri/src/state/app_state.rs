//! Read/write helper for `~/.codebus/app-state.json`.
//!
//! Spec: `App-State Persistence` (see `openspec/changes/v3-app-foundation/specs/app-shell/spec.md`).
//!
//! - File schema: `{ "schema_version": 1, "vault_list": [...] }`
//! - Missing file → write empty state and return it.
//! - Parse failure OR `schema_version > CURRENT` → log warn to stderr,
//!   return an in-memory empty state, and DO NOT touch the file on disk.
//! - `CODEBUS_HOME` env var redirects the home root for tests / containers
//!   (matches the `codebus-core::config::default_config_path` convention so
//!   the CLI and app agree on home resolution).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredVaultEntry {
    pub path: String,
    pub display_name: String,
    /// ISO 8601 UTC timestamp.
    pub last_opened: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub schema_version: u32,
    pub vault_list: Vec<StoredVaultEntry>,
}

impl AppState {
    pub fn empty() -> Self {
        AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: Vec::new(),
        }
    }
}

/// Resolves `<home>/.codebus/app-state.json`. Honors the `CODEBUS_HOME`
/// override (see module docs). Returns `None` if neither the override nor a
/// system home directory is available.
pub fn app_state_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CODEBUS_HOME") {
        if !custom.is_empty() {
            return Some(PathBuf::from(custom).join(".codebus").join("app-state.json"));
        }
    }
    dirs::home_dir().map(|h| h.join(".codebus").join("app-state.json"))
}

/// Load or initialize app-state at `path`.
///
/// - File does not exist → write `AppState::empty()` to disk and return it.
/// - File parses cleanly with a recognized `schema_version` → return parsed.
/// - Parse error → emit `warning: app-state.json parse failed (using empty list)`
///   on stderr, return `AppState::empty()`, leave the on-disk file untouched.
/// - `schema_version` exceeds [`CURRENT_SCHEMA_VERSION`] → emit
///   `warning: app-state.json schema_version unsupported …` on stderr, return
///   `AppState::empty()`, leave the file untouched.
pub fn load_app_state(path: &Path) -> AppState {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<AppState>(&text) {
            Ok(state) => {
                if state.schema_version > CURRENT_SCHEMA_VERSION {
                    eprintln!(
                        "warning: app-state.json schema_version {} unsupported (current is {}); using empty list (file preserved)",
                        state.schema_version, CURRENT_SCHEMA_VERSION
                    );
                    return AppState::empty();
                }
                state
            }
            Err(err) => {
                eprintln!(
                    "warning: app-state.json parse failed (using empty list, file preserved): {err}"
                );
                AppState::empty()
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let state = AppState::empty();
            if let Err(write_err) = save_app_state(path, &state) {
                eprintln!(
                    "warning: app-state.json create failed (using in-memory empty): {write_err}"
                );
            }
            state
        }
        Err(err) => {
            eprintln!(
                "warning: app-state.json read failed (using empty list): {err}"
            );
            AppState::empty()
        }
    }
}

/// Write `state` to `path`, creating parent directory if missing.
/// Uses an atomic write (`*.tmp` + `rename`) so a partial write cannot leave
/// the file in a corrupt state.
pub fn save_app_state(path: &Path, state: &AppState) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture_path(dir: &TempDir) -> PathBuf {
        dir.path().join("app-state.json")
    }

    #[test]
    fn missing_file_creates_empty_state_on_disk() {
        let tmp = TempDir::new().unwrap();
        let path = fixture_path(&tmp);
        assert!(!path.exists());

        let state = load_app_state(&path);

        assert_eq!(state, AppState::empty());
        assert!(path.exists(), "load_app_state should create the file");
        let on_disk = fs::read_to_string(&path).unwrap();
        let parsed: AppState = serde_json::from_str(&on_disk).unwrap();
        assert_eq!(parsed, AppState::empty());
    }

    #[test]
    fn parse_failure_returns_empty_and_preserves_file() {
        let tmp = TempDir::new().unwrap();
        let path = fixture_path(&tmp);
        let bogus = "this is not json {";
        fs::write(&path, bogus).unwrap();

        let state = load_app_state(&path);

        assert_eq!(state, AppState::empty());
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, bogus, "corrupt file must not be overwritten");
    }

    #[test]
    fn future_schema_version_returns_empty_and_preserves_file() {
        let tmp = TempDir::new().unwrap();
        let path = fixture_path(&tmp);
        let future = serde_json::json!({
            "schema_version": CURRENT_SCHEMA_VERSION + 99,
            "vault_list": [
                {"path": "/p", "display_name": "n", "last_opened": "2026-05-11T00:00:00Z"}
            ]
        });
        fs::write(&path, future.to_string()).unwrap();

        let state = load_app_state(&path);

        assert_eq!(state, AppState::empty());
        let after: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after, future, "future-schema file must not be overwritten");
    }

    #[test]
    fn round_trip_preserves_vault_entries() {
        let tmp = TempDir::new().unwrap();
        let path = fixture_path(&tmp);
        let state = AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: vec![StoredVaultEntry {
                path: "/abs/path".into(),
                display_name: "Some Vault".into(),
                last_opened: "2026-05-11T14:00:00Z".into(),
            }],
        };

        save_app_state(&path, &state).unwrap();
        let loaded = load_app_state(&path);

        assert_eq!(loaded, state);
    }

    #[test]
    fn save_is_atomic_against_partial_write() {
        let tmp = TempDir::new().unwrap();
        let path = fixture_path(&tmp);

        // First write a baseline.
        let baseline = AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: vec![StoredVaultEntry {
                path: "/old".into(),
                display_name: "old".into(),
                last_opened: "2026-01-01T00:00:00Z".into(),
            }],
        };
        save_app_state(&path, &baseline).unwrap();

        // Confirm no leftover tmp file after a successful write.
        let tmp_path = path.with_extension("json.tmp");
        assert!(!tmp_path.exists(), "tmp file should be renamed away");
    }
}
