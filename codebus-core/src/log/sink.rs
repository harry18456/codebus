//! [`LogSink`] trait + shared run-log shape.
//!
//! Sync trait by design (decision §"Trait sync/async"): file writes wrap
//! `BufWriter`; OTel SDKs do their own batching; nothing inside requires
//! `await`. Object-safe so a `Box<dyn LogSink>` can be swapped via
//! [`super::factory::build_sink`].
//!
//! `LogSink` is **not wired into `goal` / `query` flows yet** — Phase 1
//! ships the trait + impls so the contract exists, but the default
//! [`super::sinks::null_sink::NullSink`] keeps user-visible behavior
//! identical to 0.2.0. Plumbing the sink to actually receive run summaries
//! is a follow-up change (#4 token tracking).

use serde::{Deserialize, Serialize};

/// Token usage for one LLM invocation. Fields default to `0` when the
/// provider didn't report a number (e.g. local / mock providers).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
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
