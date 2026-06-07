//! `verb::quiz_validate` — deterministic quiz-body validator (design D2/D5).
//!
//! Authority for the structural correctness of a generated quiz markdown
//! body and for the existence of its `[[slug]]` explanation citations. The
//! codebus-quiz SKILL references this validator rather than restating its
//! rules (design D5, no schema double-delivery). Findings reuse the
//! existing `wiki::types::LintIssue` shape so the CLI sub-action, the
//! library final-verify, and the event pipeline surface them uniformly
//! (design D3).
//!
//! Two finding classes, all severity `Error`:
//!   - schema: each `## Q<n>.` block MUST have a non-empty stem, exactly
//!     the four choice keys `A`/`B`/`C`/`D`, an `## Answer: X` with
//!     `X ∈ {A,B,C,D}`, and an `## Explanation:` line. Blank lines and
//!     surrounding whitespace are tolerated (parity with the tolerant
//!     frontend `quiz-parse.ts`, but enforced here rather than silently
//!     skipped).
//!   - wikilink-existence: every `[[slug]]` cited in an `## Explanation`
//!     MUST resolve to a page slug present in the vault wiki catalog
//!     (`wiki::lint` `VaultContext` / `Catalog`).

use std::path::Path;

use regex::Regex;

use crate::wiki::lint::VaultContext;
use crate::wiki::types::{LintIssue, LintSeverity};

/// Validate a fence/preamble-stripped quiz markdown body against the
/// schema rules and the vault wiki catalog at `wiki_root`. Returns one
/// `LintIssue` per violation (severity `Error`); an empty vec means the
/// body is structurally sound and every citation resolves. `path` on
/// each issue is the question identifier (`Q<n>`); callers may prefix
/// the source file for display.
///
/// `expected_count`: when `Some(n)`, the validator additionally emits one
/// body-level `quiz-question-count` finding (`path = "quiz"`) if the number
/// of `## Q<n>.` blocks differs from `n`. When `None`, the question count is
/// not checked — count-unaware callers (e.g. `codebus quiz validate` without
/// `--count`) keep their prior behavior.
const CHOICE_KEYS: [char; 4] = ['A', 'B', 'C', 'D'];

fn err(qnum: &str, rule_id: &str, message: String) -> LintIssue {
    LintIssue {
        path: format!("Q{qnum}"),
        severity: LintSeverity::Error,
        rule_id: rule_id.to_string(),
        message,
    }
}

