//! Concrete `PiiScanner` implementations. `null_scanner` is the day-one
//! default; `regex_basic` is the always-available built-in pattern pack.
//! Heavy-dep scanners (`presidio`, `aws`) land here behind cargo feature
//! gates in follow-up changes.
//!
//! Selection happens in [`crate::pii::factory::build_scanner`]; callers
//! never reference these submodules directly.

pub mod null_scanner;
pub mod regex_basic;
