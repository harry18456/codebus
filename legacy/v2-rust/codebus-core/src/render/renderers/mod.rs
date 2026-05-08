//! Concrete `EventRenderer` implementations. `terminal` is the day-one
//! impl; `json_lines` and `tauri` will land here when their consumers
//! arrive (json_lines from a `--render json` CLI flag; tauri from the
//! codebus-app webview shell).

pub mod terminal;
