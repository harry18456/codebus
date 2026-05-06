//! Regex-based PII scanner. Detects common secrets and PII via a small,
//! pre-compiled regex pack plus user-supplied extras.
//!
//! Pattern selection criteria: high-precision, low-false-positive patterns
//! that cover the most-leaked credential shapes seen in dev wikis. Anything
//! that needs context (e.g. variable names, structural cues) is out of
//! scope here — that's `presidio` / future ML scanners' job.
//!
//! Audit lens (sharp edges):
//!   - Regex compilation happens once at construction (`new`), not per scan.
//!   - The `regex` crate is RE2-style, no catastrophic backtracking.
//!   - User-supplied `patterns_extra` is rejected at construction if any
//!     entry fails to compile — fail fast, do not silently drop.

use crate::pii::provider::{PiiMatch, PiiScanner, PiiSeverity};
use regex::Regex;

/// Built-in pattern set. Each entry is `(label, severity, source)`.
///
/// Patterns are pinned with comments so future edits don't accidentally
/// loosen them.
const BUILTIN_PATTERNS: &[(&str, PiiSeverity, &str)] = &[
    // AWS access key: 4-char prefix + 16 alphanumerics. AKIA = long-lived
    // user keys; ASIA = STS temporary; AGPA, AIDA, AROA, ANPA, etc., are
    // service principals — stick to the two most-common (AKIA, ASIA) to
    // keep false-positive rate low.
    (
        "aws-access-key",
        PiiSeverity::Critical,
        r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
    ),
    // Anthropic API key: prefix `sk-ant-` then arbitrary URL-safe chars.
    // Real keys are ~95 chars; we require at least 20 to dodge stray
    // mentions of the prefix in docs.
    (
        "anthropic-api-key",
        PiiSeverity::Critical,
        r"sk-ant-[A-Za-z0-9_\-]{20,}",
    ),
    // RFC 5322-ish email. Conservative: ASCII local + domain with at
    // least one dot, label-by-label. False-positive on `user@host` (no
    // dot) is acceptable.
    (
        "email",
        PiiSeverity::Warn,
        r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
    ),
    // IPv4 dotted quad. We don't validate octet range (0-255) — the
    // pattern flags `999.999.999.999` too, but that's fine for a "PII
    // hint" warning. Caller can post-filter if precision matters.
    (
        "ipv4",
        PiiSeverity::Warn,
        r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b",
    ),
];

pub struct RegexBasicScanner {
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    label: String,
    severity: PiiSeverity,
    re: Regex,
}

impl RegexBasicScanner {
    /// Construct with the built-in pattern set plus optional `patterns_extra`.
    /// Each extra pattern compiles eagerly; the first compile failure is
    /// returned so users see typos immediately rather than at scan time.
    pub fn new(patterns_extra: &[String]) -> Result<Self, regex::Error> {
        let mut rules: Vec<CompiledRule> = BUILTIN_PATTERNS
            .iter()
            .map(|(label, severity, src)| {
                Ok::<_, regex::Error>(CompiledRule {
                    label: (*label).to_string(),
                    severity: *severity,
                    re: Regex::new(src)?,
                })
            })
            .collect::<Result<_, _>>()?;

        for (idx, src) in patterns_extra.iter().enumerate() {
            rules.push(CompiledRule {
                label: format!("custom-{idx}"),
                severity: PiiSeverity::Critical,
                re: Regex::new(src)?,
            });
        }

        Ok(Self { rules })
    }
}

impl PiiScanner for RegexBasicScanner {
    fn name(&self) -> &str {
        "regex_basic"
    }

    fn scan(&self, content: &str, _path: &str) -> Vec<PiiMatch> {
        let mut matches: Vec<PiiMatch> = Vec::new();
        for rule in &self.rules {
            for m in rule.re.find_iter(content) {
                matches.push(PiiMatch {
                    pattern_name: rule.label.clone(),
                    start: m.start(),
                    end: m.end(),
                    matched_text: m.as_str().to_string(),
                    severity: rule.severity,
                });
            }
        }
        matches.sort_by_key(|m| m.start);
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanner() -> RegexBasicScanner {
        RegexBasicScanner::new(&[]).expect("builtin patterns must compile")
    }

    #[test]
    fn detects_aws_access_key_positive() {
        let m = scanner().scan("AWS_KEY=AKIAIOSFODNN7EXAMPLE in env", "src/aws.py");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].pattern_name, "aws-access-key");
        assert_eq!(m[0].matched_text, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(m[0].severity, PiiSeverity::Critical);
    }

