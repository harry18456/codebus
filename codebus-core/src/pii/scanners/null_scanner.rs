//! No-op scanner. Always returns an empty match list. Used primarily as a
//! `PiiScanner` trait second impl and as a test fixture inside `raw_sync`.

use crate::pii::provider::{PiiMatch, PiiScanner};

#[derive(Debug, Default, Clone, Copy)]
pub struct NullScanner;

impl NullScanner {
    pub fn new() -> Self {
        Self
    }
}

impl PiiScanner for NullScanner {
    fn name(&self) -> &str {
        "null"
    }

    fn scan(&self, _content: &str, _path: &str) -> Vec<PiiMatch> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_scanner_returns_empty_for_clean_input() {
        let s = NullScanner::new();
        assert!(s.scan("hello world", "any.txt").is_empty());
    }

    #[test]
    fn null_scanner_returns_empty_even_when_input_contains_secret_lookalike() {
        // Defensive contract pin: even when the input contains text that
        // OTHER scanners would flag (an AWS access key shape here), the null
        // scanner MUST stay silent.
        let s = NullScanner::new();
        let secret_lookalike = "key=AKIAIOSFODNN7EXAMPLE";
        assert!(s.scan(secret_lookalike, "src/aws.py").is_empty());
    }

    #[test]
    fn null_scanner_returns_empty_for_empty_input() {
        let s = NullScanner::new();
        assert!(s.scan("", "any.txt").is_empty());
    }

    #[test]
    fn null_scanner_name_is_stable() {
        assert_eq!(NullScanner::new().name(), "null");
    }

    #[test]
    fn null_scanner_is_object_safe() {
        let _: Box<dyn PiiScanner> = Box::new(NullScanner::new());
    }
}
