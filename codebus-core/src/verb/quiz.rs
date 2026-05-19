//! `verb::quiz` — read-only quiz orchestration, split into two public
//! functions so the GUI confirm-gate (design D1) can interpose between
//! planning and generation.
//!
//! - [`run_quiz_plan`] runs the **plan** spawn: takes a free-text topic,
//!   emits a `[CODEBUS_QUIZ_SCOPE]` / `[CODEBUS_QUIZ_NO_MATCH]` marker,
//!   and returns the parsed [`QuizPlanOutcome`]. It does NOT continue to
//!   generation — the caller decides (CLI: proceed immediately; GUI:
//!   wait for the user to confirm/revise the scope).
//! - [`run_quiz_generate`] runs the **generate** spawn against a fixed
//!   page list + question count and returns the [`QuizReport`].
//!
//! Goal flow = `run_quiz_plan` then (on `Scope`) `run_quiz_generate`.
//! Page flow (wiki-preview `[Quiz me on this]`) = `run_quiz_generate`
//! directly with `pages = [target]` (the SKILL expands one-hop).
//!
//! This split is why the `quiz` sub-module is the documented exception
//! to verb-library's "exactly one orchestration function per verb" rule
//! (see the `verb-library` spec delta): the confirm gate is a hard
//! requirement of `app-workspace` Quiz Tab Plan-Confirm-Generate Flow
//! and design D1, and a single connected call cannot pause mid-flight
//! for an asynchronous GUI confirmation.
//!
//! The library never reads config — `question_count` is caller-injected
//! (`Shared Quiz Config Namespace`). Grading is client-side; the agent
//! never receives user answers. raw-scope enforcement is the SKILL
//! prompt invariant (design D3). Only the generate spawn persists a
//! RunLog + events.jsonl (the plan spawn is a planning step; its stream
//! is surfaced live via `on_event` but not persisted). The generate
//! spawn's events.jsonl path is returned as `QuizReport.events_log`
//! (design D4 frontmatter pointer).
//!
//! Test layering (design D8): this module unit-tests the pure parsers
//! (`parse_plan_outcome`, `strip_code_fence`) and the vault
//! preconditions. End-to-end mock_claude spawn tests live in
//! `codebus-cli/tests/quiz_flow.rs` (task 4.2).

use crate::agent::claude_cli::InvokeReport;
use crate::agent::{InvokeAgentOptions, invoke};
use crate::config::{Verb, build_env_overrides, default_config_path, load_claude_code_config};
use crate::log::events::{EventEnvelope, EventsNullSink, EventsSink};
use crate::log::factory::build_events_sink;
use crate::log::sink::accumulate_token_usage;
use crate::log::verb_log::{load_verb_log_config, resolve_sink_dir, write_run_log};
use crate::log::{RunLog, SinkConfig, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::error::VerbError;
use crate::verb::event::{VerbEvent, VerbLifecycleEvent};
use chrono::SecondsFormat;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Read-only toolset for the quiz verb. Excludes Write / Edit / Bash —
/// quiz only reads `wiki/`; raw-scope enforcement is the SKILL prompt
/// invariant (design D3, spike ❽ verified prompt-only is sufficient).
pub const QUIZ_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

const QUIZ_SCOPE_MARKER: &str = "[CODEBUS_QUIZ_SCOPE] ";
const QUIZ_NO_MATCH_MARKER: &str = "[CODEBUS_QUIZ_NO_MATCH] ";

/// Input to [`run_quiz_plan`] — a free-text learning topic (Goal flow).
#[derive(Debug, Clone)]
pub struct QuizPlanOptions {
    pub topic: String,
}

/// Parsed first-line marker outcome of the plan spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuizPlanOutcome {
    /// `[CODEBUS_QUIZ_SCOPE]` — the wiki pages a quiz should draw from
    /// (most relevant first). The caller confirms (GUI) or proceeds
    /// (CLI), then calls [`run_quiz_generate`] with this list.
    Scope(Vec<String>),
    /// `[CODEBUS_QUIZ_NO_MATCH]` — no wiki page covers the topic. No
    /// generate spawn should run; the reason is surfaced to the user.
    NoMatch(String),
}

