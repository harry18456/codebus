//! `verb::content_verify` — shared independent-model content
//! verification + bounded repair orchestration (goal-content-verify
//! design D1).
//!
//! Extracted, behavior-preserving, from the inline quiz implementation so
//! both `run_quiz_generate` and `run_goal` consume one orchestrator. The
//! caller injects two closures — a `verify` step (run an independent
//! read-only spawn, parse its output) and a `repair` step (run a
//! repair spawn fed the defects) — and this module owns the bounded
//! loop: verify → (on defects) repair → re-verify, hard-capped at
//! [`CONTENT_VERIFY_CAP`], best-effort (a verify failure, unparseable
//! output, or repair failure is non-fatal and conservatively flagged —
//! never silently `ok`).
//!
//! The verify-output contract is the line grammar
//! `<id> | <defect-type> | <suggestion>`, or the single token
//! `CONTENT_OK` when no item has a defect. `<id>` is the quiz question
//! token (`Q3`) or the goal wiki page path (`wiki/concepts/x.md`); this
//! module keeps it an opaque `String` and lets each caller interpret it.

use crate::verb::error::VerbError;

/// Hard cap on verify→repair→re-verify iterations (design D1/D4, mirrors
/// the pre-extraction quiz cap of 3).
pub const CONTENT_VERIFY_CAP: u8 = 3;

/// One content defect parsed from a verify spawn line
/// `<id> | <defect-type> | <suggestion>`. `id` is opaque to this module
/// (quiz: `Q<n>`; goal: a `wiki/`-relative page path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDefect {
    pub id: String,
    pub kind: String,
    pub suggestion: String,
}

/// Parse a verify spawn's accumulated text. Returns `Some(vec![])` when
/// the spawn reported `CONTENT_OK`, `Some(defects)` when it emitted
/// `<id> | <type> | <suggestion>` lines, and `None` when neither is
/// present (unparseable → caller treats as a conservative non-fatal
/// flag). Semantics are identical to the pre-extraction quiz parser; the
/// only generalization is that `<id>` is an opaque string rather than a
/// `Q`-prefixed integer.
pub fn parse_content_defects(text: &str) -> Option<Vec<ContentDefect>> {
    let mut defects = Vec::new();
    let mut saw_ok = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line == "CONTENT_OK" {
            saw_ok = true;
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').map(|s| s.trim()).collect();
        if parts.len() != 3 {
            continue;
        }
        if parts[0].is_empty() || parts[1].is_empty() {
            continue;
        }
        defects.push(ContentDefect {
            id: parts[0].to_string(),
            kind: parts[1].to_string(),
            suggestion: parts[2].to_string(),
        });
    }
    if !defects.is_empty() {
        return Some(defects);
    }
    if saw_ok {
        return Some(Vec::new());
    }
    // Tolerance for the F38/F78 STOP-boundary best-effort prompt contract:
    // the agent CLI occasionally delivers a brief rationale and the
    // `CONTENT_OK` marker as separate text content blocks (separate Thought
    // events) that the verb-side accumulator concatenates without inserting
    // a newline (the accumulator MUST stay newline-free so quiz generate
    // bodies split across multiple text chunks reassemble correctly). When
    // no defect lines were found, accept a stand-alone `CONTENT_OK` token
    // anywhere in the text — surrounding chars must be non-alphanumeric /
    // non-underscore so a literal `CONTENT_OK` inside a longer identifier
    // (e.g., `MY_CONTENT_OK_FLAG`) does not falsely OK an unparseable run.
    if contains_content_ok_token(text) {
        return Some(Vec::new());
    }
    None
}

/// Whether `text` contains the literal `CONTENT_OK` marker as a standalone
/// token (its surrounding chars are not alphanumeric / not `_`). Used by
/// the tolerant parser fallback above.
fn contains_content_ok_token(text: &str) -> bool {
    const MARKER: &str = "CONTENT_OK";
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find(MARKER) {
        let abs = search_from + rel;
        let end = abs + MARKER.len();
        let prev_ok = abs == 0
            || !text[..abs]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        let next_ok = end == text.len()
            || !text[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if prev_ok && next_ok {
            return true;
        }
        search_from = end;
    }
    false
}

/// Shared content-review status (design D1), persisted by the quiz verb
/// as the `content_review` frontmatter scalar. `QuizContentReview` is a
/// re-export alias of this type. `frontmatter_value` is unchanged from
/// the pre-extraction quiz form: `ok` / `flagged [1, 3]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentReview {
    Ok,
    /// Item numbers still flagged after the cap (or `[]` when the verify
    /// spawn failed / was unparseable — conservatively flagged).
    Flagged(Vec<u32>),
}

