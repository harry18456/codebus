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

use crate::agent::{ClaudeBackend, Permission, SpawnSpec, invoke};
use crate::config::{Verb, build_env_overrides, default_config_path, load_claude_code_config};
use crate::log::events::{EventEnvelope, EventsNullSink, EventsSink};
use crate::log::factory::build_events_sink;
use crate::log::verb_log::{load_verb_log_config, resolve_sink_dir, write_run_log};
use crate::log::{RunLog, SinkConfig, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbBanner, VerbEvent, VerbLifecycleEvent};
use chrono::SecondsFormat;
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
/// entry written internally records them as zero). `agent_exit_code`
/// is the spawned agent's process exit code (None when the platform
/// reported a signal termination); CLI thin wrapper propagates it as
/// its own exit code so existing exit-code-propagation tests stay green.
#[derive(Debug, Clone)]
pub struct QueryReport {
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
}

/// Run the query verb against `repo`. See module docs for orchestration order.
pub fn run_query(
    repo: &Path,
    options: QueryOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QueryReport, VerbError> {
    let paths = vault_paths(repo);

    // Capture run started_at early — used for both events.jsonl filename
    // slug AND the final RunLog.started_at value, so events file name
    // matches the RunLog row's started_at (GUI joins on this).
    let run_started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Step 1: strict vault precondition. Emitted before any banner so
    // that the early-return path on VaultMissing does not produce a
    // half-state events file (sink build needs vault dir to exist).
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    // Step 2: load claude_code + log config (log config needed for events sink).
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => {
            load_claude_code_config(&p).map_err(|e| VerbError::ConfigParse {
                which: "claude_code",
                source: e,
            })?
        }
        _ => Default::default(),
    };
    let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
        which: "log",
        source: e,
    })?;
    let sink_cfg: SinkConfig = resolve_sink_dir(log_cfg, &paths.log);

    // Build events sink. Failure → fallback to no-op sink + stderr warn
    // (events-log Write Failure Is Non-Fatal).
    let mut events_sink: Box<dyn EventsSink> = match build_events_sink(&sink_cfg, &run_started_at) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: events-log sink build failed (skipping persistence): {e}");
            Box::new(EventsNullSink::new())
        }
    };

    // Step 4: resolve verb config.
    let query_resolved = cc_cfg.resolve(Verb::Query);
    let slash_command = format!("/codebus-query \"{}\"", options.text);

    // Step 5: build env overrides (azure profile keyring fetch), then build
    // the Claude backend (holds config for model resolution + env).
    let query_env =
        build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;
    let backend = ClaudeBackend::new(cc_cfg, query_env);

    // Fan out each VerbEvent to (a) the events sink and (b) the caller's
    // on_event closure. Built here so the Start banner below also lands
    // in events.jsonl.
    let mut fan_out = |event: VerbEvent| {
        let envelope = EventEnvelope {
            ts: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            event: event.clone(),
        };
        if let Err(e) = events_sink.write_event(&envelope) {
            eprintln!("warning: events-log write failed (non-fatal): {e}");
        }
        on_event(event);
    };

    // Start banner (now emitted post-sink-build so events.jsonl captures it).
    fan_out(VerbEvent::Banner(VerbBanner::Start {
        repo_path: repo.to_path_buf(),
    }));

    // Step 6: spawn agent.
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Query,
    }));
    let invoke_report = {
        let fan_out = &mut fan_out;
        invoke(
            &backend,
            SpawnSpec {
                verb: Verb::Query,
                prompt: slash_command,
                permission: Permission::ReadOnly,
                command_allowance: None,
                // query verb is one-shot (no session resume); chat verb is
                // the only caller that sets Some(...) on this field.
                resume_session_id: None,
            },
            &paths.root,
            |event: StreamEvent| fan_out(VerbEvent::Stream(event)),
            cancel.clone(),
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Query,
        exit_code: invoke_report.exit.code(),
    }));

    // Cancellation: write RunLog with outcome cancelled BEFORE returning
    // Err — GUI Goals overview row still needs the entry to list it.
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        let cancel_run_log = RunLog {
            goal: options.text.clone(),
            mode: "query".into(),
            model: query_resolved.model.clone(),
            effort: query_resolved.effort.clone(),
            started_at: run_started_at.clone(),
            finished_at: invoke_report.finished_at.clone(),
            tokens: invoke_report.accumulated_tokens.clone(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "cancelled".into(),
            session_id: None,
        };
        write_run_log(sink_cfg.clone(), &cancel_run_log);
        return Err(VerbError::Cancelled);
    }

    // Step 7: write RunLog (success path).
    // outcome: "succeeded" — query is read-only; the verb itself
    // completed even when the agent exits non-zero (CLI propagates
    // the agent exit via QueryReport.agent_exit_code field).
    let run_log = RunLog {
        goal: options.text.clone(),
        mode: "query".into(),
        model: query_resolved.model.clone(),
        effort: query_resolved.effort.clone(),
        started_at: run_started_at,
        finished_at: invoke_report.finished_at.clone(),
        tokens: invoke_report.accumulated_tokens.clone(),
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: "succeeded".into(),
        session_id: None,
    };
    write_run_log(sink_cfg, &run_log);

    // Step 9: return report. started_at uses run_started_at to match
    // the RunLog row and events.jsonl filename slug.
    Ok(QueryReport {
        accumulated_tokens: invoke_report.accumulated_tokens,
        started_at: run_log.started_at,
        finished_at: invoke_report.finished_at,
        agent_exit_code: invoke_report.exit.code(),
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
                assert!(
                    path.ends_with(".codebus"),
                    "expected path to .codebus, got {path:?}"
                );
            }
            other => panic!("expected VaultMissing, got {other:?}"),
        }
        // After v3-run-log-events: vault precondition runs BEFORE any
        // banner emission so that an early VaultMissing return does
        // not produce a half-state events file. No Start banner on
        // this error path.
        let collected = events.borrow();
        assert!(
            !collected
                .iter()
                .any(|e| matches!(e, VerbEvent::Banner(VerbBanner::Start { .. }))),
            "Start banner must not fire when vault is missing (sink build needs vault)"
        );
        // SpawnStart SHALL NOT fire — agent never spawned.
        assert!(
            !collected.iter().any(|e| matches!(
                e,
                VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { .. })
            )),
            "agent must not spawn when vault is missing"
        );
    }
}
