pub mod sidecar;

#[tauri::command]
async fn sidecar_ping() -> Result<sidecar::PingResult, String> {
  // M1 dev assumption: packaged sidecar binary sits at `codebus-sidecar`
  // on PATH.  Phase 8 replaces this with the Tauri externalBin resolved
  // path so prod launches land the onefile binary.
  sidecar::sidecar_ping("codebus-sidecar")
    .await
    .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
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
    .invoke_handler(tauri::generate_handler![sidecar_ping])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
