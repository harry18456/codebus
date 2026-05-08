//! PII scanner trait + shared types.
//!
//! Sync trait by design: regex is pure CPU; HTTP-based scanners (deferred to
//! future changes) wrap their own runtime internally. `Send + Sync` so a
//! single scanner instance can be shared across threads.

/// One PII finding inside a scanned blob. Offsets are **byte offsets** into
/// the input `&str` (consistent with `&str` slicing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiiMatch {
    /// Stable identifier for the rule that matched (e.g. `"aws-access-key"`,
    /// `"anthropic-api-key"`, `"email"`, `"ipv4"`, or a user-supplied label).
    pub pattern_name: String,
    /// Inclusive byte offset of match start.
    pub start: usize,
    /// Exclusive byte offset of match end.
    pub end: usize,
    /// The matched substring. Owned to keep `PiiMatch` `'static`.
    pub matched_text: String,
    /// User-facing severity bucket. `Critical` flags credentials; `Warn`
    /// flags context-dependent PII like emails or IPs.
    pub severity: PiiSeverity,
}

/// Severity bucket. Closed enum on purpose — adding a level is a breaking
/// change that callers must opt into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiSeverity {
    /// Definitely sensitive: secrets, API keys, credentials.
    Critical,
    /// Probably sensitive: emails, IPs — context-dependent.
    Warn,
}

/// Behavior on hit. Default is `Warn` (mirror file + stderr warn each match).
/// `Skip` and `Mask` are reserved for v3-config; v3-pii hardcodes `Warn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnHit {
    /// Surface a warning but include the file in raw_sync as-is.
    #[default]
    Warn,
    /// Skip the file entirely from raw_sync.
    Skip,
    /// Replace the matched substring with `[REDACTED:<pattern_name>]`.
    Mask,
}

/// Object-safe scanner trait. Each impl scans a file's contents for PII.
///
/// `path` is the file's path relative to the scanned root, included so impls
/// can suppress noisy paths (e.g. test fixtures) without changing the global
/// pattern set.
pub trait PiiScanner: Send + Sync {
    /// Stable scanner name (`"null"`, `"regex_basic"`, ...). Distinct from
    /// [`PiiMatch::pattern_name`] (which identifies the rule inside the
    /// scanner).
    fn name(&self) -> &str;

    /// Scan `content` from `path` for PII. Returns matches in ascending byte
    /// offset; empty `Vec` means clean.
    fn scan(&self, content: &str, path: &str) -> Vec<PiiMatch>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_hit_default_is_warn() {
        assert_eq!(OnHit::default(), OnHit::Warn);
    }

    #[test]
    fn pii_severity_critical_and_warn_are_distinct() {
        assert_ne!(PiiSeverity::Critical, PiiSeverity::Warn);
    }
}
