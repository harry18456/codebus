//! `LogSink` trait + shared `RunLog` / `TokenUsage` shapes.
//!
//! v3-run-log carry from v2 with two simplifications:
//!   - Drop async — sync trait. `JsonlSink` wraps `BufWriter`; OTel SDKs
//!     do their own batching internally; nothing requires `.await`.
//!   - Drop the `OpenTelemetry` variant from `SinkConfig` (see `factory.rs`)
//!     until there is real demand; v3 ships only Null + Jsonl.
//!
//! `Send + Sync` so a single sink instance can outlive a verb invocation if
//! a future `codebus daemon` mode keeps one process across multiple runs.

use serde::{Deserialize, Serialize};

/// Token usage for one LLM invocation, normalized across providers.
///
/// `input_tokens` / `output_tokens` are universal — every LLM API exposes
/// them. The remaining fields are `Option<u64>` because not every provider
/// has them (e.g. OpenAI legacy / Ollama have no cache concept). `None`
/// means "the provider does not have this concept"; `Some(0)` means "the
/// concept exists but no tokens were attributed this run".
///
/// `extras` is the escape hatch for vendor-specific fields the normalized
/// shape can't carry. Providers SHOULD place the original wire-format
/// `usage` object here so downstream tools can recover full fidelity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// Reasoning tokens for o-series / extended-thinking models that bill
    /// reasoning separately from output. `None` for providers without this
    /// concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Vendor-specific raw JSON. Default `Value::Null` is skipped during
    /// serialization to keep jsonl entries compact.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub extras: serde_json::Value,
}

/// One row in the run log. Captures everything a future analytics consumer
/// would want without needing per-event detail.
///
/// Timestamps are RFC 3339 strings (rather than `chrono::DateTime`) so the
/// serialized shape is human-grepable in jsonl files without round-tripping
/// through chrono parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLog {
    /// The goal / query text that triggered the run. Empty string for
    /// `fix` (which has no positional argument). For `chat`, the
    /// per-turn user prompt text. For `quiz`, the comma-joined list of
    /// the selected page paths the quiz was generated from
    /// (`options.pages.join(",")`).
    pub goal: String,
    /// `"goal"` / `"query"` / `"fix"` / `"chat"` / `"quiz"`. Required
    /// because runs.jsonl mixes modes; consumers filter on this field.
    pub mode: String,
    /// Model alias / id passed to the provider, if any. `None` when the
    /// caller did not configure a model (provider used its default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Reasoning effort level passed to the provider, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// RFC 3339 UTC start timestamp.
    pub started_at: String,
    /// RFC 3339 UTC end timestamp.
    pub finished_at: String,
    pub tokens: TokenUsage,
    pub wiki_changed: bool,
    pub lint_error_count: usize,
    pub lint_warn_count: usize,
    /// Verb termination outcome. Closed set of three values:
    /// `"succeeded"` — agent exited zero + any post-spawn phases ok;
    /// `"failed"` — agent exited non-zero or fix loop reported issues;
    /// `"cancelled"` — caller cancel signal observed mid-run (RunLog
    /// written before returning `VerbError::Cancelled` per the
    /// `Cancellation Signal Polling` requirement of `verb-library`).
    /// Serde default `"succeeded"` gives forward-compat for legacy
    /// jsonl rows written before v3-run-log-events shipped.
    #[serde(default = "default_outcome")]
    pub outcome: String,
    /// Claude CLI session id for the spawned `claude` child process,
    /// extracted from the spawn's first `init` stream event.
    ///
    /// Semantics by mode (v3-chat-verb):
    /// - `mode == "chat"`: SHALL always be `Some(<session_id>)`. Every chat
    ///   turn spawns through `agent::invoke` and the init event always emits
    ///   a session_id, which the chat verb writes here so a multi-turn chat
    ///   REPL session produces multiple `RunLog` entries sharing the same
    ///   session_id value.
    /// - `mode == "goal" / "query" / "fix"`: SHALL always be `None`. These
    ///   verbs do not currently expose session resume to the user; the field
    ///   is reserved for future expansion if any of them grows multi-turn
    ///   behavior.
    /// - `mode == "quiz"`: carries the generate spawn's session_id (typically
    ///   `Some(<session_id>)` on a completed spawn, mirroring `chat`). Unlike
    ///   `chat`, the quiz verb does not resume from it — it is recorded for
    ///   logging only.
    ///
    /// Serde `default + skip_serializing_if = "Option::is_none"` so legacy
    /// jsonl rows written before v3-chat-verb deserialize cleanly to `None`
    /// and serialized rows for goal/query/fix omit the field entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Classification of why the run did not reach the success path.
    ///
    /// Orthogonal to [`outcome`](Self::outcome) — `outcome` stays the closed
    /// set `"succeeded" / "failed" / "cancelled"`; `interrupt_reason`
    /// classifies the cause when present. Populated by:
    /// - `AppClose`: GUI Interrupted Run Detection synthesizing a virtual
    ///   entry from an orphan events jsonl file at next app launch.
    /// - `UserCancel`: verb-layer cancel signal handler when the
    ///   caller-supplied cancel flag flipped to `true` mid-run.
    /// - `NetworkDrop`: external connection error that aborted the verb.
    /// - `Other(String)`: free-form fallback for future classifications not
    ///   yet promoted to a named variant.
    ///
    /// Serde `default + skip_serializing_if = "Option::is_none"` so legacy
    /// jsonl rows written before this change deserialize cleanly to `None`
    /// and rows without a reason (e.g. normal `outcome == "succeeded"` runs)
    /// omit the field entirely.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interrupt_reason: Option<InterruptReason>,
}

