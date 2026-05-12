//! Shared run-log helpers used by `goal` / `query` / `fix` verb commands.
//!
//! Encapsulates the repeated logic of: load `log` config → resolve directory
//! (default `<vault>/.codebus/log/`, override via `dir:`, tilde expansion) →
//! build the configured sink → write a [`RunLog`] entry → warn-without-fail
//! on persistence error.

use codebus_core::config::{LogConfig, default_config_path, load_log_config};
use codebus_core::log::{RunLog, SinkConfig, build_sink};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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

/// Resolve a `LogConfig.sink` (which may carry `dir: None`) to a concrete
/// `SinkConfig` whose `Jsonl` variant has a `Some(dir)`. Default `None`
/// resolves to `<vault>/.codebus/log/`. Tilde-prefixed paths expand to the
/// home directory. Other paths pass through verbatim.
pub fn resolve_sink_dir(cfg: LogConfig, vault_log_dir: &Path) -> SinkConfig {
    match cfg.sink {
        SinkConfig::Null {} => SinkConfig::Null {},
        SinkConfig::Jsonl { dir } => {
            let resolved = match dir {
                None => vault_log_dir.to_path_buf(),
                Some(p) => expand_tilde(p),
            };
            SinkConfig::Jsonl {
                dir: Some(resolved),
            }
        }
    }
}

/// Expand a leading `~` in a path to the user's home directory.
/// Pass-through when no leading `~` or when home cannot be resolved.
fn expand_tilde(path: PathBuf) -> PathBuf {
    let s = match path.to_str() {
        Some(s) => s,
        None => return path,
    };
    let stripped = match s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        Some(t) => t,
        None => return path,
    };
    match dirs::home_dir() {
        Some(home) => home.join(stripped),
        None => path,
    }
}

/// Persist a [`RunLog`] entry through the configured sink. On failure, emit
/// a stderr warning prefixed with `warning: run-log` and return without
/// propagating — the caller's exit code SHALL NOT change because of a log
/// write failure (per `RunLog Write Failure Is Non-Fatal` requirement).
pub fn write_run_log(sink_cfg: SinkConfig, entry: &RunLog) {
    let mut sink = match build_sink(sink_cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: run-log sink build failed (skipping persistence): {e}");
            return;
        }
    };
    if let Err(e) = sink.write_run(entry) {
        eprintln!("warning: run-log write failed (non-fatal): {e}");
    }
}

/// Compute `wiki_changed` for a `RunLog` entry by checking whether the
/// vault's `wiki/` subtree differs from the previous commit. Uses
/// `git -C <vault> diff --quiet HEAD~1 -- wiki/` exit code: 0 = no diff
/// (false), 1 = diff (true). Any other failure (no HEAD~1, git missing) →
/// false (best-effort).
pub fn wiki_changed_since_last_commit(vault_root: &Path) -> bool {
    // Pipe stderr to /dev/null equivalent so the probe stays silent on
    // first-run vaults (HEAD~1 doesn't resolve → git emits
    // "fatal: bad revision 'HEAD~1'" which would leak to the user's
    // terminal). Best-effort: any failure → false.
    let status = std::process::Command::new("git")
        .args(["-C"])
        .arg(vault_root)
        .args(["diff", "--quiet", "HEAD~1", "--", "wiki/"])
        .stderr(std::process::Stdio::null())
        .status();
    matches!(status, Ok(s) if s.code() == Some(1))
}

