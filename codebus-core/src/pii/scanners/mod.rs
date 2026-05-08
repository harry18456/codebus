//! Concrete `PiiScanner` implementations.
//!
//! - `null_scanner`: no-op; used as a trait second impl and as a test fixture
//!   when raw_sync tests want a scanner that returns zero matches.
//! - `regex_basic`: the always-available built-in 4-pattern pack
//!   (aws-access-key, anthropic-api-key, email, ipv4).
//!
//! Heavier scanners (Presidio, AWS Comprehend, etc.) are deferred until a
//! second real impl arrives via a future change; v3-pii does not carry the
//! v2 factory dispatch / tagged-enum config layer.

pub mod null_scanner;
pub mod regex_basic;