/// Why a run did not reach the success path. See
/// [`RunLog::interrupt_reason`] for population rules.
///
/// JSON wire shape (per `#[serde(rename_all = "kebab-case")]`):
/// - `AppClose`    → `"app-close"`
/// - `UserCancel`  → `"user-cancel"`
/// - `NetworkDrop` → `"network-drop"`
/// - `Other(String)` → `{"other": "<string>"}` (untagged newtype)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterruptReason {
    AppClose,
    UserCancel,
    NetworkDrop,
    Other(String),
}

fn default_outcome() -> String {
    "succeeded".to_string()
}

#[derive(Debug)]
pub enum LogError {
    Io(std::io::Error),
    Serialize(serde_json::Error),
}

impl std::fmt::Display for LogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogError::Io(e) => write!(f, "log sink io: {e}"),
            LogError::Serialize(e) => write!(f, "log sink serialize: {e}"),
        }
    }
}

impl std::error::Error for LogError {}

impl From<std::io::Error> for LogError {
    fn from(e: std::io::Error) -> Self {
        LogError::Io(e)
    }
}

impl From<serde_json::Error> for LogError {
    fn from(e: serde_json::Error) -> Self {
        LogError::Serialize(e)
    }
}

/// Object-safe sink for run summaries. Sinks may buffer internally; callers
/// SHOULD call [`flush`](Self::flush) before process exit (the v3 verb
/// commands do this implicitly via `drop` — `JsonlSink` writes synchronously
/// per-call so flush is currently a no-op, but kept on the trait for future
/// buffering impls).
pub trait LogSink: Send + Sync {
    /// Stable identifier (`"null"` / `"jsonl"` / future `"otel"`).
    fn name(&self) -> &str;

    /// Persist one [`RunLog`] entry. Synchronous — caller blocks until the
    /// underlying I/O completes (or fails).
    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError>;

    /// Flush any internal buffers. Default no-op for buffer-free sinks.
    fn flush(&mut self) -> Result<(), LogError> {
        Ok(())
    }
}

/// Field-by-field accumulator for [`TokenUsage`]. Used by stream consumers
/// that may receive multiple `result` events per spawn (e.g., long sessions
/// with cache rotation): accumulate every event, then write the final sum
/// into [`RunLog::tokens`].
///
/// `input_tokens` / `output_tokens` use `saturating_add` so a pathologically
/// long run does not panic on overflow. `Option<u64>` fields combine via:
/// both `Some` → sum; one `Some` → keep that value; both `None` → `None`.
/// `extras` keeps the most recent non-null value (later events typically
/// have more complete data; this preserves the latest snapshot).
pub fn accumulate_token_usage(acc: &mut TokenUsage, addend: &TokenUsage) {
    acc.input_tokens = acc.input_tokens.saturating_add(addend.input_tokens);
    acc.output_tokens = acc.output_tokens.saturating_add(addend.output_tokens);
    acc.cache_read_tokens = combine_opt(acc.cache_read_tokens, addend.cache_read_tokens);
    acc.cache_write_tokens = combine_opt(acc.cache_write_tokens, addend.cache_write_tokens);
    acc.reasoning_tokens = combine_opt(acc.reasoning_tokens, addend.reasoning_tokens);
    if !addend.extras.is_null() {
        acc.extras = addend.extras.clone();
    }
}

