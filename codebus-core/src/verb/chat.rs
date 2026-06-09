//! `verb::chat::run_chat_turn` — per-turn read-only multi-turn chat
//! orchestration as a library function. Each call spawns one `claude -p`
//! child process via `agent::invoke`; the caller (CLI REPL or GUI overlay)
//! drives the multi-turn loop by feeding `ChatTurnReport.session_id` back
//! into the next call as `ChatTurnOptions.session_id`.
//!
//! See spec capabilities `chat-verb` + `verb-library` (Agent Invoke Resume
//! Session Support) and design `docs/internal/2026-05-13-chat-verb-discussion.md`.
//!
//! Behavior order (one turn):
//!  1. Vault precondition — `VerbError::VaultMissing` when `<repo>/.codebus/`
//!     missing (chat does not auto-init; chat is wiki user, not producer).
//!  2. Load `claude_code` + `log` config (propagate `VerbError::ConfigParse`).
//!  3. Resolve verb settings via `cc_cfg.resolve(Verb::Chat)` (reuses query
//!     model / effort by design — chat is read-only exploration, no
//!     dedicated config section at v1).
//!  4. Build env overrides (`VerbError::KeyringMissing` on azure key fetch
//!     failure).
//!  5. Emit `VerbLifecycleEvent::SpawnStart { verb: Verb::Chat }`, call
//!     `agent::invoke` with `CHAT_TOOLSET` + `resume_session_id`. The
//!     `on_event` wrapper:
//!       - Forwards every `StreamEvent` as `VerbEvent::Stream(...)`.
//!       - Detects the promote-suggestion line marker
//!         `[CODEBUS_PROMOTE_SUGGESTION] <reason>` at the start of any
//!         `StreamEvent::Thought { text }` and emits
//!         `VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion
//!         { reason })` exactly once per marker.
//!  6. Capture `InvokeReport.session_id` (set by `agent::invoke` from the
//!     first `init` stream-json line). `None` here means the spawn never
//!     reached the init phase — surface as `VerbError::Internal`.
//!  7. Emit `VerbLifecycleEvent::SpawnEnd { verb: Verb::Chat, exit_code }`.
//!  8. On observed cancel: write `RunLog { mode: "chat", session_id, outcome:
//!     "cancelled" }` and return `Err(VerbError::Cancelled)`.
//!  9. On success: write `RunLog { mode: "chat", session_id, outcome:
//!     "succeeded" }` and return `Ok(ChatTurnReport)`.
//!
//! No auto_commit at any point (chat is read-only; `CHAT_TOOLSET` excludes
//! `Write`/`Edit` at the binary layer + SKILL.md restates the invariant).

use crate::agent::{Permission, SpawnSpec, build_backend, invoke, load_provider_config};
use crate::config::{Verb, default_config_path};
use crate::log::events::{EventEnvelope, EventsNullSink, EventsSink};
use crate::log::factory::build_events_sink;
use crate::log::verb_log::{load_verb_log_config, resolve_sink_dir, write_run_log};
use crate::log::{InterruptReason, RunLog, SinkConfig, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbEvent, VerbLifecycleEvent};
use chrono::SecondsFormat;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Read-only toolset for the chat verb. Excludes Write / Edit / Bash /
/// NotebookEdit. The `mcp_*` tools are NOT gated here — they are handled
/// at the SKILL.md prompt layer per the `MCP Tool Prompt Layer Exclusion`
/// requirement of `chat-verb`.
pub const CHAT_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

/// Literal line-marker prefix the chat agent emits at the start of an
/// assistant message to suggest the conversation content is worth
/// promoting to a wiki page. Parsed by `run_chat_turn`'s stream filter
/// per the `Promote Suggestion Line Marker` requirement of `chat-verb`.
pub(crate) const PROMOTE_SUGGESTION_MARKER: &str = "[CODEBUS_PROMOTE_SUGGESTION] ";