pub fn validate_quiz_body(
    quiz_md: &str,
    wiki_root: &Path,
    expected_count: Option<u8>,
) -> Vec<LintIssue> {
    // Wiki slug catalog is the existence authority (reuse wiki::lint).
    let page_slugs = VaultContext::build(wiki_root).catalog.page_slugs;

    // Question header: `## Q<n>.` (mirrors the frontend quiz-parse split).
    let header = Regex::new(r"(?m)^##[ \t]+Q(\d+)\.[ \t]*").unwrap();
    let choice_re = Regex::new(r"(?m)^[ \t]*-[ \t]*([A-D])\)[ \t]*(.+)$").unwrap();
    let answer_re = Regex::new(r"(?m)^##[ \t]+Answer:[ \t]*([A-D])\b").unwrap();
    let expl_re = Regex::new(r"(?m)^##[ \t]+Explanation:[ \t]*(.*)$").unwrap();
    let link_re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();

    let mut issues = Vec::new();

    // Slice each question block: from one header to the next (or EOF).
    let headers: Vec<_> = header.captures_iter(quiz_md).collect();
    for (idx, cap) in headers.iter().enumerate() {
        let m = cap.get(0).unwrap();
        let qnum = cap.get(1).unwrap().as_str();
        let block_start = m.end();
        let block_end = headers
            .get(idx + 1)
            .map(|n| n.get(0).unwrap().start())
            .unwrap_or(quiz_md.len());
        let block = &quiz_md[block_start..block_end];

        // Stem = remainder of the header line (up to first newline).
        let stem = block.split('\n').next().unwrap_or("").trim();
        if stem.is_empty() {
            issues.push(err(
                qnum,
                "quiz-schema-stem",
                "question stem is empty".to_string(),
            ));
        }

        let mut seen: Vec<char> = Vec::new();
        for c in choice_re.captures_iter(block) {
            if let Some(k) = c.get(1).and_then(|g| g.as_str().chars().next()) {
                if !seen.contains(&k) {
                    seen.push(k);
                }
            }
        }
        let have_all = CHOICE_KEYS.iter().all(|k| seen.contains(k));
        if !have_all || seen.len() != 4 {
            issues.push(err(
                qnum,
                "quiz-schema-choices",
                format!(
                    "expected exactly the four choices A/B/C/D, found [{}]",
                    seen.iter().collect::<String>()
                ),
            ));
        }

        if answer_re.captures(block).is_none() {
            issues.push(err(
                qnum,
                "quiz-schema-answer",
                "missing or non-A/B/C/D `## Answer:` line".to_string(),
            ));
        }

        match expl_re.captures(block) {
            None => issues.push(err(
                qnum,
                "quiz-schema-explanation",
                "missing `## Explanation:` line".to_string(),
            )),
            Some(e) => {
                let expl = e.get(1).map(|g| g.as_str()).unwrap_or("");
                for l in link_re.captures_iter(expl) {
                    let slug = l.get(1).unwrap().as_str().trim();
                    if !slug.is_empty() && !page_slugs.contains(slug) {
                        issues.push(err(
                            qnum,
                            "quiz-broken-wikilink",
                            format!(
                                "explanation cites [[{slug}]] but no page \
                                 named {slug}.md exists in any wiki/<type>/ folder"
                            ),
                        ));
                    }
                }
            }
        }
    }

    // Question-count finding (body-level): only when the caller supplies an
    // expected count. `None` (e.g. `codebus quiz validate` without `--count`)
    // skips the check entirely so count-unaware callers are unaffected. The
    // GUI/CLI generate path passes the run's requested count so the agent
    // self-repair loop and the final-verify marker both react to a count drift.
    if let Some(expected) = expected_count {
        let actual = headers.len();
        if actual != expected as usize {
            issues.push(LintIssue {
                path: "quiz".to_string(),
                severity: LintSeverity::Error,
                rule_id: "quiz-question-count".to_string(),
                message: format!("expected {expected} question(s), found {actual}"),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn err(issues: &[LintIssue], rule_id: &str) -> usize {
        issues
            .iter()
            .filter(|i| i.rule_id == rule_id && i.severity == LintSeverity::Error)
            .count()
    }

    /// A wiki root with `concepts/<slug>.md` for each given slug, so the
    /// catalog resolves those citations.
    fn wiki_with(slugs: &[&str]) -> tempfile::TempDir {
        let d = tempfile::TempDir::new().unwrap();
        let concepts = d.path().join("concepts");
        fs::create_dir_all(&concepts).unwrap();
        fs::write(d.path().join("index.md"), "# index\n").unwrap();
        for s in slugs {
            fs::write(concepts.join(format!("{s}.md")), "# p\n").unwrap();
        }
        d
    }

    const GOOD_Q: &str = "## Q1. What does AuthMiddleware return on an expired token?\n\n\
- A) 200\n- B) 301\n- C) 401\n- D) 500\n\n## Answer: C\n\n## Explanation: expired tokens 401.";

    #[test]
    fn missing_answer_yields_schema_answer_finding() {
        let w = wiki_with(&[]);
        let md = "## Q1. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n## Explanation: e";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-schema-answer"), 1);
    }

    #[test]
    fn fewer_than_four_choices_yields_choices_finding() {
        let w = wiki_with(&[]);
        let md = "## Q1. stem?\n- A) a\n- B) b\n- C) c\n## Answer: A\n## Explanation: e";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-schema-choices"), 1);
    }

    #[test]
    fn empty_stem_yields_stem_finding() {
        let w = wiki_with(&[]);
        let md = "## Q1. \n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A\n## Explanation: e";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-schema-stem"), 1);
    }

    #[test]
    fn missing_explanation_yields_explanation_finding() {
        let w = wiki_with(&[]);
        let md = "## Q1. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n## Answer: A";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-schema-explanation"), 1);
    }

    #[test]
    fn well_formed_with_blank_lines_yields_no_findings() {
        let w = wiki_with(&[]);
        let issues = validate_quiz_body(GOOD_Q, w.path(), None);
        assert!(issues.is_empty(), "expected no findings, got {issues:?}");
    }

    #[test]
    fn explanation_broken_wikilink_yields_finding() {
        let w = wiki_with(&[]);
        let md = "## Q1. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n\
## Answer: A\n## Explanation: see [[no-such-page]] for detail";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-broken-wikilink"), 1);
    }

    #[test]
    fn explanation_existing_wikilink_no_finding() {
        let w = wiki_with(&["jwt-pitfalls"]);
        let md = "## Q1. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n\
## Answer: A\n## Explanation: see [[jwt-pitfalls]] for detail";
        let issues = validate_quiz_body(md, w.path(), None);
        assert_eq!(err(&issues, "quiz-broken-wikilink"), 0);
    }

    /// Build a body of `n` structurally-valid, citation-free questions so
    /// the only finding under test is the question-count rule.
    fn n_questions(n: usize) -> String {
        (1..=n)
            .map(|i| {
                format!(
                    "## Q{i}. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n\
## Answer: A\n## Explanation: ok"
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    #[test]
    fn expected_count_mismatch_yields_count_finding() {
        // Spec example: nine blocks, expected five → one count finding.
        let w = wiki_with(&[]);
        let issues = validate_quiz_body(&n_questions(9), w.path(), Some(5));
        assert_eq!(err(&issues, "quiz-question-count"), 1);
        let f = issues
            .iter()
            .find(|i| i.rule_id == "quiz-question-count")
            .unwrap();
        assert_eq!(f.path, "quiz", "count finding is body-level, not per-Q");
        assert!(
            f.message.contains('5') && f.message.contains('9'),
            "message must state expected and actual; got `{}`",
            f.message
        );
    }

    #[test]
    fn matching_expected_count_yields_no_count_finding() {
        let w = wiki_with(&[]);
        let issues = validate_quiz_body(&n_questions(5), w.path(), Some(5));
        assert_eq!(err(&issues, "quiz-question-count"), 0);
    }

    #[test]
    fn no_expected_count_skips_count_check() {
        // None → count is not checked even when the body has nine blocks.
        let w = wiki_with(&[]);
        let issues = validate_quiz_body(&n_questions(9), w.path(), None);
        assert_eq!(err(&issues, "quiz-question-count"), 0);
    }
}
