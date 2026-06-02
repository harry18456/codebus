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
use crate::agent::{
    AgentBackend, CommandPrefix, Permission, SpawnSpec, build_backend, invoke, load_provider_config,
};
use crate::config::{Verb, default_config_path};
use crate::log::events::{EventEnvelope, EventsNullSink, EventsSink};
use crate::log::factory::build_events_sink;
use crate::log::sink::accumulate_token_usage;
use crate::log::verb_log::{load_verb_log_config, resolve_sink_dir, write_run_log};
use crate::log::{InterruptReason, RunLog, SinkConfig, TokenUsage};
use crate::stream::StreamEvent;
use crate::vault::layout::vault_paths;
use crate::verb::content_verify::{
    ContentDefect, ContentVerifyOutcome, parse_content_defects, run_content_verify_loop,
};
use crate::verb::error::VerbError;
use crate::verb::event::{VerbBanner, VerbEvent, VerbLifecycleEvent};
use crate::verb::quiz_validate::validate_quiz_body;
use chrono::SecondsFormat;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Read-only toolset for the quiz **plan** spawn. Excludes Write / Edit
/// / Bash — planning only reads `wiki/`; raw-scope enforcement is the
/// SKILL prompt invariant (design D3, spike ❽ verified prompt-only is
/// sufficient).
pub const QUIZ_TOOLSET: &[&str] = &["Read", "Glob", "Grep"];

/// Toolset for the quiz **generate** spawn: read-only plus a Bash tool
/// hard-gated to the quiz validator (design «Bash sandbox»). The agent
/// self-validates its in-context draft via stdin — it has no scratch
/// file to write, so Write / Edit are deliberately NOT added (this is a
/// process-shape consequence, not a least-privilege claim).
pub const QUIZ_GENERATE_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Bash"];

/// `--allowedTools` fine-grained Bash specifier for the generate spawn:
/// the agent may invoke only `codebus quiz validate ...` (mirrors
/// `wiki::fix::FIX_BASH_WHITELIST`'s `Bash(codebus lint *)` shape).
pub const QUIZ_BASH_WHITELIST: &str = "Bash(codebus quiz validate *)";

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
    /// quiz-content-verify (design D5): run the optional independent
    /// content verify + caller-orchestrated repair loop after the
    /// deterministic final-verify. Caller-injected from `quiz.content_verify`
    /// config (the library never reads config itself).
    pub content_verify: bool,
    /// quiz-content-verify (design D6): the user's originating topic for
    /// the off-topic content check. `Some` for the Goal flow, `None` for
    /// the Page flow (off-topic check skipped, other four still run).
    pub topic: Option<String>,
}

/// Independent content-verify outcome (design D2/D4) persisted as the
/// `content_review` caller frontmatter field. `None` on `QuizReport`
/// means content verification was not run (config off) — readers MUST
/// NOT treat absence as `Ok`.
///
/// goal-content-verify D1: this is now a re-export alias of the shared
/// [`crate::verb::content_verify::ContentReview`] — the type, its
/// variants, and `frontmatter_value` (`ok` / `flagged [1, 3]`) are
/// unchanged from the pre-extraction quiz form.
pub type QuizContentReview = crate::verb::content_verify::ContentReview;

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
    /// Deterministic final-verify outcome over `quiz_md` (design D4).
    /// `Ok` when the validator found zero issues; `Failed` when residual
    /// schema / broken-citation findings remain. The caller persists
    /// this as the `validation:` frontmatter field; an absent field is
    /// "not validated" and MUST NOT be read as `Ok`.
    pub validation: QuizValidation,
    /// Independent content-verify outcome (design D2/D4). `None` when
    /// `content_verify` was off (not run) — readers MUST NOT treat
    /// `None` as `Ok`. `Some` is persisted as `content_review:`.
    pub content_review: Option<QuizContentReview>,
}

/// Final-verify status carried on [`QuizReport`] and persisted as the
/// `validation:` frontmatter field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuizValidation {
    Ok,
    Failed,
}