/// Caller-controllable inputs to `run_chat_turn`. `session_id` is `None`
/// on the first turn of a REPL session; on subsequent turns the caller
/// MUST pass `Some(prev_report.session_id)` so the spawn drives
/// `--resume <id>` and the conversation history persists.
#[derive(Debug, Clone)]
pub struct ChatTurnOptions {
    pub text: String,
    pub session_id: Option<String>,
}

/// Outcome of a successful `run_chat_turn` invocation. `session_id` is
/// non-Option here (vs. `Option<String>` on `ChatTurnOptions`) because
/// the first `init` stream event always carries the field — see the
/// spike `❷` result in `docs/internal/2026-05-13-chat-verb-discussion.md`.
/// `agent_exit_code` mirrors the underlying `claude` child exit; the
/// CLI REPL uses it to decide whether the turn errored.
#[derive(Debug, Clone)]
pub struct ChatTurnReport {
    pub session_id: String,
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
}

/// Extract the `reason` payload from a `Thought` event whose `text`
/// starts with the promote-suggestion line marker. Returns `Some(reason)`
/// when the very first character of the text is the marker prefix; the
/// reason is the substring after the prefix up to (but not including)
/// the first newline. Returns `None` for any other shape — the marker
/// MUST appear at the start of the message per spec.
pub(crate) fn extract_promote_suggestion(text: &str) -> Option<String> {
    let suffix = text.strip_prefix(PROMOTE_SUGGESTION_MARKER)?;
    // Reason = first line after the prefix. Trim trailing whitespace only
    // (preserve leading whitespace inside the reason itself is moot — agent
    // emits reason as plain prose).
    let reason = match suffix.find('\n') {
        Some(idx) => &suffix[..idx],
        None => suffix,
    };
    let reason = reason.trim_end();
    if reason.is_empty() {
        return None;
    }
    Some(reason.to_string())
}

