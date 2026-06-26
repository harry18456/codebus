//! PII (Personally Identifiable Information) scanner domain.
//!
//! Day-one wiring lands three pieces:
//!
//! - [`provider::PiiScanner`] trait + [`provider::PiiMatch`] / [`provider::PiiSeverity`] / [`provider::OnHit`]
//! - [`scanners::null_scanner::NullScanner`] — no-op, primarily a test fixture and trait second-impl
//! - [`scanners::regex_basic::RegexBasicScanner`] — built-in regex pattern pack ([`builtin_pattern_count`] entries)
//!
//! `raw_sync` invokes the scanner via `&dyn PiiScanner`; the caller picks
//! which scanner to construct. v3-pii hardcodes `RegexBasicScanner::new(&[])`
//! at the init call site; config-driven selection (`patterns_extra` / `on_hit`
//! override) is deferred to v3-config.

pub mod provider;
pub mod scanners;

pub use provider::{OnHit, PiiMatch, PiiScanner, PiiSeverity};
pub use scanners::regex_basic::builtin_pattern_count;