/// Result of [`run_quiz_plan`]. Tokens/timestamps are for the plan
/// spawn only (the plan step is not persisted to RunLog).
#[derive(Debug, Clone)]
pub struct QuizPlanReport {
    pub outcome: QuizPlanOutcome,
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
}

/// Input to [`run_quiz_generate`] — a fixed page list (from a confirmed
/// plan, or `[target]` for the wiki-preview Page flow) + question count.
#[derive(Debug, Clone)]
pub struct QuizGenerateOptions {
    pub pages: Vec<String>,
    pub question_count: u8,
}

/// Successful generate-spawn summary. `quiz_md` is the fence-stripped
/// question body WITHOUT caller frontmatter — the caller injects
/// `quiz_id`, `trigger`, `topic`/`target_page`, `planned_pages`,
/// `generation_token_usage`, and `events_log` on persistence (design D4).
#[derive(Debug, Clone)]
pub struct QuizReport {
    pub quiz_md: String,
    pub planned_pages: Vec<String>,
    pub accumulated_tokens: TokenUsage,
    pub started_at: String,
    pub finished_at: String,
    pub agent_exit_code: Option<i32>,
    /// On-disk path of this run's events.jsonl, when a path-bearing sink
    /// is active (`None` under `log.sink: none`). Caller writes this as
    /// the `events_log` frontmatter pointer (design D4).
    pub events_log: Option<String>,
}

/// Parse the plan-spawn marker. Per spec the marker MUST be the first
/// line; the parser is tolerant on read. `[CODEBUS_QUIZ_SCOPE]` yields a
/// comma-separated `wiki/` path list (≥1 non-empty entry);
/// `[CODEBUS_QUIZ_NO_MATCH]` yields the trimmed reason. When no line
/// contains either marker, returns `None`.
pub(crate) fn parse_plan_outcome(text: &str) -> Option<QuizPlanOutcome> {
    // Tolerant recovery (design D6, mirroring the D4 generate-body
    // `strip_preamble_before_first_question` fix): strip a leading/
    // trailing code fence the agent may have wrapped its response in,
    // then accept the FIRST line that *contains* either marker — even
    // when the agent glued a preamble sentence onto the same line before
    // the marker with no newline (manual-e2e defect). The marker text
    // anywhere on the line is taken; everything after it is the payload.
    // The SKILL still mandates the marker on line 1; this is caller-side
    // robustness, not a relaxation of the agent contract.
    let stripped = strip_code_fence(text);
    for raw in stripped.lines() {
        let line = raw.trim();
        if let Some(idx) = line.find(QUIZ_SCOPE_MARKER) {
            let rest = &line[idx + QUIZ_SCOPE_MARKER.len()..];
            let pages: Vec<String> = rest
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if pages.is_empty() {
                return None;
            }
            return Some(QuizPlanOutcome::Scope(pages));
        }
        if let Some(idx) = line.find(QUIZ_NO_MATCH_MARKER) {
            let rest = line[idx + QUIZ_NO_MATCH_MARKER.len()..].trim();
            if rest.is_empty() {
                return None;
            }
            return Some(QuizPlanOutcome::NoMatch(rest.to_string()));
        }
    }
    None
}

/// Build the no-marker plan error WITH a truncated head (≤200 chars) of
/// the actual spawn output, so the failure is self-diagnosing instead of
/// opaque (design D6). Used on the `parse_plan_outcome` → `None` path.
pub(crate) fn plan_no_marker_error(plan_text: &str) -> VerbError {
    let head: String = plan_text.chars().take(200).collect();
    let suffix = if plan_text.chars().nth(200).is_some() {
        " …(truncated)"
    } else {
        ""
    };
    VerbError::Internal {
        message: format!(
            "quiz plan spawn produced no [CODEBUS_QUIZ_SCOPE]/\
             [CODEBUS_QUIZ_NO_MATCH] marker on any line; spawn output \
             head: {head}{suffix}"
        ),
    }
}

