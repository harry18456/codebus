//! codebus-app-tauri — Tauri v2 backend for the codebus desktop app.
//!
//! Lobby + Settings + Workspace stub. IPC commands are registered in the
//! `ipc` module; persistent state (vault list) lives in `state::app_state`.
//! See `openspec/specs/app-shell/spec.md` for the full contract.

pub mod config;
pub mod error;
pub mod ipc;
pub mod state;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(generate_ipc_handler!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
