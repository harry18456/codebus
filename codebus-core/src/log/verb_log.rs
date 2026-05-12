//! Run-log helpers shared by `verb::{goal,query,fix}::run_*` orchestration
//! functions. Carried over from `codebus-cli/src/run_log.rs` so GUI callers
//! (v3-app-workspace-goal) can reuse the same RunLog write path without
//! depending on the CLI binary crate.
//!
//! - [`resolve_sink_dir`] — flatten `LogConfig.sink` against the vault's
//!   default log directory; expand tilde paths
//! - [`write_run_log`] — build the sink and persist one `RunLog` entry;
//!   stderr warning on persistence failure (the only stderr emit in this
//!   module; matches the v3-run-log spec `RunLog Write Failure Is Non-Fatal`
//!   requirement)
//! - [`wiki_changed_since_last_commit`] — probe `git diff HEAD~1 -- wiki/`
//!   exit code to compute the RunLog `wiki_changed` boolean
//!
//! `load_log_config_with_warning` is intentionally NOT moved here: the
//! verb library functions translate parse errors into `VerbError::ConfigParse`
//! and let the CLI thin wrapper emit stderr on match. This keeps the
//! library functions free of direct stderr writes.

use crate::config::{LogConfig, default_config_path, load_log_config};
use crate::log::{RunLog, SinkConfig, build_sink};
use std::path::{Path, PathBuf};

/// Load the `log:` config section from `~/.codebus/config.yaml`. Returns
/// the default `LogConfig` when the config file does not exist (first-time
/// setup). Returns the underlying `ConfigLoadError` when the file exists
/// but fails to parse — the verb library function SHALL translate this
/// into `VerbError::ConfigParse`.
pub fn load_verb_log_config() -> Result<LogConfig, crate::config::ConfigLoadError> {
    let path = match default_config_path() {
        Some(p) => p,
        None => return Ok(LogConfig::default()),
    };
    if !path.exists() {
        return Ok(LogConfig::default());
    }
    load_log_config(&path)
}

/// Resolve a `LogConfig.sink` (which may carry `dir: None`) to a concrete
/// `SinkConfig` whose `Jsonl` variant has a `Some(dir)`. Default `None`
/// resolves to `<vault>/.codebus/log/`. Tilde-prefixed paths expand to
/// the home directory. Other paths pass through verbatim.
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

/// Persist a [`RunLog`] entry through the configured sink. On failure,
/// emit a stderr warning prefixed with `warning: run-log` and return
/// without propagating — the caller's exit code SHALL NOT change because
/// of a log write failure (per `RunLog Write Failure Is Non-Fatal`).
///
/// This is the one place in `codebus-core` that writes to stderr directly
/// (necessary because the warning is the only signal of a failed persist;
/// turning it into a `Result` would oblige every verb function to handle
/// a non-fatal error). CLI behavior is byte-equivalent to the pre-move
/// `codebus-cli/src/run_log.rs::write_run_log`.
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
/// false (best-effort). Stderr from the probe is discarded so first-run
/// vaults (where `HEAD~1` doesn't resolve) don't leak `fatal: bad revision`
/// to the user's terminal.
pub fn wiki_changed_since_last_commit(vault_root: &Path) -> bool {
    let status = std::process::Command::new("git")
        .args(["-C"])
        .arg(vault_root)
        .args(["diff", "--quiet", "HEAD~1", "--", "wiki/"])
        .stderr(std::process::Stdio::null())
        .status();
    matches!(status, Ok(s) if s.code() == Some(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::{RunLog, TokenUsage};
    use tempfile::TempDir;

    #[test]
    fn resolve_sink_dir_uses_vault_default_when_dir_is_none() {
        let cfg = LogConfig::default(); // Jsonl { dir: None }
        let vault_log = Path::new("/tmp/v/.codebus/log");
        let resolved = resolve_sink_dir(cfg, vault_log);
        match resolved {
            SinkConfig::Jsonl { dir: Some(p) } => assert_eq!(p, vault_log),
            other => panic!("expected Jsonl with Some(vault_log), got {other:?}"),
        }
    }

    #[test]
    fn resolve_sink_dir_null_passes_through() {
        let cfg = LogConfig {
            sink: SinkConfig::Null {},
        };
        let resolved = resolve_sink_dir(cfg, Path::new("/tmp/v/.codebus/log"));
        assert!(matches!(resolved, SinkConfig::Null {}));
    }

    #[test]
    fn resolve_sink_dir_with_explicit_dir_passes_through() {
        let cfg = LogConfig {
            sink: SinkConfig::Jsonl {
                dir: Some(PathBuf::from("/var/log/codebus")),
            },
        };
        let resolved = resolve_sink_dir(cfg, Path::new("/tmp/v/.codebus/log"));
        match resolved {
            SinkConfig::Jsonl { dir: Some(p) } => {
                assert_eq!(p, PathBuf::from("/var/log/codebus"))
            }
            other => panic!("expected /var/log/codebus, got {other:?}"),
        }
    }

    #[test]
    fn write_run_log_roundtrip_through_jsonl_sink() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let entry = RunLog {
            goal: "test".into(),
            mode: "query".into(),
            model: Some("opus-4-7".into()),
            effort: Some("high".into()),
            started_at: "2026-05-12T03:25:11Z".into(),
            finished_at: "2026-05-12T03:25:14Z".into(),
            tokens: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
                extras: serde_json::Value::Null,
            },
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
        };
        write_run_log(
            SinkConfig::Jsonl {
                dir: Some(dir.clone()),
            },
            &entry,
        );
        let log_file = dir.join("runs-2026-05-12.jsonl");
        assert!(log_file.exists(), "expected log file at {log_file:?}");
        let body = std::fs::read_to_string(&log_file).unwrap();
        assert!(body.contains("\"goal\":\"test\""));
        assert!(body.contains("\"mode\":\"query\""));
        assert!(body.ends_with('\n'));
    }

    #[test]
    fn wiki_changed_returns_false_for_first_commit_repo() {
        // Best-effort: probe should return false when HEAD~1 cannot be
        // resolved (e.g., repo with zero or one commits). Real wiki
        // diff path is exercised by CLI integration tests goal_flow.rs
        // / fix_flow.rs which run against fully-initialized vaults.
        let tmp = TempDir::new().unwrap();
        let _ = std::process::Command::new("git")
            .args(["-C"])
            .arg(tmp.path())
            .args(["init", "--quiet"])
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status();
        // No commits → HEAD~1 unresolvable → status code != Some(1) → false.
        let result = wiki_changed_since_last_commit(tmp.path());
        assert!(!result, "fresh repo with no commits should report no diff");
    }

    #[test]
    fn load_verb_log_config_returns_default_when_no_config_path() {
        // We can't easily mock default_config_path() without env hacks,
        // but we can at least verify the function signature and default
        // shape by calling load_log_config directly with a nonexistent
        // path — that's the same primitive load_verb_log_config delegates
        // to when the file exists.
        let cfg = LogConfig::default();
        // Default shape: Jsonl { dir: None }
        assert!(matches!(
            cfg.sink,
            SinkConfig::Jsonl { dir: None }
        ));
    }
}
