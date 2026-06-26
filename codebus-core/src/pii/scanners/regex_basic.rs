//! Regex-based PII scanner. Detects common secrets and PII via a small,
//! pre-compiled regex pack plus optional user-supplied extras.
//!
//! Pattern selection criteria: high-precision, low-false-positive shapes
//! that cover the most-leaked credential forms seen in dev wikis. Anything
//! that needs context (e.g. variable names, structural cues) is out of
//! scope here — that is reserved for HTTP-based scanners in a future change.
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
/// Patterns are pinned with comments so future edits do not accidentally
/// loosen them.
const BUILTIN_PATTERNS: &[(&str, PiiSeverity, &str)] = &[
    // AWS access key: 4-char prefix + 16 alphanumerics. AKIA = long-lived
    // user keys; ASIA = STS temporary. Other prefixes (AGPA, AIDA, AROA,
    // ANPA, etc.) are deliberately omitted to keep false positives down.
    (
        "aws-access-key",
        PiiSeverity::Critical,
        r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
    ),
    // Anthropic API key: prefix `sk-ant-` followed by 20+ URL-safe chars.
    // Real keys are ~95 chars; the 20-char floor avoids matching stray
    // mentions of the prefix in docs.
    (
        "anthropic-api-key",
        PiiSeverity::Critical,
        r"sk-ant-[A-Za-z0-9_\-]{20,}",
    ),
    // RFC 5322-ish email. Conservative: ASCII local + dot-bearing domain
    // with TLD of 2+ letters. False-negative on `user@host` (no dot in
    // domain) is an accepted trade-off.
    (
        "email",
        PiiSeverity::Warn,
        r"\b[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}\b",
    ),
    // IPv4 dotted quad. Octet range (0-255) is not validated — `999.999.999.999`
    // matches too, but for a Warn-level hint that is acceptable. Word boundary
    // + 4 dotted segments rules out `v1.2.3` (only 3 dots).
    (
        "ipv4",
        PiiSeverity::Warn,
        r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b",
    ),
    // pii-mirror-completeness: high-precision, low-false-positive secret shapes.
    // Each carries a fixed prefix + high-entropy body so a `[REDACTED:...]` hit
    // is overwhelmingly a real credential, not stray prose.
    //
    // GitHub classic personal access token: prefix `ghp_`/`gho_`/`ghu_`/`ghs_`/
    // `ghr_` + 36 base62. The `[pousr]` class enumerates the documented token
    // type letters.
    (
        "github-pat",
        PiiSeverity::Critical,
        r"\bgh[pousr]_[A-Za-z0-9]{36}\b",
    ),
    // GitHub fine-grained PAT: `github_pat_` + 82 chars from [0-9A-Za-z_].
    (
        "github-fine-grained-pat",
        PiiSeverity::Critical,
        r"\bgithub_pat_[0-9A-Za-z_]{82}\b",
    ),
    // Slack token: `xoxb-`/`xoxa-`/`xoxp-`/`xoxr-`/`xoxs-` + hyphen-separated
    // alphanumerics. The 16-char floor avoids matching a bare prefix mention.
    (
        "slack-token",
        PiiSeverity::Critical,
        r"\bxox[baprs]-[0-9A-Za-z-]{16,}",
    ),
    // Google API key: `AIza` + 35 chars from [0-9A-Za-z_-]. No trailing \b
    // because a `-` final char is a non-word boundary edge case.
    (
        "google-api-key",
        PiiSeverity::Critical,
        r"\bAIza[0-9A-Za-z_\-]{35}",
    ),
    // OpenAI key: `sk-proj-...` OR `sk-` + 20+ pure-alnum. The second branch
    // forbids `-`/`_`, so an Anthropic `sk-ant-...` (hyphen after `ant`) fails
    // before reaching 20 chars and is NOT swallowed here — regex crate has no
    // lookaround, so the alternation is how the overlap is avoided.
    (
        "openai-api-key",
        PiiSeverity::Critical,
        r"\bsk-(?:proj-[A-Za-z0-9_\-]{20,}|[A-Za-z0-9]{20,})\b",
    ),
    // Stripe live secret key: `sk_live_` + 24+ base62.
    (
        "stripe-secret-key",
        PiiSeverity::Critical,
        r"\bsk_live_[0-9A-Za-z]{24,}\b",
    ),
    // PEM private-key header. The optional key-type group covers RSA / EC /
    // OPENSSH / DSA / PGP variants as well as the bare header.
    (
        "pem-private-key",
        PiiSeverity::Critical,
        r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----",
    ),
    // JSON Web Token: three base64url segments, first two beginning `eyJ`.
    // Warn (not Critical) because a JWT can be a non-sensitive token.
    (
        "jwt",
        PiiSeverity::Warn,
        r"\beyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+",
    ),
    // Database connection string with an embedded password: scheme `://` then
    // `user:password@`. The `@`-terminated userinfo with a `:` password segment
    // is what distinguishes a credential leak from a host-only URI.
    (
        "db-connection-string",
        PiiSeverity::Critical,
        r"\b(?:postgres(?:ql)?|mysql|mongodb(?:\+srv)?|redis|amqp)://[^:@/\s]+:[^@/\s]+@",
    ),
];