/// Strip a leading+trailing markdown code fence the agent may have
/// wrapped the quiz body in despite the SKILL prohibition (design D4
/// tolerant strip). Only strips when BOTH an opening fence at the very
/// start and a closing fence exist; otherwise returns the trimmed body.
pub(crate) fn strip_code_fence(body: &str) -> String {
    let trimmed = body.trim();
    if let Some(after_open) = trimmed.strip_prefix("```") {
        if let Some(nl) = after_open.find('\n') {
            let inner = &after_open[nl + 1..];
            if let Some(close) = inner.rfind("```") {
                return inner[..close].trim().to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Byte offset of the first `## Q<digits>.` question heading, wherever
/// it appears (even glued mid-line behind agent preamble). `## Q` is
/// ASCII so the returned index is always a valid char boundary.
fn first_question_index(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    const PAT: &[u8] = b"## Q";
    let mut i = 0usize;
    while i + PAT.len() <= bytes.len() {
        if &bytes[i..i + PAT.len()] == PAT {
            let mut j = i + PAT.len();
            let digits_start = j;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > digits_start && j < bytes.len() && bytes[j] == b'.' {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// Discard any agent preamble that precedes the first `## Q<n>.`
/// question heading — including a preamble the agent glued onto the
/// same line as `## Q1.` — so the cleaned body begins exactly at the
/// first question and `## Q1.` starts a line (design D7 / defect #4,
/// same tolerant-cleanup shape as `strip_code_fence`). When no question
/// heading exists at all, the body is returned unchanged so the
/// downstream "no well-formed questions" handling still owns that case.
pub(crate) fn strip_preamble_before_first_question(body: &str) -> String {
    match first_question_index(body) {
        Some(idx) => body[idx..].trim().to_string(),
        None => body.to_string(),
    }
}

fn is_cancelled(cancel: &Option<Arc<AtomicBool>>) -> bool {
    cancel
        .as_ref()
        .map(|f| f.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Load claude_code config + resolve the quiz verb settings + build env
/// overrides. Shared by both spawns (each verb call loads independently,
/// matching the existing verb convention).
fn load_quiz_agent_config()
-> Result<(crate::config::ResolvedVerb, crate::agent::EnvOverrides), VerbError> {
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => {
            load_claude_code_config(&p).map_err(|e| VerbError::ConfigParse {
                which: "claude_code",
                source: e,
            })?
        }
        _ => Default::default(),
    };
    let resolved = cc_cfg.resolve(Verb::Quiz);
    let env = build_env_overrides(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;
    Ok((resolved, env))
}

/// Run one spawn, accumulating assistant `text` (Thought) into a single
/// String while forwarding every stream event through `fan_out`. Emits
/// `SpawnStart` before and `SpawnEnd` after.
fn run_spawn(
    fan_out: &mut dyn FnMut(VerbEvent),
    slash_command: String,
    vault_root: std::path::PathBuf,
    resolved: &crate::config::ResolvedVerb,
    env: crate::agent::EnvOverrides,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<(String, InvokeReport), VerbError> {
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Quiz,
    }));
    let mut accumulated = String::new();
    let report = {
        let acc = &mut accumulated;
        let fan_out = &mut *fan_out;
        invoke(
            InvokeAgentOptions {
                slash_command,
                vault_root,
                toolset: QUIZ_TOOLSET,
                bash_whitelist: None,
                model: resolved.model.clone(),
                effort: resolved.effort.clone(),
                env,
                resume_session_id: None,
            },
            |event: StreamEvent| {
                if let StreamEvent::Thought { text } = &event {
                    acc.push_str(text);
                }
                fan_out(VerbEvent::Stream(event));
            },
            cancel,
        )
        .map_err(|e| VerbError::Spawn { source: e })?
    };
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd {
        verb: Verb::Quiz,
        exit_code: report.exit.code(),
    }));
    Ok((accumulated, report))
}

/// Run the **plan** spawn for the Goal flow. Emits plan-spawn
/// `VerbEvent`s through `on_event`, then a terminal
/// `QuizScopePlanned { pages }` or `QuizNoMatch { reason }` lifecycle
/// event, and returns the parsed [`QuizPlanReport`]. Does NOT continue
/// to generation — the caller interposes (CLI proceeds; GUI waits for
/// the user's confirm/revise).
///
/// `F` is a named generic so the signature is turbofish-addressable for
/// the spec's compile scenario.
pub fn run_quiz_plan<F: FnMut(VerbEvent)>(
    repo: &Path,
    options: QuizPlanOptions,
    mut on_event: F,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QuizPlanReport, VerbError> {
    let paths = vault_paths(repo);
    let started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Strict vault precondition (no auto-init; quiz is a wiki reader).
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    let (resolved, env) = load_quiz_agent_config()?;

    // The plan step is not persisted (RunLog/events.jsonl) — it is a
    // planning sub-step. Its stream is surfaced live via on_event for
    // the GUI activity stream; fan_out is just on_event here.
    let mut fan_out = |event: VerbEvent| on_event(event);

    let (plan_text, plan_report) = run_spawn(
        &mut fan_out,
        format!("/codebus-quiz plan: {}", options.topic),
        paths.root.clone(),
        &resolved,
        env,
        cancel.clone(),
    )?;

    if is_cancelled(&cancel) {
        return Err(VerbError::Cancelled);
    }

    let outcome = match parse_plan_outcome(&plan_text) {
        Some(QuizPlanOutcome::Scope(pages)) => {
            fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::QuizScopePlanned {
                pages: pages.clone(),
            }));
            QuizPlanOutcome::Scope(pages)
        }
        Some(QuizPlanOutcome::NoMatch(reason)) => {
            fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::QuizNoMatch {
                reason: reason.clone(),
            }));
            QuizPlanOutcome::NoMatch(reason)
        }
        None => {
            return Err(plan_no_marker_error(&plan_text));
        }
    };

    Ok(QuizPlanReport {
        outcome,
        accumulated_tokens: plan_report.accumulated_tokens,
        started_at,
        finished_at: plan_report.finished_at,
        agent_exit_code: plan_report.exit.code(),
    })
}

/// Run the **generate** spawn against a fixed page list + question
/// count. Persists a RunLog (mode `quiz`) + events.jsonl and returns the
/// fence-stripped [`QuizReport`]. Used by the Goal flow after a
/// confirmed plan, and directly by the Page flow (wiki-preview).
pub fn run_quiz_generate<F: FnMut(VerbEvent)>(
    repo: &Path,
    options: QuizGenerateOptions,
    mut on_event: F,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<QuizReport, VerbError> {
    let paths = vault_paths(repo);
    let run_started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    let (resolved, env) = load_quiz_agent_config()?;

    let log_cfg = load_verb_log_config().map_err(|e| VerbError::ConfigParse {
        which: "log",
        source: e,
    })?;
    let sink_cfg: SinkConfig = resolve_sink_dir(log_cfg, &paths.log);
    let mut events_sink: Box<dyn EventsSink> = match build_events_sink(&sink_cfg, &run_started_at) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("warning: events-log sink build failed (skipping persistence): {e}");
            Box::new(EventsNullSink::new())
        }
    };
    // Absolutize the events.jsonl pointer: the sink path is relative to
    // the CLI cwd, but `events_log` is persisted into the quiz markdown
    // and later read by the GUI (`read_quiz_events`) from a different
    // cwd, whose containment guard requires a vault-rooted path. Resolve
    // against cwd now so the pointer is stable regardless of reader cwd.
    let events_log: Option<String> = events_sink.events_path().map(|p| {
        std::path::absolute(&p)
            .unwrap_or(p)
            .display()
            .to_string()
    });

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

    let goal_text = options.pages.join(",");
    let (gen_text, gen_report) = run_spawn(
        &mut fan_out,
        format!(
            "/codebus-quiz generate: pages=[{}] count={}",
            options.pages.join(","),
            options.question_count
        ),
        paths.root.clone(),
        &resolved,
        env,
        cancel.clone(),
    )?;
    let quiz_md =
        strip_preamble_before_first_question(&strip_code_fence(&gen_text));

    let outcome = if is_cancelled(&cancel) {
        "cancelled"
    } else {
        "succeeded"
    };
    let run_log = RunLog {
        goal: goal_text,
        mode: "quiz".into(),
        model: resolved.model.clone(),
        effort: resolved.effort.clone(),
        started_at: run_started_at.clone(),
        finished_at: gen_report.finished_at.clone(),
        tokens: gen_report.accumulated_tokens.clone(),
        wiki_changed: false,
        lint_error_count: 0,
        lint_warn_count: 0,
        outcome: outcome.into(),
        session_id: gen_report.session_id.clone(),
    };
    write_run_log(sink_cfg, &run_log);

    if is_cancelled(&cancel) {
        return Err(VerbError::Cancelled);
    }

    let mut total_tokens = TokenUsage::default();
    accumulate_token_usage(&mut total_tokens, &gen_report.accumulated_tokens);

    Ok(QuizReport {
        quiz_md,
        planned_pages: options.pages,
        accumulated_tokens: total_tokens,
        started_at: run_started_at,
        finished_at: gen_report.finished_at,
        agent_exit_code: gen_report.exit.code(),
        events_log,
    })
}

/// Trigger provenance for a persisted quiz attempt. Drives the slug
/// (design D7: topic slug for `ai_planned`, page slug for
/// `wiki_preview`) and the `trigger` / `topic` / `target_page`
/// frontmatter (design D4).
#[derive(Debug, Clone)]
pub enum QuizTrigger {
    /// Sidebar `+ New quiz` Goal flow.
    AiPlanned { topic: String },
    /// Wiki-preview `[Quiz me on this]` Page flow.
    WikiPreview { target_page: String },
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Filesystem-safe slug: ASCII alphanumerics lower-cased and joined by
/// `-`, plus an FNV-1a 32-bit short hash so non-ASCII / collision-prone
/// inputs stay unique (design Open Question: "中文 topic → slug, hash
/// 後綴避免碰撞").
pub fn quiz_slug(input: &str) -> String {
    let mut base: String = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while base.contains("--") {
        base = base.replace("--", "-");
    }
    let base = base.trim_matches('-');
    let base: String = base.chars().take(40).collect();

    let mut hash: u32 = 0x811c_9dc5;
    for b in input.as_bytes() {
        hash ^= u32::from(*b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let suffix = format!("{hash:08x}");
    if base.is_empty() {
        format!("topic-{suffix}")
    } else {
        format!("{base}-{suffix}")
    }
}

/// Persist a generated quiz with caller-injected frontmatter (design
/// D4) under `<vault>/.codebus/quiz/<slug>/<quiz_id>.md` (design D7).
/// Single-source for the CLI and the GUI so slug + frontmatter logic
/// cannot drift between callers. Returns the written path.
pub fn persist_quiz(
    repo: &Path,
    trigger: &QuizTrigger,
    report: &QuizReport,
) -> std::io::Result<std::path::PathBuf> {
    let quiz_id = chrono::Utc::now()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
        .replace(':', "-");
    let (slug, trigger_str, topic_line, target_line) = match trigger {
        QuizTrigger::AiPlanned { topic } => (
            quiz_slug(topic),
            "ai_planned",
            format!("topic: {}\n", yaml_quote(topic)),
            "target_page: null\n".to_string(),
        ),
        QuizTrigger::WikiPreview { target_page } => (
            quiz_slug(target_page),
            "wiki_preview",
            "topic: null\n".to_string(),
            format!("target_page: {}\n", yaml_quote(target_page)),
        ),
    };
    let dir = vault_paths(repo).root.join("quiz").join(&slug);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{quiz_id}.md"));

    let mut fm = String::new();
    fm.push_str("---\n");
    fm.push_str(&format!("quiz_id: {quiz_id}\n"));
    fm.push_str(&format!("trigger: {trigger_str}\n"));
    fm.push_str(&topic_line);
    fm.push_str(&target_line);
    if report.planned_pages.is_empty() {
        fm.push_str("planned_pages: []\n");
    } else {
        fm.push_str("planned_pages:\n");
        for p in &report.planned_pages {
            fm.push_str(&format!("  - {p}\n"));
        }
    }
    fm.push_str("generation_token_usage:\n");
    fm.push_str(&format!(
        "  input: {}\n",
        report.accumulated_tokens.input_tokens
    ));
    fm.push_str(&format!(
        "  output: {}\n",
        report.accumulated_tokens.output_tokens
    ));
    match &report.events_log {
        Some(p) => fm.push_str(&format!("events_log: {}\n", yaml_quote(p))),
        None => fm.push_str("events_log: null\n"),
    }
    fm.push_str("---\n\n");
    fm.push_str(report.quiz_md.trim());
    fm.push('\n');

    std::fs::write(&path, fm)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn quiz_public_surface_compiles() {
        let _p = QuizPlanOptions {
            topic: "auth".into(),
        };
        let _g = QuizGenerateOptions {
            pages: vec!["wiki/modules/auth-middleware.md".into()],
            question_count: 5,
        };
        assert!(matches!(
            QuizPlanOutcome::Scope(vec!["wiki/a.md".into()]),
            QuizPlanOutcome::Scope(_)
        ));
        // Both orchestration fns resolve with the documented signatures.
        let _plan: fn(
            &Path,
            QuizPlanOptions,
            fn(VerbEvent),
            Option<Arc<AtomicBool>>,
        ) -> Result<QuizPlanReport, VerbError> = run_quiz_plan::<fn(VerbEvent)>;
        let _gen: fn(
            &Path,
            QuizGenerateOptions,
            fn(VerbEvent),
            Option<Arc<AtomicBool>>,
        ) -> Result<QuizReport, VerbError> = run_quiz_generate::<fn(VerbEvent)>;
    }

    #[test]
    fn quiz_toolset_is_read_only() {
        assert_eq!(QUIZ_TOOLSET, &["Read", "Glob", "Grep"]);
        assert!(!QUIZ_TOOLSET.contains(&"Write"));
        assert!(!QUIZ_TOOLSET.contains(&"Edit"));
        assert!(!QUIZ_TOOLSET.contains(&"Bash"));
    }

    // --- parse_plan_outcome ---

    #[test]
    fn parse_scope_marker_at_start() {
        let text = "[CODEBUS_QUIZ_SCOPE] wiki/concepts/jwt-token-lifecycle.md, \
                    wiki/modules/auth-middleware.md\n\nRationale...";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::Scope(vec![
                "wiki/concepts/jwt-token-lifecycle.md".into(),
                "wiki/modules/auth-middleware.md".into(),
            ]))
        );
    }

    #[test]
    fn parse_no_match_marker() {
        let text = "[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth; \
                    no page relates to baking\n";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::NoMatch(
                "vault only covers web auth; no page relates to baking".into()
            ))
        );
    }

    // fix-app-quiz defect #3 / design D6: the parser tolerantly
    // recovers the marker (strip leading fence, accept first marker
    // line despite preamble) — mirroring the D4 generate-body fence
    // tolerance. The SKILL still mandates the marker on line 1.

    #[test]
    fn parse_marker_after_preamble_is_recovered() {
        let text = "Sure, here is the scope.\n[CODEBUS_QUIZ_SCOPE] wiki/a.md";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::Scope(vec!["wiki/a.md".into()]))
        );
    }

    #[test]
    fn parse_marker_inside_code_fence_is_recovered() {
        let text = "```\n[CODEBUS_QUIZ_NO_MATCH] vault only covers web auth\n```";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::NoMatch(
                "vault only covers web auth".into()
            ))
        );
    }

    // Defect (manual e2e, quiz-e2e vault): the plan agent sometimes glues
    // a preamble sentence onto the SAME line as the marker (no newline
    // before it), e.g. `先掃描…頁面。[CODEBUS_QUIZ_SCOPE] wiki/a.md`. The
    // marker IS present, just not at line start. Same defect class as the
    // generate-body `strip_preamble_before_first_question` fix — the plan
    // parser must also accept a marker mid-line, not only as a line prefix.

    #[test]
    fn parse_scope_marker_glued_after_inline_preamble_is_recovered() {
        let text = "先掃描 `wiki/` 找 JWT authentication 相關頁面。\
                    [CODEBUS_QUIZ_SCOPE] wiki/synthesis/jwt-auth-system.md, \
                    wiki/concepts/jwt-pitfalls.md";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::Scope(vec![
                "wiki/synthesis/jwt-auth-system.md".into(),
                "wiki/concepts/jwt-pitfalls.md".into(),
            ]))
        );
    }

    #[test]
    fn parse_no_match_marker_glued_after_inline_preamble_is_recovered() {
        let text = "分析後沒有相關頁面。[CODEBUS_QUIZ_NO_MATCH] vault 只涵蓋 web auth";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::NoMatch("vault 只涵蓋 web auth".into()))
        );
    }

    #[test]
    fn plan_no_marker_error_includes_truncated_output_head() {
        let junk = "x".repeat(500);
        let err = plan_no_marker_error(&junk);
        let msg = match err {
            VerbError::Internal { message } => message,
            other => panic!("expected Internal, got {other:?}"),
        };
        assert!(
            msg.contains("xxxxx"),
            "error must include a head of the spawn output: {msg}"
        );
        // Head is truncated (≤200 chars of output), not the full 500.
        assert!(
            !msg.contains(&junk),
            "error must NOT embed the full spawn output"
        );
    }

    #[test]
    fn parse_scope_marker_empty_list_is_none() {
        assert_eq!(parse_plan_outcome("[CODEBUS_QUIZ_SCOPE] \n"), None);
        assert_eq!(parse_plan_outcome("[CODEBUS_QUIZ_SCOPE] , ,\n"), None);
    }

    #[test]
    fn parse_no_match_empty_reason_is_none() {
        assert_eq!(parse_plan_outcome("[CODEBUS_QUIZ_NO_MATCH] \n"), None);
    }

    #[test]
    fn parse_supports_non_ascii_reason() {
        let text = "[CODEBUS_QUIZ_NO_MATCH] vault 只涵蓋 web 認證，無量子力學頁面\n";
        assert_eq!(
            parse_plan_outcome(text),
            Some(QuizPlanOutcome::NoMatch(
                "vault 只涵蓋 web 認證，無量子力學頁面".into()
            ))
        );
    }

    // --- strip_code_fence ---

    #[test]
    fn strip_fence_with_language_tag() {
        let body = "```markdown\n## Q1. ...\n## Answer: A\n```";
        assert_eq!(strip_code_fence(body), "## Q1. ...\n## Answer: A");
    }

    #[test]
    fn strip_fence_absent_returns_trimmed_body() {
        let body = "\n## Q1. ...\n## Answer: B\n";
        assert_eq!(strip_code_fence(body), "## Q1. ...\n## Answer: B");
    }

    #[test]
    fn strip_fence_only_opening_is_not_stripped() {
        let body = "```\n## Q1. only opening";
        assert_eq!(strip_code_fence(body), "```\n## Q1. only opening");
    }

    // --- strip_preamble_before_first_question (defect #4 / design D7) ---

    #[test]
    fn strip_preamble_glued_onto_first_question() {
        let body = "讀取三個指定的 wiki 頁面以產生測驗題目。## Q1. stem\n\
                    - A) a\n## Answer: A\n## Explanation: e";
        let out = strip_preamble_before_first_question(body);
        assert!(
            out.starts_with("## Q1. stem"),
            "cleaned body must begin at the first question: {out:?}"
        );
        assert!(
            !out.contains("讀取三個指定的 wiki 頁面以產生測驗題目。"),
            "agent preamble must be discarded: {out:?}"
        );
    }

    #[test]
    fn strip_preamble_noop_when_already_clean() {
        let body = "## Q1. x\n## Answer: A\n## Explanation: e";
        assert_eq!(strip_preamble_before_first_question(body), body);
    }

    #[test]
    fn strip_preamble_noop_when_no_question_heading() {
        let body = "random text with no question heading at all";
        assert_eq!(strip_preamble_before_first_question(body), body);
    }

    // --- vault preconditions (no spawn) ---

    #[test]
    fn run_quiz_plan_returns_vault_missing_without_spawn() {
        let tmp = TempDir::new().unwrap();
        let events: std::cell::RefCell<Vec<VerbEvent>> = std::cell::RefCell::new(Vec::new());
        let result = run_quiz_plan(
            tmp.path(),
            QuizPlanOptions {
                topic: "auth".into(),
            },
            |e| events.borrow_mut().push(e),
            None,
        );
        match result {
            Err(VerbError::VaultMissing { path }) => {
                assert!(path.ends_with(".codebus"), "got {path:?}");
            }
            other => panic!("expected VaultMissing, got {other:?}"),
        }
        assert!(
            !events.borrow().iter().any(|e| matches!(
                e,
                VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { .. })
            )),
            "no spawn may start when vault is missing"
        );
    }

    #[test]
    fn run_quiz_generate_returns_vault_missing_without_spawn() {
        let tmp = TempDir::new().unwrap();
        let events: std::cell::RefCell<Vec<VerbEvent>> = std::cell::RefCell::new(Vec::new());
        let result = run_quiz_generate(
            tmp.path(),
            QuizGenerateOptions {
                pages: vec!["wiki/modules/auth-middleware.md".into()],
                question_count: 5,
            },
            |e| events.borrow_mut().push(e),
            None,
        );
        match result {
            Err(VerbError::VaultMissing { path }) => {
                assert!(path.ends_with(".codebus"), "got {path:?}");
            }
            other => panic!("expected VaultMissing, got {other:?}"),
        }
        assert!(
            !events.borrow().iter().any(|e| matches!(
                e,
                VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart { .. })
            )),
            "no spawn may start when vault is missing"
        );
    }

    // --- quiz_slug + persist_quiz (task 5.5 storage, design D4/D7) ---

    fn sample_report() -> QuizReport {
        QuizReport {
            quiz_md: "## Q1. x\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e"
                .into(),
            planned_pages: vec!["wiki/modules/auth.md".into()],
            accumulated_tokens: TokenUsage {
                input_tokens: 12,
                output_tokens: 7,
                ..Default::default()
            },
            started_at: "t0".into(),
            finished_at: "t1".into(),
            agent_exit_code: Some(0),
            events_log: Some("/v/.codebus/log/events-x.jsonl".into()),
        }
    }

    #[test]
    fn quiz_slug_is_filesystem_safe_and_unique() {
        let s = quiz_slug("How does AUTH work?");
        assert!(s.starts_with("how-does-auth-work-"));
        assert!(!s.contains(' '));
        assert!(!s.contains("--"));
        // Non-ASCII collapses to the hash-only form.
        let z = quiz_slug("量子力學");
        assert!(z.starts_with("topic-"));
        // Different inputs get different suffixes.
        assert_ne!(quiz_slug("auth"), quiz_slug("authz"));
    }

    #[test]
    fn persist_quiz_ai_planned_writes_frontmatter_and_body() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codebus")).unwrap();
        let path = persist_quiz(
            tmp.path(),
            &QuizTrigger::AiPlanned {
                topic: "how auth works".into(),
            },
            &sample_report(),
        )
        .expect("persist");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(path.to_string_lossy().contains("quiz"));
        assert!(body.starts_with("---\n"));
        assert!(body.contains("trigger: ai_planned"));
        assert!(body.contains("topic: \"how auth works\""));
        assert!(body.contains("target_page: null"));
        assert!(body.contains("- wiki/modules/auth.md"));
        assert!(body.contains("input: 12"));
        assert!(body.contains("events_log: \"/v/.codebus/log/events-x.jsonl\""));
        assert!(body.contains("## Q1. x"));
    }

    #[test]
    fn persist_quiz_wiki_preview_uses_target_page() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codebus")).unwrap();
        let path = persist_quiz(
            tmp.path(),
            &QuizTrigger::WikiPreview {
                target_page: "wiki/modules/auth-middleware.md".into(),
            },
            &sample_report(),
        )
        .expect("persist");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("trigger: wiki_preview"));
        assert!(body.contains("target_page: \"wiki/modules/auth-middleware.md\""));
        assert!(body.contains("topic: null"));
    }

    #[test]
    fn persist_quiz_is_non_destructive() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codebus")).unwrap();
        let trig = QuizTrigger::AiPlanned {
            topic: "same topic".into(),
        };
        let p1 = persist_quiz(tmp.path(), &trig, &sample_report()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let p2 = persist_quiz(tmp.path(), &trig, &sample_report()).unwrap();
        assert_ne!(p1, p2, "retry must not overwrite the prior attempt");
        assert!(p1.exists() && p2.exists());
        assert_eq!(
            p1.parent(),
            p2.parent(),
            "same topic → same slug directory"
        );
    }
}
