//! `verb::query::run_query` — read-only query orchestration as a library
//! function. Mirrors the behavior previously inlined in
//! `codebus-cli/src/commands/query.rs`. CLI thin wrapper after
//! v3-goal-library is built around this function.
//!
//! Behavior order:
//! 1. Emit `VerbBanner::Start { repo_path }` via `on_event`
//! 2. Strict vault precondition — return `VerbError::VaultMissing` if
//!    `<repo>/.codebus/` is missing (query is a wiki user, not an ingest
//!    producer; missing vault is a user input error, not a trigger to
//!    mutate state)
//! 3. Load `claude_code` config — propagate as `VerbError::ConfigParse`
//!    on parse failure (the `which` field carries `"claude_code"`)
//! 4. Resolve verb config (model / effort)
//! 5. Build env overrides — propagate as `VerbError::KeyringMissing`
//!    when the Azure profile is active and the key is unreachable
//! 6. Emit `VerbLifecycleEvent::SpawnStart`, call `agent::invoke` with a
//!    wrapping closure that re-emits each `StreamEvent` as
//!    `VerbEvent::Stream`, accumulate tokens; emit
//!    `VerbLifecycleEvent::SpawnEnd` after wait
//! 7. Load `log` config — propagate as `VerbError::ConfigParse` (`which`
//!    is `"log"`)
//! 8. Resolve sink dir and write `RunLog` (mode `"query"`,
//!    `wiki_changed: false`, lint counts both `0`)
//! 9. Return `QueryReport { accumulated_tokens, started_at, finished_at }`
//!
//! No auto_commit at any point (query is read-only).

use crate::agent::{InvokeAgentOptions, invoke};
use crate::config::{Verb, build_env_overrides, default_config_path, load_claude_code_config};
use crate::log::verb_log::{load_verb_log_config, resolve_sink_dir, write_run_log};
use crate::log::{RunLog, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbBanner, VerbEvent, VerbLifecycleEvent};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Read-only toolset for the query verb. Excludes Write / Edit / Bash.
/// v2 iter-9 carry, spike-verified 2026-05-09 to be a real hard gate.
pub const QUERY_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

/// Caller-controllable inputs to `run_query`.
#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub text: String,
}

/// Successful query run summary. Query is read-only so `wiki_changed`
/// and lint counts are absent (always zero conceptually — the RunLog
/// entry written internally records them as zero).
#[derive(Debug, Clone)]
pub struct QueryReport {
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
}

/// Run the query verb against `repo`. See module docs for orchestration order.
pub fn run_query(
    repo: &Path,
    options: QueryOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QueryReport, VerbError> {
    let paths = vault_paths(repo);

    // Step 1: Start banner.
    on_event(VerbEvent::Banner(VerbBanner::Start {
        repo_path: repo.to_path_buf(),
    }));

    // Step 2: strict vault precondition.
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    // Step 3: load claude_code config.
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => load_claude_code_config(&p).map_err(|e| VerbError::ConfigParse {
            which: "claude_code",
            source: e,
        })?,
        _ => Default::default(),
    };

    // Step 4: resolve verb config.
    let query_resolved = cc_cfg.resolve(Verb::Query);
    let slash_command = format!("/codebus-query \"{}\"", options.text);

    // Step 5: build env overrides (azure profile keyring fetch).
    let query_env =
        build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;

    // Step 6: spawn agent. Wrap stream events in VerbEvent::Stream and
    // forward to the caller closure.
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Query,
    }));
    // The closure borrows `on_event` via `&mut`; but `on_event` is the
    // outer closure parameter. We need to share it between this and the
    // post-invoke SpawnEnd emit. Use a workaround: collect events via
    // a Vec and forward after... no, that breaks streaming. Instead
    // restructure: take `&mut on_event` to a local closure.
    //
    // Rust trick: pass `|e| on_event(VerbEvent::Stream(e))` as a closure
    // capturing `&mut on_event`. Since `on_event` itself is `impl FnMut`,
    // we can re-borrow it.
    let invoke_report = {
        let on_event = &mut on_event;
        invoke(
            InvokeAgentOptions {
                slash_command,
                vault_root: paths.root.clone(),
                toolset: QUERY_TOOLSET,
                bash_whitelist: None,
                model: query_resolved.model.clone(),
                effort: query_resolved.effort.clone(),
                env: query_env,
            },
            |event: StreamEvent| on_event(VerbEvent::Stream(event)),
            cancel.clone(),
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    on_event(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Query,
        exit_code: invoke_report.exit.code(),
    }));

    // If cancel was observed, return Cancelled (no run_log write — we
    // mirror auto-commit-skip semantics: cancelled runs leave minimal
    // trace; the events.jsonl persisted by v3-run-log-events will carry
    // the partial state).
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        return Err(VerbError::Cancelled);
    }

    // Step 7: load log config.
    let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
        which: "log",
        source: e,
    })?;

    // Step 8: resolve sink + write RunLog.
    let sink_cfg = resolve_sink_dir(log_cfg, &paths.log);
    let run_log = RunLog {
        goal: options.text.clone(),
        mode: "query".into(),
        model: query_resolved.model.clone(),
        effort: query_resolved.effort.clone(),
        started_at: invoke_report.started_at.clone(),
        finished_at: invoke_report.finished_at.clone(),
        tokens: invoke_report.accumulated_tokens.clone(),
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
    };
    write_run_log(sink_cfg, &run_log);

    // Step 9: return report.
    Ok(QueryReport {
        accumulated_tokens: invoke_report.accumulated_tokens,
        started_at: invoke_report.started_at,
        finished_at: invoke_report.finished_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_query_returns_vault_missing_when_no_codebus_dir() {
        let tmp = TempDir::new().unwrap();
        // tmp.path() has no .codebus/ subdirectory.
        let events: std::cell::RefCell<Vec<VerbEvent>> = std::cell::RefCell::new(Vec::new());
        let result = run_query(
            tmp.path(),
            QueryOptions {
                text: "test query".into(),
            },
            |event| events.borrow_mut().push(event),
            None,
        );
        match result {
            Err(VerbError::VaultMissing { path }) => {
                assert!(path.ends_with(".codebus"), "expected path to .codebus, got {path:?}");
            }
            other => panic!("expected VaultMissing, got {other:?}"),
        }
        // The Start banner SHOULD fire before precondition check.
        let collected = events.borrow();
        assert!(
            collected.iter().any(|e| matches!(e, VerbEvent::Banner(VerbBanner::Start { .. }))),
            "expected Start banner before precondition check"
        );
        // SpawnStart SHALL NOT fire — agent never spawned.
        assert!(
            !collected
                .iter()
                .any(|e| matches!(e, VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { .. }))),
            "agent must not spawn when vault is missing"
        );
    }
}
