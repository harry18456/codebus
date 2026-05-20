//! codebus-app-tauri — Tauri v2 backend for the codebus desktop app.
//!
//! Lobby + Settings + Workspace stub. IPC commands are registered in the
//! `ipc` module; persistent state (vault list) lives in `state::app_state`.
//! See `openspec/specs/app-shell/spec.md` for the full contract.

pub mod config;
pub mod error;
pub mod ipc;
pub mod state;
pub mod watcher;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .manage(state::app_state::AppRuntimeState::new())
        .manage(watcher::WatcherRegistry::new())
        .setup(|app| {
            use tauri::Manager;
            let handle = app.handle().clone();
            let registry = handle.state::<watcher::WatcherRegistry>();
            if let Err(e) = watcher::setup_lobby_watcher(&handle, &registry) {
                eprintln!("lobby watcher setup failed (auto-refresh disabled): {e}");
            }
            Ok(())
        })
        .invoke_handler(generate_ipc_handler!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