/// Number of built-in patterns the `regex_basic` scanner ships with.
///
/// Single source of truth for the count surfaced in the app's Settings UI
/// (`regex_basic · N patterns`) so the displayed number can never drift from
/// the actual pattern set.
pub fn builtin_pattern_count() -> usize {
    BUILTIN_PATTERNS.len()
}

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
        // `v1.2.3` is 3 dot groups, not 4 → does not match the IPv4 shape.
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
        // A typo in `patterns_extra` errors at construction, not silently
        // drops the rule.
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

    // --- pii-mirror-completeness: expanded builtin pattern coverage ---

    /// Count matches carrying a given `pattern_name`.
    fn count_named(matches: &[PiiMatch], name: &str) -> usize {
        matches.iter().filter(|m| m.pattern_name == name).count()
    }

    #[test]
    fn builtin_pattern_count_is_thirteen() {
        assert_eq!(builtin_pattern_count(), 13);
        assert_eq!(BUILTIN_PATTERNS.len(), 13);
    }

    #[test]
    fn detects_classic_github_pat_positive() {
        let key = format!("ghp_{}", "a".repeat(36));
        let m = scanner().scan(&format!("token = {key}"), "src/ci.rs");
        assert_eq!(count_named(&m, "github-pat"), 1, "got {m:?}");
        let hit = m.iter().find(|x| x.pattern_name == "github-pat").unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn ignores_github_prefix_lookalike_negative() {
        // `ghp_short` has fewer than 36 trailing chars.
        let m = scanner().scan("see ghp_short in docs", "README.md");
        assert_eq!(count_named(&m, "github-pat"), 0, "got {m:?}");
    }

    #[test]
    fn detects_fine_grained_github_pat_positive() {
        let key = format!("github_pat_{}", "A1b2".repeat(20) + "ab"); // 82 chars
        assert_eq!(key.len(), "github_pat_".len() + 82);
        let m = scanner().scan(&format!("PAT={key}"), "src/ci.rs");
        assert_eq!(count_named(&m, "github-fine-grained-pat"), 1, "got {m:?}");
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "github-fine-grained-pat")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn detects_slack_token_positive() {
        // Split the prefix from the body (as the other pattern tests do) so the
        // source never contains a complete `xoxb-` literal that push-protection
        // secret scanners flag as a real Slack token.
        let key = format!("xoxb-{}", "123456789012-1234567890123-abcdEFGHijklMNOPqrstUVWX");
        let m = scanner().scan(&format!("SLACK={key}"), "src/bot.rs");
        assert_eq!(count_named(&m, "slack-token"), 1, "got {m:?}");
        let hit = m.iter().find(|x| x.pattern_name == "slack-token").unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn detects_google_api_key_positive() {
        let key = format!("AIza{}", "Bc3_-d".repeat(6).chars().take(35).collect::<String>());
        let m = scanner().scan(&format!("GOOGLE_KEY={key}"), "src/maps.rs");
        assert_eq!(count_named(&m, "google-api-key"), 1, "got {m:?}");
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "google-api-key")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn detects_openai_api_key_positive() {
        let key = format!("sk-{}", "a1B2c3D4".repeat(6)); // 48 alnum
        let m = scanner().scan(&format!("OPENAI_API_KEY={key}"), "src/llm.rs");
        assert_eq!(count_named(&m, "openai-api-key"), 1, "got {m:?}");
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "openai-api-key")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn openai_pattern_does_not_match_anthropic_key_negative() {
        // The OpenAI alternation must NOT swallow an `sk-ant-` Anthropic key.
        let key = "sk-ant-api01-abcDEF123456789_-XYZ012345";
        let m = scanner().scan(&format!("api_key=\"{key}\""), "src/llm.rs");
        assert_eq!(count_named(&m, "anthropic-api-key"), 1, "got {m:?}");
        assert_eq!(count_named(&m, "openai-api-key"), 0, "got {m:?}");
    }

    #[test]
    fn detects_stripe_secret_key_positive() {
        let key = format!("sk_live_{}", "Ab3Cd9Ef".repeat(3)); // 24 base62
        let m = scanner().scan(&format!("STRIPE={key}"), "src/pay.rs");
        assert_eq!(count_named(&m, "stripe-secret-key"), 1, "got {m:?}");
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "stripe-secret-key")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn detects_pem_private_key_header_positive() {
        let m = scanner().scan("-----BEGIN OPENSSH PRIVATE KEY-----", "id_ed25519");
        assert_eq!(count_named(&m, "pem-private-key"), 1, "got {m:?}");
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "pem-private-key")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn detects_jwt_positive() {
        let key = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let m = scanner().scan(&format!("Authorization: Bearer {key}"), "src/auth.rs");
        assert_eq!(count_named(&m, "jwt"), 1, "got {m:?}");
        let hit = m.iter().find(|x| x.pattern_name == "jwt").unwrap();
        assert_eq!(hit.severity, PiiSeverity::Warn);
    }

    #[test]
    fn detects_db_connection_string_positive() {
        let m = scanner().scan(
            "DATABASE_URL=postgres://dbuser:s3cr3tPassw0rd@db.internal:5432/app",
            "src/db.rs",
        );
        assert!(
            count_named(&m, "db-connection-string") >= 1,
            "expected a db-connection-string hit, got {m:?}"
        );
        let hit = m
            .iter()
            .find(|x| x.pattern_name == "db-connection-string")
            .unwrap();
        assert_eq!(hit.severity, PiiSeverity::Critical);
    }

    #[test]
    fn ignores_db_uri_without_password_negative() {
        // Host-only URI with no `user:password@` userinfo → no credential leak.
        let m = scanner().scan("postgres://db.internal:5432/app", "src/db.rs");
        assert_eq!(count_named(&m, "db-connection-string"), 0, "got {m:?}");
    }
}