impl QuizValidation {
    /// Frontmatter scalar (`validation: ok` / `validation: failed`).
    pub fn as_str(&self) -> &'static str {
        match self {
            QuizValidation::Ok => "ok",
            QuizValidation::Failed => "failed",
        }
    }
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
///
/// Returns `(resolved, verify_resolved, env)`:
/// - `resolved` — `Verb::Quiz` settings used by plan / generate / repair spawns
/// - `verify_resolved` — `Verb::Verify` settings used by the content-verify spawn
///   (per `verify-stage-independent-model`; see spec `claude-code-config`
///   `Endpoint Profile Schema` and spec `quiz` `Quiz Content Verification and Repair`)
fn load_quiz_agent_config() -> Result<
    (
        crate::config::ResolvedVerb,
        crate::config::ResolvedVerb,
        Box<dyn AgentBackend>,
    ),
    VerbError,
> {
    let cc_cfg = match default_config_path() {
        Some(p) if p.exists() => {
            load_provider_config(&p).map_err(|e| VerbError::ConfigParse {
                which: "claude_code",
                source: e,
            })?
        }
        _ => Default::default(),
    };
    // resolved / verify_resolved are kept for the caller's RunLog (model /
    // effort fields); the backend resolves model/effort per spawn from its
    // own copy of the config via the SpawnSpec's verb.
    let resolved = cc_cfg.resolve(Verb::Quiz);
    let verify_resolved = cc_cfg.resolve(Verb::Verify);
    let backend = build_backend(&cc_cfg).map_err(|e| VerbError::KeyringMissing { source: e })?;
    Ok((resolved, verify_resolved, backend))
}

/// Compose the verify-spawn input payload (spec `quiz` / Quiz Content
/// Verification and Repair — Verify spawn input includes planned page
/// list). The verify spawn agent SHALL receive an originating-topic
/// segment, a `PLANNED PAGES:` block (one vault-relative path per line —
/// empty body when no pages planned), and a `QUIZ:` block carrying the
/// generated quiz body. Closes prompt-surface-review F93 (verify agent
/// previously had to reverse-engineer page coverage from `[[slug]]`
/// citations, causing missed pages and unparseable verify output).
fn compose_verify_input(topic: &str, pages: &[String], body: &str) -> String {
    let pages_block = pages.join("\n");
    format!("topic={topic}\n\nPLANNED PAGES:\n{pages_block}\n\nQUIZ:\n{body}")
}

/// Compose the generate-spawn input payload (spec `quiz` / Quiz Generate
/// Spawn Carries Topic Language Signal). The Page flow (`topic` = `None`)
/// reproduces the historical `pages=[...] count=N` shape byte for byte so
/// existing behavior is untouched; the Goal flow (`topic` = `Some`) prepends
/// a `topic=<topic>` segment as the language signal the generate agent
/// follows per the §0 Language Policy quiz rule. codebus does not parse this
/// payload — the topic is free-text fed to the agent, so spaces / CJK /
/// commas inside it are fine (quiz-output-language-follows-topic).
fn compose_generate_input(topic: Option<&str>, pages: &[String], count: u8) -> String {
    let core = format!("pages=[{}] count={}", pages.join(","), count);
    match topic {
        Some(t) => format!("topic={t}\n{core}"),
        None => core,
    }
}

/// Compose the content-verify repair (regenerate) spawn input. Reuses
/// [`compose_generate_input`] for the leading `topic=`/`pages=` segment so a
/// repair iteration carries the same topic language signal as the original
/// generate, then appends the revise-only-flagged instructions plus the
/// previous quiz body and the content defect lines.
fn compose_repair_input(
    topic: Option<&str>,
    pages: &[String],
    count: u8,
    body: &str,
    defect_lines: &str,
) -> String {
    format!(
        "{}\n\nThe previous quiz had content defects. Revise ONLY the flagged questions, keep all other questions verbatim, and keep exactly {count} questions.\n\nPREVIOUS QUIZ:\n{body}\n\nCONTENT DEFECTS:\n{defect_lines}",
        compose_generate_input(topic, pages, count),
    )
}