impl ContentReview {
    /// Caller-frontmatter scalar: `ok` or `flagged [1, 3]`.
    pub fn frontmatter_value(&self) -> String {
        match self {
            ContentReview::Ok => "ok".to_string(),
            ContentReview::Flagged(ns) => {
                let list = ns
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("flagged [{list}]")
            }
        }
    }
}

/// Terminal outcome of [`run_content_verify_loop`]. `Flagged` carries
/// the residual defects still reported at the cap (or after a non-fatal
/// failure); an **empty** `Flagged` vec is the conservative
/// spawn-failed / unparseable non-fatal flag. `Ok` means a verify step
/// reported no defects within the cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentVerifyOutcome {
    Ok,
    Flagged(Vec<ContentDefect>),
}

/// Bounded verify → (on defects) repair → re-verify orchestration
/// (design D1). Owns the hard cap, the keep-best-on-cap semantics, the
/// stop-early-on-clean exit, and the best-effort failure handling;
/// callers inject the two spawn-running closures and a cancellation
/// probe.
///
/// - `verify(&body) -> Ok(Some(vec![]))` → clean, loop returns
///   [`ContentVerifyOutcome::Ok`].
/// - `verify(&body) -> Ok(Some(defects))` → run `repair(&body,
///   &defects)`; on `Ok(new_body)` re-verify, on `Err` stop keeping the
///   current body, outcome `Flagged(defects)`.
/// - `verify(&body) -> Ok(None)` (unparseable) or `Err(_)` → stop,
///   outcome `Flagged(vec![])` (conservative, never silently `Ok`).
/// - Cancellation observed at the top of an iteration → stop, returning
///   the outcome accumulated so far.
///
/// Returns the final (possibly repaired) body and the terminal outcome.
pub fn run_content_verify_loop<B, C, V, R>(
    initial: B,
    cancelled: C,
    verify: V,
    repair: R,
) -> (B, ContentVerifyOutcome)
where
    C: Fn() -> bool,
    V: FnMut(&B) -> Result<Option<Vec<ContentDefect>>, VerbError>,
    R: FnMut(&B, &[ContentDefect]) -> Result<B, VerbError>,
{
    let mut verify = verify;
    let mut repair = repair;
    let mut body = initial;
    let mut outcome = ContentVerifyOutcome::Ok;
    for _ in 0..CONTENT_VERIFY_CAP {
        if cancelled() {
            break;
        }
        let parsed = match verify(&body) {
            Ok(p) => p,
            Err(_) => {
                // Best-effort: a verify-spawn failure is non-fatal and
                // conservatively flagged (never silently Ok).
                outcome = ContentVerifyOutcome::Flagged(Vec::new());
                break;
            }
        };
        match parsed {
            None => {
                // Unparseable verify output → conservative flag.
                outcome = ContentVerifyOutcome::Flagged(Vec::new());
                break;
            }
            Some(d) if d.is_empty() => {
                outcome = ContentVerifyOutcome::Ok;
                break;
            }
            Some(d) => {
                outcome = ContentVerifyOutcome::Flagged(d.clone());
                match repair(&body, &d) {
                    Ok(new_body) => {
                        body = new_body;
                    }
                    Err(_) => {
                        // Keep the prior best body; stay flagged.
                        break;
                    }
                }
            }
        }
    }
    (body, outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    // --- parse_content_defects (design D1; spec verb-library / Goal
    // Content Verification and Repair — the verify-output parser) ---

    #[test]
    fn parse_content_ok_yields_empty_defects() {
        assert_eq!(parse_content_defects("CONTENT_OK"), Some(vec![]));
        assert_eq!(
            parse_content_defects("some preamble\nCONTENT_OK\n"),
            Some(vec![])
        );
    }

    #[test]
    fn parse_defect_lines_yield_per_item_defects() {
        let text = "Q1 | answer-wrong | pick the supported option\n\
                    Q3 | out-of-scope | not stated by the pages";
        assert_eq!(
            parse_content_defects(text),
            Some(vec![
                ContentDefect {
                    id: "Q1".into(),
                    kind: "answer-wrong".into(),
                    suggestion: "pick the supported option".into(),
                },
                ContentDefect {
                    id: "Q3".into(),
                    kind: "out-of-scope".into(),
                    suggestion: "not stated by the pages".into(),
                },
            ])
        );
    }

    #[test]
    fn parse_supports_non_q_string_ids_for_goal_pages() {
        let text = "wiki/concepts/jwt.md | unfaithful | not grounded in raw/code";
        assert_eq!(
            parse_content_defects(text),
            Some(vec![ContentDefect {
                id: "wiki/concepts/jwt.md".into(),
                kind: "unfaithful".into(),
                suggestion: "not grounded in raw/code".into(),
            }])
        );
    }

    #[test]
    fn parse_neither_ok_nor_defects_is_none() {
        assert_eq!(parse_content_defects("just prose, no verdict"), None);
        assert_eq!(parse_content_defects(""), None);
        // Malformed (wrong arity / empty type) lines are skipped; with no
        // CONTENT_OK and no well-formed defect → None (unparseable).
        assert_eq!(parse_content_defects("Q1 | only-two-parts"), None);
        assert_eq!(parse_content_defects("Q1 |  | empty type"), None);
    }

    // prompt-surface-output-discipline-batch (F38/F78 collateral): the
    // SKILL Mode C / Verify mode `STOP` boundary is a best-effort prompt
    // contract; real-LLM observation shows the agent occasionally inlines
    // a brief rationale BEFORE `CONTENT_OK` AND the agent CLI delivers
    // them as separate `text` content blocks (each one a Thought event)
    // that the verb-side accumulator concatenates without an inserted
    // newline. The accumulator must NOT inject newlines (that would
    // corrupt other spawns whose body is legitimately split across
    // multiple text blocks, e.g., quiz generate). Instead the verify
    // parser tolerantly recognises a `CONTENT_OK` token in the text when
    // no defect lines were found.
    #[test]
    fn parse_tolerates_inline_prose_before_content_ok() {
        // The exact 2026-05-24 verify spawn output, after Thought-event
        // concatenation: a narrative sentence followed by CONTENT_OK with
        // no separator between them.
        let text = "我正在讀取三個計畫頁面來驗證測驗內容的正確性。CONTENT_OK";
        assert_eq!(parse_content_defects(text), Some(vec![]));
    }

    #[test]
    fn parse_tolerates_content_ok_anywhere_when_no_defects() {
        // Even when the marker appears on its own logical position but
        // the accumulator concatenated it with surrounding chunks.
        assert_eq!(
            parse_content_defects("  read all 3 pages.  CONTENT_OK  "),
            Some(vec![])
        );
        assert_eq!(
            parse_content_defects("CONTENT_OK following a thought"),
            Some(vec![])
        );
    }

    #[test]
    fn parse_defects_take_precedence_over_inline_content_ok_substring() {
        // If the agent emits a real defect line AND the substring
        // CONTENT_OK appears elsewhere (rare; SKILL forbids), defect
        // wins — never silently OK a flagged response.
        let text = "Q1 | answer-wrong | the marked option does not match — CONTENT_OK pattern broken";
        let defects = parse_content_defects(text).expect("must parse as defects");
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].id, "Q1");
        assert_eq!(defects[0].kind, "answer-wrong");
    }

    // --- ContentReview::frontmatter_value (unchanged from quiz) ---

    #[test]
    fn frontmatter_value_matches_quiz_form() {
        assert_eq!(ContentReview::Ok.frontmatter_value(), "ok");
        assert_eq!(
            ContentReview::Flagged(vec![1, 3]).frontmatter_value(),
            "flagged [1, 3]"
        );
        assert_eq!(
            ContentReview::Flagged(vec![]).frontmatter_value(),
            "flagged []"
        );
    }

    // --- run_content_verify_loop (design D1/D4) ---

    fn defect(id: &str) -> ContentDefect {
        ContentDefect {
            id: id.into(),
            kind: "x".into(),
            suggestion: "s".into(),
        }
    }

    #[test]
    fn loop_no_defects_stops_immediately_ok() {
        let calls = Cell::new(0u8);
        let (body, outcome) = run_content_verify_loop(
            "body".to_string(),
            || false,
            |_b| {
                calls.set(calls.get() + 1);
                Ok(Some(vec![]))
            },
            |_b, _d| -> Result<String, VerbError> {
                panic!("repair must not run when verify is clean")
            },
        );
        assert_eq!(body, "body");
        assert_eq!(outcome, ContentVerifyOutcome::Ok);
        assert_eq!(calls.get(), 1, "verify runs exactly once then stops");
    }

    #[test]
    fn loop_repairs_then_clears_returns_ok_with_repaired_body() {
        let vcalls = Cell::new(0u8);
        let (body, outcome) = run_content_verify_loop(
            "v0".to_string(),
            || false,
            |_b| {
                let n = vcalls.get();
                vcalls.set(n + 1);
                if n == 0 {
                    Ok(Some(vec![defect("Q1")]))
                } else {
                    Ok(Some(vec![]))
                }
            },
            |_b, _d| Ok("v1".to_string()),
        );
        assert_eq!(body, "v1", "repaired body is threaded through");
        assert_eq!(outcome, ContentVerifyOutcome::Ok);
        assert_eq!(vcalls.get(), 2, "verify, repair, re-verify clean");
    }

    #[test]
    fn loop_caps_at_three_iterations_keeping_best_and_flagged() {
        let vcalls = Cell::new(0u8);
        let rcalls = Cell::new(0u8);
        let (body, outcome) = run_content_verify_loop(
            0u32,
            || false,
            |_b| {
                vcalls.set(vcalls.get() + 1);
                Ok(Some(vec![defect("Q1")]))
            },
            |b, _d| {
                rcalls.set(rcalls.get() + 1);
                Ok(b + 1)
            },
        );
        // Hard cap of 3 verify iterations; repair runs after each
        // still-flagged verify (3 times — once per capped iteration).
        assert_eq!(vcalls.get(), CONTENT_VERIFY_CAP);
        assert_eq!(rcalls.get(), CONTENT_VERIFY_CAP);
        assert_eq!(body, u32::from(CONTENT_VERIFY_CAP), "best body kept");
        assert_eq!(
            outcome,
            ContentVerifyOutcome::Flagged(vec![defect("Q1")]),
            "residual defects reported after the cap"
        );
    }

    #[test]
    fn loop_verify_error_is_conservative_flagged_empty() {
        let (_b, outcome) = run_content_verify_loop(
            "b".to_string(),
            || false,
            |_b| Err(VerbError::Internal {
                message: "spawn boom".into(),
            }),
            |_b, _d| Ok("unused".to_string()),
        );
        assert_eq!(
            outcome,
            ContentVerifyOutcome::Flagged(vec![]),
            "verify Err → conservative flagged (never silently Ok)"
        );
    }

    #[test]
    fn loop_unparseable_verify_is_conservative_flagged_empty() {
        let (_b, outcome) = run_content_verify_loop(
            "b".to_string(),
            || false,
            |_b| Ok(None),
            |_b, _d| Ok("unused".to_string()),
        );
        assert_eq!(outcome, ContentVerifyOutcome::Flagged(vec![]));
    }

    #[test]
    fn loop_repair_error_keeps_body_and_stays_flagged() {
        let (body, outcome) = run_content_verify_loop(
            "orig".to_string(),
            || false,
            |_b| Ok(Some(vec![defect("Q2")])),
            |_b, _d| Err(VerbError::Internal {
                message: "repair boom".into(),
            }),
        );
        assert_eq!(body, "orig", "repair failure keeps the prior best body");
        assert_eq!(
            outcome,
            ContentVerifyOutcome::Flagged(vec![defect("Q2")])
        );
    }

    #[test]
    fn loop_cancelled_before_first_verify_returns_ok_without_spawn() {
        let (_b, outcome) = run_content_verify_loop(
            "b".to_string(),
            || true,
            |_b| -> Result<Option<Vec<ContentDefect>>, VerbError> {
                panic!("verify must not run when cancelled up front")
            },
            |_b, _d| Ok("x".to_string()),
        );
        assert_eq!(outcome, ContentVerifyOutcome::Ok);
    }
}
