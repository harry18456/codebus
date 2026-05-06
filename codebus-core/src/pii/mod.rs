//! PII (Personally Identifiable Information) scanner plugin domain.
//!
//! Day-one wiring lands four pieces:
//!
//! - [`provider::PiiScanner`] trait + [`provider::PiiMatch`] / [`provider::PiiSeverity`]
//! - [`factory::build_scanner`] for explicit `ScannerKind` → `Box<dyn PiiScanner>`
//! - [`scanners::null_scanner::NullScanner`] — default, behavior-neutral with 0.2.0
//! - [`scanners::regex_basic::RegexBasicScanner`] — built-in pattern pack, always available
//!
//! `raw_sync` does NOT call into a scanner yet — plumbing scanner output
//! into the sync pipeline is a follow-up change.

pub mod factory;
pub mod provider;
pub mod scanners;

pub use factory::{ScannerConfig, ScannerError, ScannerKind, build_scanner};
pub use provider::{OnHit, PiiMatch, PiiScanner, PiiSeverity};
