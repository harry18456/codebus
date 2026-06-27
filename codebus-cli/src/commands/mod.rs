pub mod chat;
pub mod config;
pub mod fix;
pub mod goal;
pub mod hook;
pub mod init;
pub mod lint;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod query;
pub mod quiz;

use std::time::Duration;

/// run-outcome-lifecycle-integrity: resolve the per-run wall-clock timeout
/// from `lifecycle.run_timeout_secs` for injection into a `run_*` verb. A
/// missing file / section / field, a `0` value, OR a load error all resolve
/// to `None` (no limit) — a malformed config conservatively defaults to "no
/// limit" after a stderr warning rather than silently fabricating a limit.
pub(crate) fn resolve_run_timeout(debug: bool) -> Option<Duration> {
    use codebus_core::config::{default_config_path, load_lifecycle_config};
    let path = default_config_path()?;
    match load_lifecycle_config(&path) {
        Ok(cfg) => cfg.run_timeout_secs.map(Duration::from_secs),
        Err(e) => {
            eprintln!("warning: lifecycle config load failed, run timeout disabled: {e}");
            if debug {
                eprintln!("[debug] lifecycle: load_lifecycle_config error: {e}");
            }
            None
        }
    }
}
