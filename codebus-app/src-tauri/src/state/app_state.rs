//! Thin wrapper over the file-persistent app-state registry.
//!
//! The `AppState` / `StoredVaultEntry` schema plus the `load_app_state` /
//! `save_app_state` / `app_state_path` helpers were moved to
//! `codebus_core::app_state` so the CLI `mcp` subcommand can read the same
//! `~/.codebus/app-state.json` registry read-only (see the `mcp-server`
//! capability). The app remains the sole *writer*; this module re-exports the
//! moved symbols (so existing `crate::state::app_state::…` imports keep
//! resolving unchanged) and additionally owns the non-persistent
//! `AppRuntimeState` Tauri runtime state, which has no place in core.

use std::sync::Arc;

use super::active_runs::ActiveRuns;

pub use codebus_core::app_state::{
    AppState, CURRENT_SCHEMA_VERSION, StoredVaultEntry, app_state_path, load_app_state,
    save_app_state,
};

/// Tauri-managed runtime state. Lives only in process memory — never
/// serialized to disk. Owns mutable runtime concerns (active goal runs)
/// distinct from the file-persistent [`AppState`] above. Tauri commands
/// receive this as `tauri::State<AppRuntimeState>`.
///
/// `active_runs` is `Arc<ActiveRuns>` so the background goal thread can own a
/// clone for its cleanup-on-completion path without borrowing from the
/// Tauri-managed `State<'_, AppRuntimeState>` (which is short-lived per
/// command invocation).
#[derive(Debug, Default)]
pub struct AppRuntimeState {
    pub active_runs: Arc<ActiveRuns>,
}

impl AppRuntimeState {
    pub fn new() -> Self {
        Self::default()
    }
}