    #[test]
    fn ignores_aws_lookalike_negative() {
        // 15 trailing alnums, not 16 → must NOT trigger.
        let m = scanner().scan("AKIA12345ABCDEFGH ", "src/x.py");
        assert!(m.is_empty(), "expected no match, got {m:?}");
    }

    #[test]
    fn detects_anthropic_api_key_positive() {
        let key = "sk-ant-api01-abcDEF123456789_-XYZ012345";
        let line = format!("client = Anthropic(api_key=\"{key}\")");
        let m = scanner().scan(&line, "src/llm.py");
        assert_eq!(m.len(), 1, "expected 1 match, got {m:?}");
        assert_eq!(m[0].pattern_name, "anthropic-api-key");
        assert_eq!(m[0].severity, PiiSeverity::Critical);
    }

    #[test]
    fn ignores_short_anthropic_prefix_negative() {
        // `sk-ant-` mentioned without a real key body — too short to match.
        let m = scanner().scan("Set ANTHROPIC_API_KEY=sk-ant-... in env", "README.md");
        assert!(m.is_empty(), "expected no match, got {m:?}");
    }

    #[test]
    fn detects_email_positive() {
        let m = scanner().scan("contact alice@example.com please", "docs/contact.md");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].pattern_name, "email");
        assert_eq!(m[0].matched_text, "alice@example.com");
        assert_eq!(m[0].severity, PiiSeverity::Warn);
    }

    #[test]
    fn ignores_email_lookalike_no_tld_negative() {
        // No dot in domain → not RFC 5322-ish, our conservative rule skips.
        let m = scanner().scan("user@localhost is the dev box", "README.md");
        assert!(m.is_empty(), "expected no match, got {m:?}");
    }

    #[test]
    fn detects_ipv4_positive() {
        let m = scanner().scan("server at 192.168.1.42 is up", "docs/net.md");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].pattern_name, "ipv4");
        assert_eq!(m[0].matched_text, "192.168.1.42");
    }

    #[test]
    fn ignores_version_string_lookalike_negative() {
        // `v1.2.3` is not an IP. Word-boundary + 3 dots requirement keeps
        // us from over-matching here.
        let m = scanner().scan("upgraded to v1.2.3 today", "CHANGELOG.md");
        assert!(m.is_empty(), "expected no match, got {m:?}");
    }

    #[test]
    fn custom_pattern_triggers_via_patterns_extra() {
        let extras = vec![r"\bINTERNAL-\d{6}\b".to_string()];
        let s = RegexBasicScanner::new(&extras).expect("custom pattern must compile");
        let m = s.scan("ticket INTERNAL-123456 is closed", "notes.md");
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].pattern_name, "custom-0");
        assert_eq!(m[0].matched_text, "INTERNAL-123456");
        assert_eq!(m[0].severity, PiiSeverity::Critical);
    }

    #[test]
    fn malformed_custom_pattern_fails_fast_at_construction() {
        // Sharp-edge / Confused-developer lens: a typo in `patterns_extra`
        // should error at construction, not silently drop the rule.
        let bad = vec!["[unterminated".to_string()];
        let r = RegexBasicScanner::new(&bad);
        assert!(r.is_err(), "malformed regex must reject at construction");
    }

    #[test]
    fn matches_returned_in_ascending_offset_order() {
        let line = "alice@a.com and 10.0.0.1 and bob@b.com";
        let m = scanner().scan(line, "docs.md");
        assert!(
            m.windows(2).all(|w| w[0].start <= w[1].start),
            "matches not sorted: {m:?}"
        );
    }

    #[test]
    fn scanner_name_is_stable() {
        assert_eq!(scanner().name(), "regex_basic");
    }

    #[test]
    fn scanner_is_object_safe() {
        let _: Box<dyn PiiScanner> = Box::new(scanner());
    }
}