/// Run one chat turn against `repo`. See module docs for orchestration order.
pub fn run_chat_turn(
    repo: &Path,
    options: ChatTurnOptions,
    mut on_event: impl FnMut(VerbEvent),
    cancel: Option<Arc<AtomicBool>>,
    timeout: Option<std::time::Duration>,
) -> Result<ChatTurnReport, VerbError> {
    let paths = vault_paths(repo);

    // Millis precision: matches the IPC `active_runs` key precision so
    // the orphan-detection invariant in `list_runs_impl` (events-file
    // slug ↔ active_runs key) holds. See app-workspace § Interrupted
    // Run Detection NOTE.
    let run_started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    // Step 1: strict vault precondition. Chat is a wiki reader, not a
    // producer — missing vault is a user input error.
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    // Step 2: load claude_code + log config.
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => {
            load_provider_config(&p).map_err(|e| VerbError::ConfigParse {
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

    // Events sink failure → fallback to null + stderr warn (matches
    // existing verb behavior; per events-log Write Failure Is Non-Fatal).
    let mut events_sink: Box<dyn EventsSink> = match build_events_sink(&sink_cfg, &run_started_at) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: events-log sink build failed (skipping persistence): {e}");
            Box::new(EventsNullSink::new())
        }
    };

    // Step 3: resolve verb config (chat reuses query model/effort by design).
    let chat_resolved = cc_cfg.resolve(Verb::Chat);

    // Step 4: build env overrides (azure profile keyring fetch), then build
    // the Claude backend (holds config for model resolution + env).
    let backend =
        build_backend(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;

    // Fan-out closure: each VerbEvent → events sink + caller's on_event.
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

    // Step 5: spawn agent.
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Chat,
    }));
    let invoke_report = {
        let fan_out = &mut fan_out;
        let stream_options = options.session_id.clone();
        invoke(
            &*backend,
            SpawnSpec {
                verb: Verb::Chat,
                resolve_as: None,
                sub_mode: None,
                input: options.text.clone(),
                permission: Permission::ReadOnly,
                command_allowance: None,
                resume_session_id: stream_options,
            },
            &paths.root,
            |event: StreamEvent| {
                // Detect promote-suggestion marker on Thought text before
                // forwarding the event — emission order matches spec
                // requirement (CLI sees PromoteSuggestion inline during
                // the stream, not after turn end).
                if let StreamEvent::Thought { text } = &event
                    && let Some(reason) = extract_promote_suggestion(text)
                {
                    fan_out(VerbEvent::Lifecycle(
                        VerbLifecycleEvent::PromoteSuggestion { reason },
                    ));
                }
                fan_out(VerbEvent::Stream(event));
            },
            cancel.clone(),
            timeout,
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Chat,
        exit_code: invoke_report.exit.code(),
    }));

    // Capture session_id from the spawn. Spec invariant: every chat
    // spawn that reaches the init phase carries a session_id; failure
    // to see it indicates the spawn died before init → Internal error.
    let session_id_from_spawn = invoke_report.session_id.clone().ok_or_else(|| {
        VerbError::Internal {
            message: "chat spawn produced no init event (session_id unavailable)".to_string(),
        }
    })?;

    // Cancellation path: write RunLog with outcome=cancelled BEFORE
    // returning the Err so the GUI / CLI can still observe the turn.
    if let Some(flag) = &cancel
        && flag.load(Ordering::Relaxed)
    {
        let cancel_run_log = RunLog {
            goal: options.text.clone(),
            mode: "chat".into(),
            model: chat_resolved.model.clone(),
            effort: chat_resolved.effort.clone(),
            started_at: run_started_at.clone(),
            finished_at: invoke_report.finished_at.clone(),
            tokens: invoke_report.accumulated_tokens.clone(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "cancelled".into(),
            session_id: Some(session_id_from_spawn.clone()),
            sandbox_denial_count: 0,
            interrupt_reason: Some(InterruptReason::UserCancel),
        };
        write_run_log(sink_cfg.clone(), &cancel_run_log);
        return Err(VerbError::Cancelled);
    }

    // Outcome reflects the agent child's exit code. A non-zero exit (e.g.
    // codex `exec resume` rejecting a cross-provider switch) is a FAILED turn
    // — previously this was hardcoded "succeeded", so a broken resume was
    // logged as success and the GUI showed an empty response with no error.
    let exit_code = invoke_report.exit.code();
    let succeeded = exit_code == Some(0);
    // run-outcome-lifecycle-integrity: a wall-clock timeout forces
    // outcome=failed + Timeout (cancel was handled by the early return).
    let (outcome, interrupt_reason) = if invoke_report.timed_out {
        ("failed", Some(InterruptReason::Timeout))
    } else if succeeded {
        ("succeeded", None)
    } else {
        ("failed", None)
    };
    crate::verb::warn_sandbox_denials(invoke_report.sandbox_denial_count);
    let run_log = RunLog {
        goal: options.text.clone(),
        mode: "chat".into(),
        model: chat_resolved.model.clone(),
        effort: chat_resolved.effort.clone(),
        started_at: run_started_at,
        finished_at: invoke_report.finished_at.clone(),
        tokens: invoke_report.accumulated_tokens.clone(),
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: outcome.into(),
        session_id: Some(session_id_from_spawn.clone()),
        sandbox_denial_count: invoke_report.sandbox_denial_count,
        interrupt_reason,
    };
    write_run_log(sink_cfg, &run_log);

    if !succeeded {
        return Err(VerbError::AgentFailed { exit_code });
    }

    Ok(ChatTurnReport {
        session_id: session_id_from_spawn,
        accumulated_tokens: invoke_report.accumulated_tokens,
        started_at: run_log.started_at,
        finished_at: invoke_report.finished_at,
        agent_exit_code: exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// `Chat Verb Toolset` requirement: only Read/Glob/Grep, no write tools.
    #[test]
    fn chat_toolset_is_read_only() {
        assert_eq!(CHAT_TOOLSET, &["Read", "Glob", "Grep"]);
        assert!(!CHAT_TOOLSET.contains(&"Write"));
        assert!(!CHAT_TOOLSET.contains(&"Edit"));
        assert!(!CHAT_TOOLSET.contains(&"Bash"));
        assert!(!CHAT_TOOLSET.contains(&"NotebookEdit"));
    }

    /// `Chat Verb Library Function` requirement: pin the public API shape
    /// (ChatTurnOptions / ChatTurnReport field set). `session_id` on the
    /// report is non-Option per spec.
    #[test]
    fn chat_turn_options_and_report_fields_present() {
        let options = ChatTurnOptions {
            text: "hello".into(),
            session_id: Some("abc-123".into()),
        };
        assert_eq!(options.text, "hello");
        assert_eq!(options.session_id.as_deref(), Some("abc-123"));

        let report = ChatTurnReport {
            session_id: "abc-123".into(),
            accumulated_tokens: TokenUsage::default(),
            started_at: "2026-05-13T00:00:00Z".into(),
            finished_at: "2026-05-13T00:00:01Z".into(),
            agent_exit_code: Some(0),
        };
        // Compile-time assertion that session_id is `String`, not Option.
        let _: String = report.session_id;
        assert_eq!(report.agent_exit_code, Some(0));
    }

    /// `Promote Suggestion Line Marker` scenario:
    /// "Marker at message start triggers PromoteSuggestion event"
    #[test]
    fn parse_promote_suggestion_marker_at_start() {
        let text =
            "[CODEBUS_PROMOTE_SUGGESTION] auth lifecycle including JWT issuance\n\nThe flow is...";
        assert_eq!(
            extract_promote_suggestion(text).as_deref(),
            Some("auth lifecycle including JWT issuance")
        );
    }

    /// `Promote Suggestion Line Marker` scenario:
    /// "Marker not at message start is not detected"
    #[test]
    fn parse_promote_suggestion_marker_not_at_start() {
        let text = "Sure, here is the answer.\n[CODEBUS_PROMOTE_SUGGESTION] x";
        assert_eq!(extract_promote_suggestion(text), None);
    }

    /// `Promote Suggestion Line Marker` scenario:
    /// "Marker reason supports non-ASCII characters"
    #[test]
    fn parse_promote_suggestion_supports_non_ascii() {
        let text = "[CODEBUS_PROMOTE_SUGGESTION] uv-lib 與 uv-child 的關係與子進程處理\n\n...";
        assert_eq!(
            extract_promote_suggestion(text).as_deref(),
            Some("uv-lib 與 uv-child 的關係與子進程處理")
        );
    }

    /// Edge case: empty reason after prefix should not emit (forbids
    /// degenerate `[CODEBUS_PROMOTE_SUGGESTION] \n...` shape).
    #[test]
    fn parse_promote_suggestion_rejects_empty_reason() {
        assert_eq!(extract_promote_suggestion("[CODEBUS_PROMOTE_SUGGESTION] "), None);
        assert_eq!(extract_promote_suggestion("[CODEBUS_PROMOTE_SUGGESTION] \n"), None);
    }

    /// Vault precondition: chat is read-only, no auto-init. Missing
    /// vault → `VerbError::VaultMissing` per `Chat Subcommand Behavior`.
    #[test]
    fn run_chat_turn_returns_vault_missing_when_no_codebus_dir() {
        let tmp = TempDir::new().unwrap();
        let events: std::cell::RefCell<Vec<VerbEvent>> = std::cell::RefCell::new(Vec::new());
        let result = run_chat_turn(
            tmp.path(),
            ChatTurnOptions {
                text: "hi".into(),
                session_id: None,
            },
            |event| events.borrow_mut().push(event),
            None,
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
        // No spawn lifecycle should fire when vault missing.
        let collected = events.borrow();
        assert!(
            !collected.iter().any(|e| matches!(
                e,
                VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { .. })
            )),
            "agent must not spawn when vault is missing"
        );
    }
}
