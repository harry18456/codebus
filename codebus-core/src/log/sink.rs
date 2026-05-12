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
    /// `fix` (which has no positional argument).
    pub goal: String,
    /// `"goal"` / `"query"` / `"fix"`. Required because runs.jsonl mixes
    /// modes; consumers filter on this field.
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
        };
        let json = serde_json::to_string(&entry).unwrap();
        // Required fields present
        assert!(json.contains("\"goal\":\"describe X\""));
        assert!(json.contains("\"mode\":\"goal\""));
        assert!(json.contains("\"input_tokens\":100"));
        assert!(json.contains("\"wiki_changed\":true"));
        assert!(json.contains("\"lint_error_count\":0"));
        // None / null-extras fields skipped
        assert!(!json.contains("\"model\""));
        assert!(!json.contains("\"effort\""));
        assert!(!json.contains("\"cache_read_tokens\""));
        assert!(!json.contains("\"extras\""));
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
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: RunLog = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.goal, original.goal);
        assert_eq!(parsed.mode, original.mode);
        assert_eq!(parsed.model, original.model);
        assert_eq!(parsed.tokens.cache_read_tokens, Some(3));
        assert_eq!(parsed.tokens.extras, json!({"foo": "bar"}));
    }
}
