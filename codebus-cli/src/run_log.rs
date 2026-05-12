//! Run-log glue for the CLI binary.
//!
//! After v3-goal-library, the pure helpers (`resolve_sink_dir`,
//! `write_run_log`, `wiki_changed_since_last_commit`) live in
//! `codebus_core::log::verb_log` so verb library functions (and GUI
//! callers) can reuse them. The CLI keeps `load_log_config_with_warning`
//! locally because it emits stderr — keeping the stderr write at the CLI
//! boundary preserves the library's pure-functional contract.

use codebus_core::config::{LogConfig, default_config_path, load_log_config};
use std::process::ExitCode;

// Re-export the helpers that used to live here so existing CLI call
// sites keep working without a sweeping import change.
pub use codebus_core::log::verb_log::{
    resolve_sink_dir, wiki_changed_since_last_commit, write_run_log,
};

/// Load the `log:` config section from `~/.codebus/config.yaml`. Returns
/// `Default::default()` when the config file does not exist (first-time
/// setup). Returns `Err(ExitCode)` when the file exists but fails to
/// parse — caller SHALL propagate the exit code without writing to the
/// run-log sink (spec: cli / `Config Parse Failure Aborts Invocation`).
pub fn load_log_config_with_warning() -> Result<LogConfig, ExitCode> {
    let path = match default_config_path() {
        Some(p) => p,
        None => return Ok(LogConfig::default()),
    };
    match load_log_config(&path) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            eprintln!(
                "error: log config parse failed at {}: {e}",
                path.display()
            );
            Err(ExitCode::from(2))
        }
    }
}
