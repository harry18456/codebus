//! [`LogSink`] trait + shared run-log shape.
//!
//! Sync trait by design (decision §"Trait sync/async"): file writes wrap
//! `BufWriter`; OTel SDKs do their own batching; nothing inside requires
//! `await`. Object-safe so a `Box<dyn LogSink>` can be swapped via
//! [`super::factory::build_sink`].
//!
//! Wired into `goal` / `query` flows by the `token-tracking` change. The
//! default [`super::sinks::null_sink::NullSink`] silently discards writes,
//! preserving 0.2.0 user-visible behavior; users opt into persistence by
//! configuring `log: { sink: jsonl }` in `~/.codebus/config.yaml`.

use serde::{Deserialize, Serialize};

/// Token usage for one LLM invocation, normalized across providers.
///
/// `input_tokens` and `output_tokens` are universal — every LLM API
/// exposes them. The remaining fields are `Option<u64>` because not every
/// provider has them (e.g. OpenAI legacy / Ollama have no cache concept).
/// `None` means "the provider does not have this concept"; `Some(0)`
/// means "the concept exists but no tokens were attributed this run".
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
    /// Reasoning tokens for o-series models / extended-thinking-style
    /// providers that bill reasoning separately from output. `None` for
    /// providers without this concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Vendor-specific raw JSON. Providers SHOULD set this to their
    /// original `usage` object so high-fidelity post-hoc analysis is
    /// possible. Default is `Value::Null`, which is skipped during
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
    /// The goal / query text that triggered the run.
    pub goal: String,
    /// `"goal"` or `"query"`. Required because runs.jsonl mixes both
    /// modes; consumers filter on this field.
    pub mode: String,
    /// Model alias / id passed to the provider for this run, if any.
    /// `None` when the user didn't configure a model (provider used its
    /// default).
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

/// Add `addend` into `acc` field-by-field. `input_tokens` and
/// `output_tokens` use `saturating_add` so a pathologically long-running
/// session doesn't panic on overflow. `Option<u64>` fields combine via:
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

/// Object-safe sink for run summaries. Sinks may buffer internally; callers
/// should call [`flush`](Self::flush) before process exit.
pub trait LogSink: Send + Sync {
    /// Stable name (`"null"`, `"jsonl"`, `"otel"`).
    fn name(&self) -> &str;

    /// Persist one [`RunLog`] entry.
    fn write_run(&mut self, entry: &RunLog) -> Result<(), LogError>;

    /// Persist incremental token usage (e.g. mid-run streaming counts).
    /// Default implementation is a no-op so impls that only care about
    /// final per-run totals don't need to override.
    fn write_token_usage(&mut self, _usage: &TokenUsage) -> Result<(), LogError> {
        Ok(())
    }

    /// Flush any internal buffers. Default no-op.
    fn flush(&mut self) -> Result<(), LogError> {
        Ok(())
    }
}