/// Run one spawn, accumulating assistant `text` (Thought) into a single
/// String while forwarding every stream event through `fan_out`. Emits
/// `SpawnStart` before and `SpawnEnd` after.
#[allow(clippy::too_many_arguments)]
fn run_spawn(
    fan_out: &mut dyn FnMut(VerbEvent),
    backend: &dyn crate::agent::AgentBackend,
    sub_mode: Option<String>,
    input: String,
    vault_root: &std::path::Path,
    resolve_as: Option<Verb>,
    permission: Permission,
    command_allowance: Option<CommandPrefix>,
    cancel: Option<Arc<AtomicBool>>,
    timeout: Option<std::time::Duration>,
) -> Result<(String, InvokeReport), VerbError> {
    // Lifecycle phase stays `Verb::Quiz` (UI grouping). The SKILL bundle is
    // always Quiz (cross-flow verify spawn still invokes /codebus-quiz verify:);
    // `resolve_as` is the model-resolution override (Some(Verify) for the
    // content-verify spawn, None for plan/generate which use Quiz config).
    fan_out(VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnStart {
        verb: Verb::Quiz,
    }));
    let mut accumulated = String::new();
    let report = {
        let acc = &mut accumulated;
        let fan_out = &mut *fan_out;
        invoke(
            backend,
            SpawnSpec {
                verb: Verb::Quiz,
                resolve_as,
                sub_mode,
                input,
                permission,
                command_allowance,
                resume_session_id: None,
            },
            vault_root,
            |event: StreamEvent| {
                if let StreamEvent::Thought { text } = &event {
                    acc.push_str(text);
                }
                fan_out(VerbEvent::Stream(event));
            },
            cancel,
            timeout,
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
    timeout: Option<std::time::Duration>,
) -> Result<QuizPlanReport, VerbError> {
    let paths = vault_paths(repo);
    let started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);

    // Strict vault precondition (no auto-init; quiz is a wiki reader).
    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    let (_resolved, _verify_resolved, backend) = load_quiz_agent_config()?;

    // The plan step is not persisted (RunLog/events.jsonl) — it is a
    // planning sub-step. Its stream is surfaced live via on_event for
    // the GUI activity stream; fan_out is just on_event here.
    let mut fan_out = |event: VerbEvent| on_event(event);

    let (plan_text, plan_report) = run_spawn(
        &mut fan_out,
        &*backend,
        Some("plan".to_string()),
        options.topic.clone(),
        &paths.root,
        None,
        Permission::ReadOnly,
        None,
        cancel.clone(),
        timeout,
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
    timeout: Option<std::time::Duration>,
) -> Result<QuizReport, VerbError> {
    let paths = vault_paths(repo);
    // Millis precision: matches the IPC `active_runs` key precision so
    // the orphan-detection invariant (events-file slug ↔ active_runs
    // key) holds. See app-workspace § Interrupted Run Detection NOTE.
    let run_started_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    if !paths.root.exists() {
        return Err(VerbError::VaultMissing {
            path: paths.root.clone(),
        });
    }

    let (resolved, _verify_resolved, backend) = load_quiz_agent_config()?;

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
        &*backend,
        Some("generate".to_string()),
        compose_generate_input(
            options.topic.as_deref(),
            &options.pages,
            options.question_count,
        ),
        &paths.root,
        None,
        Permission::ReadOnly,
        Some(CommandPrefix::new(["codebus", "quiz", "validate"])),
        cancel.clone(),
        timeout,
    )?;
    let mut quiz_md =
        strip_preamble_before_first_question(&strip_code_fence(&gen_text));

    // Deterministic final verify (design D1/D3/D4). Run the same
    // validator the `codebus quiz validate` sub-action / agent
    // self-check use, over the fence/preamble-stripped body. Surface the
    // outcome through the same `fan_out` (events.jsonl + on_event) using
    // the existing lint-outcome banner shape — no new VerbEvent variant,
    // mirroring how `wiki::fix` surfaces lint. Residual findings are
    // best-effort: the quiz is still persisted (caller side) with a
    // `validation: failed` marker, a non-fatal warning is surfaced, no
    // question is dropped, and the verb does NOT fail for this reason.
    let findings = validate_quiz_body(&quiz_md, &paths.wiki);
    fan_out(VerbEvent::Banner(VerbBanner::LintDone {
        errors: findings.len(),
        warns: 0,
        elapsed_ms: 0,
    }));
    let validation = if findings.is_empty() {
        QuizValidation::Ok
    } else {
        eprintln!(
            "warning: quiz validation found {} issue(s) (non-fatal; quiz persisted with validation: failed):",
            findings.len()
        );
        for f in &findings {
            eprintln!("  [{}] {}: {}", f.rule_id, f.path, f.message);
        }
        QuizValidation::Failed
    };

    // quiz-content-verify (design D1/D3/D4/D6): when enabled, run an
    // independent read-only verify spawn judging content against the
    // five-item defect contract, then a caller-orchestrated bounded
    // repair loop (verify → repair generate spawn fed the defects →
    // re-verify), hard-capped at 3 iterations with emit-best-on-cap.
    // Residual / spawn-failure is best-effort: `content_review: flagged`,
    // a non-fatal warning, no question dropped, exit unchanged.
    let content_review: Option<QuizContentReview> = if options.content_verify {
        // goal-content-verify D1: the verify→repair orchestration now
        // lives in the shared `verb::content_verify` core. quiz injects
        // its two adapters (verify = a read-only verify spawn parsed via
        // the shared parser; repair = a regenerate spawn whose stripped
        // body becomes the new draft) and maps the shared outcome back
        // to the unchanged `content_review` frontmatter form. Behavior
        // (events, cap, best-effort, persisted value) is preserved.
        let topic_arg = options.topic.clone().unwrap_or_default();
        let fan_cell = std::cell::RefCell::new(&mut fan_out);

        let verify = |body: &String| -> Result<Option<Vec<ContentDefect>>, VerbError> {
            // verify-stage-independent-model: bundle=Quiz (invokes
            // /codebus-quiz verify:); resolve_as=Some(Verify) so model/effort
            // come from the dedicated verify config sub-block.
            let verify_input = compose_verify_input(&topic_arg, &options.pages, body);
            let vtext = match run_spawn(
                &mut **fan_cell.borrow_mut(),
                &*backend,
                Some("verify".to_string()),
                verify_input,
                &paths.root,
                Some(Verb::Verify),
                Permission::ReadOnly,
                None,
                cancel.clone(),
                timeout,
            ) {
                Ok((t, _)) => t,
                Err(e) => {
                    eprintln!(
                        "warning: quiz content-verify spawn failed (non-fatal; content_review: flagged): {e}"
                    );
                    return Err(e);
                }
            };
            let parsed = parse_content_defects(&vtext);
            if parsed.is_none() {
                eprintln!(
                    "warning: quiz content-verify output unparseable (non-fatal; content_review: flagged)"
                );
            }
            Ok(parsed)
        };

        let repair =
            |body: &String, defects: &[ContentDefect]| -> Result<String, VerbError> {
                let defect_lines = defects
                    .iter()
                    .map(|x| format!("{} | {} | {}", x.id, x.kind, x.suggestion))
                    .collect::<Vec<_>>()
                    .join("\n");
                // Regenerate (retry) spawn: bundle=Quiz, sub_mode=generate,
                // resolve_as=None (uses Quiz config — generate, not verify).
                let repair_input = compose_repair_input(
                    options.topic.as_deref(),
                    &options.pages,
                    options.question_count,
                    body,
                    &defect_lines,
                );
                match run_spawn(
                    &mut **fan_cell.borrow_mut(),
                    &*backend,
                    Some("generate".to_string()),
                    repair_input,
                    &paths.root,
                    None,
                    Permission::ReadOnly,
                    Some(CommandPrefix::new(["codebus", "quiz", "validate"])),
                    cancel.clone(),
                    timeout,
                ) {
                    Ok((rtext, _)) => {
                        Ok(strip_preamble_before_first_question(&strip_code_fence(&rtext)))
                    }
                    Err(e) => {
                        eprintln!(
                            "warning: quiz content-repair spawn failed (non-fatal; keeping best body, content_review: flagged): {e}"
                        );
                        Err(e)
                    }
                }
            };

        let (new_md, outcome) =
            run_content_verify_loop(quiz_md, || is_cancelled(&cancel), verify, repair);
        quiz_md = new_md;
        let review = match outcome {
            ContentVerifyOutcome::Ok => QuizContentReview::Ok,
            ContentVerifyOutcome::Flagged(defects) => {
                let qnums: Vec<u32> = defects
                    .iter()
                    .filter_map(|d| {
                        d.id.strip_prefix('Q').and_then(|s| s.parse::<u32>().ok())
                    })
                    .collect();
                eprintln!(
                    "warning: quiz content-verify flagged {} question(s) after repair cap (non-fatal; persisted content_review: flagged)",
                    qnums.len()
                );
                QuizContentReview::Flagged(qnums)
            }
        };
        Some(review)
    } else {
        None
    };

    let cancelled_now = is_cancelled(&cancel);
    // run-outcome-lifecycle-integrity: cancel > timeout > succeeded. A
    // wall-clock timeout on the generate spawn forces failed + Timeout.
    let (outcome, interrupt_reason) = if cancelled_now {
        ("cancelled", Some(InterruptReason::UserCancel))
    } else if gen_report.timed_out {
        ("failed", Some(InterruptReason::Timeout))
    } else {
        ("succeeded", None)
    };
    crate::verb::warn_sandbox_denials(gen_report.sandbox_denial_count);
    let run_log = RunLog {
        goal: goal_text,
        mode: "quiz".into(),
        model: resolved.model.clone(),
        effort: resolved.effort.clone(),
        started_at: run_started_at.clone(),
        finished_at: gen_report.finished_at.clone(),
        tokens: gen_report.accumulated_tokens.clone(),
        wiki_changed: false,
        lint_error_count: findings.len(),
        lint_warn_count: 0,
        outcome: outcome.into(),
        session_id: gen_report.session_id.clone(),
        sandbox_denial_count: gen_report.sandbox_denial_count,
        interrupt_reason,
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
        validation,
        content_review,
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
    // Deterministic final-verify outcome (design D4). Absent field =
    // "not validated"; readers MUST NOT treat absent as `ok`.
    fm.push_str(&format!("validation: {}\n", report.validation.as_str()));
    // Independent content-verify outcome (design D2/D4). Written only
    // when content verification ran; absent = not verified, readers
    // MUST NOT treat absent as `ok`.
    if let Some(cr) = &report.content_review {
        fm.push_str(&format!("content_review: {}\n", cr.frontmatter_value()));
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
            content_verify: false,
            topic: None,
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
            Option<std::time::Duration>,
        ) -> Result<QuizPlanReport, VerbError> = run_quiz_plan::<fn(VerbEvent)>;
        let _gen: fn(
            &Path,
            QuizGenerateOptions,
            fn(VerbEvent),
            Option<Arc<AtomicBool>>,
            Option<std::time::Duration>,
        ) -> Result<QuizReport, VerbError> = run_quiz_generate::<fn(VerbEvent)>;
    }

    #[test]
    fn quiz_toolset_is_read_only() {
        assert_eq!(QUIZ_TOOLSET, &["Read", "Glob", "Grep"]);
        assert!(!QUIZ_TOOLSET.contains(&"Write"));
        assert!(!QUIZ_TOOLSET.contains(&"Edit"));
        assert!(!QUIZ_TOOLSET.contains(&"Bash"));
    }

    /// design «Bash sandbox» + spec `cli` / Quiz Validate Sub-Action
    /// Behavior: the generate spawn (only) gets Bash hard-gated to
    /// `codebus quiz validate *`, and it MUST NOT gain Write/Edit (the
    /// agent self-validates via stdin, not a scratch file).
    #[test]
    fn quiz_generate_toolset_has_gated_bash_no_write() {
        assert_eq!(QUIZ_BASH_WHITELIST, "Bash(codebus quiz validate *)");
        assert!(QUIZ_GENERATE_TOOLSET.contains(&"Bash"));
        assert!(QUIZ_GENERATE_TOOLSET.contains(&"Read"));
        assert!(!QUIZ_GENERATE_TOOLSET.contains(&"Write"));
        assert!(!QUIZ_GENERATE_TOOLSET.contains(&"Edit"));
        // plan spawn stays read-only (no Bash).
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
                content_verify: false,
                topic: None,
            },
            |e| events.borrow_mut().push(e),
            None,
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
            validation: QuizValidation::Ok,
            content_review: None,
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

    // --- F93 (prompt-surface-output-discipline-batch): Quiz Content
    // Verification and Repair — verify spawn input includes planned page list ---

    #[test]
    fn verify_input_includes_planned_pages() {
        let topic = "JWT issuance and verification";
        let pages = vec![
            "wiki/processes/jwt-issue-and-verify.md".to_string(),
            "wiki/modules/auth-module.md".to_string(),
            "wiki/entities/jwt-payload.md".to_string(),
        ];
        let body = "## Q1. stem\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: see [[jwt-payload]]";
        let input = compose_verify_input(topic, &pages, body);

        assert!(
            input.contains(&format!("topic={topic}")),
            "verify input must carry topic segment; got:\n{input}"
        );
        assert!(
            input.contains("PLANNED PAGES:"),
            "verify input must carry literal `PLANNED PAGES:` header; got:\n{input}"
        );
        for p in &pages {
            assert!(
                input.lines().any(|l| l.trim() == p),
                "verify input must list each planned page on its own line; missing `{p}` in:\n{input}"
            );
        }
        assert!(
            input.contains("QUIZ:"),
            "verify input must carry the literal `QUIZ:` header; got:\n{input}"
        );
        assert!(
            input.contains(body),
            "verify input must include the generated quiz body verbatim; got:\n{input}"
        );
    }

    #[test]
    fn verify_input_empty_pages_segment() {
        // Spec scenario: verify spawn input includes empty page block when none planned.
        let input = compose_verify_input("auth", &[], "## Q1. stem ...");
        assert!(
            input.contains("PLANNED PAGES:"),
            "even an empty planned-pages list must keep the `PLANNED PAGES:` header; got:\n{input}"
        );
        // No path lines under the empty block; substring lookup must not find a wiki/ path.
        assert!(
            !input.lines().any(|l| l.trim().starts_with("wiki/")),
            "empty-pages input must not list any wiki/ path; got:\n{input}"
        );
        assert!(
            input.contains("QUIZ:"),
            "QUIZ: header still required; got:\n{input}"
        );
    }

    #[test]
    fn compose_generate_input_some_carries_topic() {
        // quiz-output-language-follows-topic: Goal flow threads the topic so
        // the generate agent has a language signal to follow.
        let pages = vec!["wiki/a.md".to_string(), "wiki/b.md".to_string()];
        let input = compose_generate_input(Some("JWT 簽發與驗證"), &pages, 5);
        assert!(
            input.contains("topic=JWT 簽發與驗證"),
            "Goal-flow generate input must carry the topic segment; got:\n{input}"
        );
        assert!(
            input.contains("pages=[wiki/a.md,wiki/b.md]") && input.contains("count=5"),
            "generate input must keep the pages/count segment; got:\n{input}"
        );
    }

    #[test]
    fn compose_generate_input_none_omits_topic_equals_old_shape() {
        // Page flow (topic=None) must reproduce the pre-change shape byte for
        // byte so existing behavior is untouched.
        let pages = vec!["wiki/a.md".to_string(), "wiki/b.md".to_string()];
        let input = compose_generate_input(None, &pages, 5);
        let old_shape = format!("pages=[{}] count={}", pages.join(","), 5);
        assert_eq!(
            input, old_shape,
            "None must reproduce the historical generate shape exactly"
        );
        assert!(
            !input.contains("topic="),
            "Page-flow input must not carry a topic segment; got:\n{input}"
        );
    }

    #[test]
    fn compose_repair_input_some_carries_topic() {
        // The content-verify repair (regenerate) spawn must carry the same
        // topic signal so a repair iteration cannot drift the quiz language.
        let pages = vec!["wiki/a.md".to_string()];
        let body = "## Q1. stem";
        let defects = "Q1 | answer-wrong | fix it";
        let input = compose_repair_input(Some("中文主題"), &pages, 3, body, defects);
        assert!(
            input.contains("topic=中文主題"),
            "repair input must carry the topic segment; got:\n{input}"
        );
        assert!(
            input.contains("PREVIOUS QUIZ:") && input.contains("CONTENT DEFECTS:"),
            "repair input must keep its previous-quiz / defects blocks; got:\n{input}"
        );
        assert!(
            input.contains(body) && input.contains(defects),
            "repair input must include the previous body and defect lines verbatim; got:\n{input}"
        );
    }

    #[test]
    fn compose_repair_input_none_omits_topic() {
        // Page-flow repair keeps the topic-less prefix.
        let pages = vec!["wiki/a.md".to_string()];
        let input = compose_repair_input(None, &pages, 3, "## Q1. stem", "Q1 | x | y");
        assert!(
            !input.contains("topic="),
            "Page-flow repair input must not carry a topic segment; got:\n{input}"
        );
        assert!(input.contains("pages=[wiki/a.md] count=3"), "got:\n{input}");
    }
}
