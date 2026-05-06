//! PII scanner factory. Mirrors the `llm::factory` shape — explicit `match`
//! over a [`ScannerKind`] enum, so a reader sees every supported scanner in
//! one place.

use crate::pii::provider::{OnHit, PiiScanner};
use crate::pii::scanners::{null_scanner::NullScanner, regex_basic::RegexBasicScanner};

/// Discriminator for which scanner implementation to build. Variants are
/// always present regardless of cargo features (mirrors [`crate::llm::factory::ProviderKind`]
/// rationale — config layer can map strings to a kind without compile-time
/// branching).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScannerKind {
    /// `null` — no-op scanner, always returns empty matches. The 0.2.0
    /// behavior-preserving default for raw_sync.
    #[default]
    Null,
    /// `regex_basic` — built-in regex pack covering common secrets.
    RegexBasic,
    /// `presidio` — Microsoft Presidio HTTP service. Requires `pii-presidio`
    /// feature (impl lands in a follow-up change).
    Presidio,
    /// `aws` — AWS Comprehend Detect-PII API. Requires `pii-aws` feature
    /// (impl lands in a follow-up change).
    Aws,
}

#[derive(Debug, Clone, Default)]
pub struct ScannerConfig {
    pub kind: ScannerKind,
    pub on_hit: OnHit,
    /// Extra regex patterns for [`ScannerKind::RegexBasic`]. Each entry is a
    /// raw regex source; the scanner labels matches `"custom-<index>"`.
    /// Empty by default. Ignored by other kinds.
    pub patterns_extra: Vec<String>,
}

/// Build a scanner from a [`ScannerConfig`].
///
/// `Null` and `RegexBasic` are always available (no feature gating; deps
/// already in tree). Heavy-dep scanners return an error when their feature
/// isn't compiled.
pub fn build_scanner(cfg: ScannerConfig) -> Result<Box<dyn PiiScanner>, ScannerError> {
    match cfg.kind {
        ScannerKind::Null => Ok(Box::new(NullScanner::new())),
        ScannerKind::RegexBasic => {
            let scanner = RegexBasicScanner::new(&cfg.patterns_extra)
                .map_err(|e| ScannerError::Setup(format!("regex_basic init failed: {e}")))?;
            Ok(Box::new(scanner))
        }
        ScannerKind::Presidio => Err(ScannerError::FeatureNotCompiled {
            feature: "pii-presidio",
            hint: "rebuild with: cargo install codebus --features pii-presidio",
        }),
        ScannerKind::Aws => Err(ScannerError::FeatureNotCompiled {
            feature: "pii-aws",
            hint: "rebuild with: cargo install codebus --features pii-aws",
        }),
    }
}

#[derive(Debug)]
pub enum ScannerError {
    Setup(String),
    FeatureNotCompiled {
        feature: &'static str,
        hint: &'static str,
    },
}

impl std::fmt::Display for ScannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScannerError::Setup(msg) => write!(f, "scanner setup failed: {msg}"),
            ScannerError::FeatureNotCompiled { feature, hint } => write!(
                f,
                "scanner requires cargo feature `{feature}` (not compiled). {hint}"
            ),
        }
    }
}

impl std::error::Error for ScannerError {}
