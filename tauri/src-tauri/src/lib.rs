pub mod audit_files;
pub mod sidecar;
pub mod tutorial;

use std::path::PathBuf;
use std::sync::Mutex;

/// Resolve the packaged sidecar binary path.
///
/// Tauri's `externalBin` copies the PyInstaller onefile next to the main
/// executable (stripping the target-triple suffix). In release mode the
/// bundled `codebus.exe` finds `codebus-sidecar.exe` as a sibling; in
/// `cargo tauri dev` it lands beside the debug binary in `target/debug`.
/// If neither is present (e.g. `cargo run` without prior bundling) we
/// fall back to the bare name so PATH lookup can still succeed.
fn resolve_sidecar_path() -> PathBuf {
    let name = if cfg!(windows) {
        "codebus-sidecar.exe"
    } else {
        "codebus-sidecar"
    };
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from(name)
}

/// Tauri-managed cache for the sidecar handshake. The Mutex keeps the
/// first-spawn result so subsequent `sidecar_handshake` IPC calls skip
/// re-spawning. The bearer/port stay in process memory only — never
/// persisted to disk.
#[derive(Default)]
pub struct SidecarState {
    handshake: Mutex<Option<sidecar::Handshake>>,
}

#[tauri::command]
async fn sidecar_ping() -> Result<sidecar::PingResult, String> {
    let path = resolve_sidecar_path();
    let path_str = path
        .to_str()
        .ok_or_else(|| "sidecar path contains non-UTF-8 characters".to_string())?;
    sidecar::sidecar_ping(path_str)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sidecar_handshake(
    state: tauri::State<'_, SidecarState>,
) -> Result<sidecar::Handshake, String> {
    {
        let guard = state
            .handshake
            .lock()
            .map_err(|e| format!("handshake mutex poisoned: {e}"))?;
        if let Some(hs) = guard.as_ref() {
            return Ok(hs.clone());
        }
    }
    let path = resolve_sidecar_path();
    let path_str = path
        .to_str()
        .ok_or_else(|| "sidecar path contains non-UTF-8 characters".to_string())?
        .to_string();
    let hs = tauri::async_runtime::spawn_blocking(move || {
        sidecar::spawn_and_handshake(&path_str)
    })
    .await
    .map_err(|e| format!("handshake task join error: {e}"))?
    .map_err(|e| e.to_string())?;
    let mut guard = state
        .handshake
        .lock()
        .map_err(|e| format!("handshake mutex poisoned: {e}"))?;
    if let Some(existing) = guard.as_ref() {
        return Ok(existing.clone());
    }
    *guard = Some(hs.clone());
    Ok(hs)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(SidecarState::default())
    .plugin(tauri_plugin_fs::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      sidecar_ping,
      sidecar_handshake,
      tutorial::read_tutorial_file,
      tutorial::write_progress_file,
      tutorial::list_tutorial_tasks,
      audit_files::read_audit_jsonl
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
