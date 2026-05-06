//! Log sink factory. Mirrors [`crate::llm::factory`] / [`crate::pii::factory`]
//! shape — explicit `match` over a [`SinkKind`] enum.

use crate::log::sink::LogSink;
use crate::log::sinks::{jsonl_sink::JsonlSink, null_sink::NullSink};
use std::path::PathBuf;

/// Discriminator for which sink to build. Variants always present so the
/// config layer can map strings to a kind regardless of cargo features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SinkKind {
    /// `null` — no-op, the 0.2.0 behavior-preserving default.
    #[default]
    Null,
    /// `jsonl` — date-rotated `.jsonl` files.
    Jsonl,
    /// `otel` — OpenTelemetry export. Requires `log-otel` feature.
    Otel,
}

#[derive(Debug, Clone, Default)]
pub struct SinkConfig {
    pub kind: SinkKind,
    /// Output directory for [`SinkKind::Jsonl`]. `None` defaults to
    /// `~/.codebus/log` (resolved by the caller — factory itself doesn't
    /// touch the home directory).
    pub jsonl_dir: Option<PathBuf>,
    /// Retention in days. Currently advisory — no rotation logic yet.
    pub retention_days: Option<u32>,
}

#[derive(Debug)]
pub enum SinkError {
    Setup(String),
    FeatureNotCompiled {
        feature: &'static str,
        hint: &'static str,
    },
}

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkError::Setup(msg) => write!(f, "log sink setup failed: {msg}"),
            SinkError::FeatureNotCompiled { feature, hint } => write!(
                f,
                "log sink requires cargo feature `{feature}` (not compiled). {hint}"
            ),
        }
    }
}

impl std::error::Error for SinkError {}

/// Build a sink from a [`SinkConfig`].
pub fn build_sink(cfg: SinkConfig) -> Result<Box<dyn LogSink>, SinkError> {
    match cfg.kind {
        SinkKind::Null => Ok(Box::new(NullSink::new())),
        SinkKind::Jsonl => {
            let dir = cfg
                .jsonl_dir
                .ok_or_else(|| SinkError::Setup("jsonl sink requires `jsonl_dir`".into()))?;
            Ok(Box::new(JsonlSink::new(dir)))
        }
        SinkKind::Otel => Err(SinkError::FeatureNotCompiled {
            feature: "log-otel",
            hint: "rebuild with: cargo install codebus --features log-otel",
        }),
    }
}