fn combine_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) | (None, Some(x)) => Some(x),
        (Some(x), Some(y)) => Some(x.saturating_add(y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn token_usage_default_is_zero_with_none_options_and_null_extras() {
        let u = TokenUsage::default();
        assert_eq!(u.input_tokens, 0);
        assert_eq!(u.output_tokens, 0);
        assert!(u.cache_read_tokens.is_none());
        assert!(u.cache_write_tokens.is_none());
        assert!(u.reasoning_tokens.is_none());
        assert!(u.extras.is_null());
    }

    #[test]
    fn accumulate_sums_input_and_output_tokens() {
        let mut acc = TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        };
        let addend = TokenUsage {
            input_tokens: 25,
            output_tokens: 10,
            ..Default::default()
        };
        accumulate_token_usage(&mut acc, &addend);
        assert_eq!(acc.input_tokens, 125);
        assert_eq!(acc.output_tokens, 60);
    }

    #[test]
    fn accumulate_combines_options_per_combine_rule() {
        let mut acc = TokenUsage {
            cache_read_tokens: Some(10),
            cache_write_tokens: None,
            reasoning_tokens: Some(5),
            ..Default::default()
        };
        let addend = TokenUsage {
            cache_read_tokens: Some(20),
            cache_write_tokens: Some(7),
            reasoning_tokens: None,
            ..Default::default()
        };
        accumulate_token_usage(&mut acc, &addend);
        assert_eq!(acc.cache_read_tokens, Some(30)); // both Some → sum
        assert_eq!(acc.cache_write_tokens, Some(7)); // None + Some → Some
        assert_eq!(acc.reasoning_tokens, Some(5)); // Some + None → keep
    }

    #[test]
    fn accumulate_saturates_on_overflow_does_not_panic() {
        let mut acc = TokenUsage {
            input_tokens: u64::MAX - 5,
            ..Default::default()
        };
        let addend = TokenUsage {
            input_tokens: 100,
            ..Default::default()
        };
        accumulate_token_usage(&mut acc, &addend);
        assert_eq!(acc.input_tokens, u64::MAX);
    }

    #[test]
    fn accumulate_extras_keeps_most_recent_non_null() {
        let mut acc = TokenUsage {
            extras: json!({"first": true}),
            ..Default::default()
        };
        let addend = TokenUsage {
            extras: json!({"second": true}),
            ..Default::default()
        };
        accumulate_token_usage(&mut acc, &addend);
        assert_eq!(acc.extras, json!({"second": true}));
    }

    #[test]
    fn accumulate_extras_null_addend_preserves_existing() {
        let mut acc = TokenUsage {
            extras: json!({"first": true}),
            ..Default::default()
        };
        let addend = TokenUsage::default(); // extras is null
        accumulate_token_usage(&mut acc, &addend);
        assert_eq!(acc.extras, json!({"first": true}));
    }

    #[test]
    fn run_log_serializes_omitting_none_fields_and_null_extras() {
        let entry = RunLog {
            goal: "describe X".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: "2026-05-10T03:00:00Z".into(),
            finished_at: "2026-05-10T03:05:00Z".into(),
            tokens: TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            wiki_changed: true,
            lint_error_count: 0,
            lint_warn_count: 1,
            outcome: "succeeded".into(),
            session_id: None,
            interrupt_reason: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        // Required fields present
        assert!(json.contains("\"goal\":\"describe X\""));
        assert!(json.contains("\"mode\":\"goal\""));
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"wiki_changed\":true"));
        assert!(json.contains("\"lint_error_count\":0"));
        // outcome field is required (always serialized; serde default
        // covers deserialize only).
        assert!(json.contains("\"outcome\":\"succeeded\""));
        // None / null-extras fields skipped
        assert!(!json.contains("\"model\""));
        assert!(!json.contains("\"effort\""));
        assert!(!json.contains("\"cache_read_tokens\""));
        assert!(!json.contains("\"extras\""));
    }

    /// Legacy jsonl rows written before v3-run-log-events shipped omit
    /// the `outcome` key. Serde default must restore it to "succeeded"
    /// so existing reader pipelines do not break.
    #[test]
    fn legacy_run_log_without_outcome_deserializes_to_succeeded() {
        let legacy = r#"{
            "goal": "describe X",
            "mode": "goal",
            "started_at": "2026-05-10T00:00:00Z",
            "finished_at": "2026-05-10T00:01:00Z",
            "tokens": {"input_tokens": 10, "output_tokens": 5},
            "wiki_changed": false,
            "lint_error_count": 0,
            "lint_warn_count": 0
        }"#;
        let parsed: RunLog = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.outcome, "succeeded");
        assert_eq!(parsed.goal, "describe X");
        assert_eq!(parsed.mode, "goal");
    }

    /// Three legal outcome values survive a serialize → deserialize
    /// round-trip without translation or normalization.
    #[test]
    fn run_log_outcome_round_trips_for_three_legal_values() {
        for outcome in ["succeeded", "failed", "cancelled"] {
            let entry = RunLog {
                goal: "x".into(),
                mode: "goal".into(),
                model: None,
                effort: None,
                started_at: "2026-05-10T00:00:00Z".into(),
                finished_at: "2026-05-10T00:01:00Z".into(),
                tokens: TokenUsage::default(),
                wiki_changed: false,
                lint_error_count: 0,
                lint_warn_count: 0,
                outcome: outcome.into(),
            session_id: None,
            interrupt_reason: None,
            };
            let json = serde_json::to_string(&entry).unwrap();
            let parsed: RunLog = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.outcome, outcome);
        }
    }

    #[test]
    fn run_log_round_trips_through_serde() {
        let original = RunLog {
            goal: "X".into(),
            mode: "fix".into(),
            model: Some("sonnet".into()),
            effort: Some("medium".into()),
            started_at: "2026-05-10T00:00:00Z".into(),
            finished_at: "2026-05-10T00:01:00Z".into(),
            tokens: TokenUsage {
                input_tokens: 1,
                output_tokens: 2,
                cache_read_tokens: Some(3),
                cache_write_tokens: Some(4),
                reasoning_tokens: Some(5),
                extras: json!({"foo": "bar"}),
            },
            wiki_changed: false,
            lint_error_count: 1,
            lint_warn_count: 2,
            outcome: "failed".into(),
            session_id: None,
            interrupt_reason: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: RunLog = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.goal, original.goal);
        assert_eq!(parsed.mode, original.mode);
        assert_eq!(parsed.model, original.model);
        assert_eq!(parsed.outcome, "failed");
        assert_eq!(parsed.tokens.cache_read_tokens, Some(3));
        assert_eq!(parsed.tokens.extras, json!({"foo": "bar"}));
    }

    /// v3-chat-verb RunLog Schema scenario:
    /// `session_id == None` MUST be omitted from the serialized JSON line
    /// so goal/query/fix rows look identical to pre-chat-verb output.
    #[test]
    fn runlog_session_id_serialize_skip_when_none() {
        let entry = RunLog {
            goal: "describe X".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: "2026-05-13T00:00:00Z".into(),
            finished_at: "2026-05-13T00:00:01Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "succeeded".into(),
            session_id: None,
            interrupt_reason: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(
            !json.contains("\"session_id\""),
            "session_id key MUST be omitted when None; got: {json}"
        );
    }

    /// v3-chat-verb RunLog Schema scenario:
    /// Legacy jsonl rows written before v3-chat-verb shipped have no
    /// `"session_id"` key. Deserialization MUST default the field to `None`
    /// without raising any error.
    #[test]
    fn runlog_session_id_deserialize_legacy_row_as_none() {
        let legacy_line = r#"{
            "goal": "X",
            "mode": "goal",
            "started_at": "2026-05-10T00:00:00Z",
            "finished_at": "2026-05-10T00:01:00Z",
            "tokens": {"input_tokens": 0, "output_tokens": 0},
            "wiki_changed": false,
            "lint_error_count": 0,
            "lint_warn_count": 0,
            "outcome": "succeeded"
        }"#;
        let parsed: RunLog =
            serde_json::from_str(legacy_line).expect("legacy row must deserialize cleanly");
        assert_eq!(parsed.session_id, None);
        assert_eq!(parsed.outcome, "succeeded");
    }

    /// v3-chat-verb RunLog Schema scenario:
    /// When a chat turn writes `session_id == Some(id)`, the serialized
    /// JSON SHALL include the field verbatim so consumers can group
    /// per-turn rows by session.
    #[test]
    fn runlog_session_id_serializes_when_some_for_chat_mode() {
        let entry = RunLog {
            goal: "what does X do?".into(),
            mode: "chat".into(),
            model: None,
            effort: None,
            started_at: "2026-05-13T00:00:00Z".into(),
            finished_at: "2026-05-13T00:00:01Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "succeeded".into(),
            session_id: Some("abc-123".into()),
            interrupt_reason: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"session_id\":\"abc-123\""));
        assert!(json.contains("\"mode\":\"chat\""));
    }

    /// Helper: minimal RunLog used by interrupt_reason tests below — only the
    /// `interrupt_reason` field is varied per case, every other field is fixed.
    fn fixture_run_log_with_reason(reason: Option<InterruptReason>) -> RunLog {
        RunLog {
            goal: "describe X".into(),
            mode: "goal".into(),
            model: None,
            effort: None,
            started_at: "2026-05-27T00:00:00Z".into(),
            finished_at: "2026-05-27T00:00:01Z".into(),
            tokens: TokenUsage::default(),
            wiki_changed: false,
            lint_error_count: 0,
            lint_warn_count: 0,
            outcome: "cancelled".into(),
            session_id: None,
            interrupt_reason: reason,
        }
    }

    /// interrupted-state-formalize scenario:
    /// `RunLog with interrupt_reason UserCancel serializes to kebab-case string literal`.
    /// Each named InterruptReason variant SHALL serialize as the kebab-case
    /// JSON string literal and round-trip back to the same variant.
    #[test]
    fn run_log_interrupt_reason_named_variants_serialize_kebab_case_and_round_trip() {
        let cases = [
            (InterruptReason::AppClose, "\"interrupt_reason\":\"app-close\""),
            (InterruptReason::UserCancel, "\"interrupt_reason\":\"user-cancel\""),
            (
                InterruptReason::NetworkDrop,
                "\"interrupt_reason\":\"network-drop\"",
            ),
        ];
        for (variant, expected_substring) in cases {
            let entry = fixture_run_log_with_reason(Some(variant.clone()));
            let json = serde_json::to_string(&entry).unwrap();
            assert!(
                json.contains(expected_substring),
                "expected {expected_substring:?} in {json}"
            );
            let parsed: RunLog = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.interrupt_reason, Some(variant));
        }
    }

    /// interrupted-state-formalize scenario:
    /// `RunLog with interrupt_reason Other serializes to object form`.
    /// The `Other(String)` newtype variant SHALL serialize as the untagged
    /// object form `{"other": "<string>"}` and round-trip back to the same
    /// value.
    #[test]
    fn run_log_interrupt_reason_other_variant_serializes_as_object_and_round_trips() {
        let entry =
            fixture_run_log_with_reason(Some(InterruptReason::Other("agent-crash".into())));
        let json = serde_json::to_string(&entry).unwrap();
        assert!(
            json.contains("\"interrupt_reason\":{\"other\":\"agent-crash\"}"),
            "expected object-form serialization; got: {json}"
        );
        let parsed: RunLog = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.interrupt_reason,
            Some(InterruptReason::Other("agent-crash".into()))
        );
    }

    /// interrupted-state-formalize scenario:
    /// `interrupt_reason: None` SHALL be omitted from the serialized JSON
    /// line so normal succeeded runs and legacy goal/query/fix rows look
    /// identical to pre-change output.
    #[test]
    fn run_log_interrupt_reason_serialize_skip_when_none() {
        let entry = fixture_run_log_with_reason(None);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(
            !json.contains("\"interrupt_reason\""),
            "interrupt_reason key MUST be omitted when None; got: {json}"
        );
    }

    /// interrupted-state-formalize scenario:
    /// `Legacy jsonl row without interrupt_reason field deserializes cleanly`.
    /// Pre-change jsonl rows lack the `"interrupt_reason"` key and MUST
    /// deserialize to `None` without raising an error.
    #[test]
    fn legacy_run_log_without_interrupt_reason_deserializes_to_none() {
        let legacy_line = r#"{
            "goal": "X",
            "mode": "goal",
            "started_at": "2026-05-10T00:00:00Z",
            "finished_at": "2026-05-10T00:01:00Z",
            "tokens": {"input_tokens": 0, "output_tokens": 0},
            "wiki_changed": false,
            "lint_error_count": 0,
            "lint_warn_count": 0,
            "outcome": "cancelled"
        }"#;
        let parsed: RunLog =
            serde_json::from_str(legacy_line).expect("legacy row must deserialize cleanly");
        assert_eq!(parsed.interrupt_reason, None);
        assert_eq!(parsed.outcome, "cancelled");
    }
}
