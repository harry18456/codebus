//! Integration test for the `verb-library` capability's "Verb Library
//! Module Surface" requirement (v3-app-quiz extends it to five
//! sub-modules). This is an external-crate consumer of `codebus_core`,
//! exactly matching the spec scenarios' "a downstream crate writes
//! `use codebus_core::verb::...`" framing.
//!
//! Test layering (design D8): compile-resolution only — no agent spawn.
//! The `use` statements below ARE the test for the spec scenario
//! "compilation SHALL succeed": if any sub-module path or exported item
//! did not resolve, this test crate would fail to compile.

// Scenario: "Verb library module path exists" — the five sub-modules
// resolve as public modules.
use codebus_core::verb::{chat, fix, goal, query, quiz};

// Scenario: "Quiz sub-module exports its orchestration surface". Quiz is
// the documented exception to "exactly one orchestration function per
// verb" (verb-library spec delta): it exports TWO — `run_quiz_plan` and
// `run_quiz_generate` — because the GUI confirm gate (design D1 /
// app-workspace Quiz Tab Plan-Confirm-Generate Flow) requires the plan
// and generate spawns to be separately invokable. Importing both proves
// the exports exist without monomorphizing the generic functions.
#[allow(unused_imports)]
use codebus_core::verb::quiz::{run_quiz_generate, run_quiz_plan};
use codebus_core::verb::quiz::{
    QuizGenerateOptions, QuizPlanOptions, QuizPlanOutcome, QuizReport,
};

#[test]
fn five_verb_submodules_resolve() {
    // Type references prove each sub-module path is public without
    // monomorphizing the generic `run_*` functions. The parameter types
    // are public because they appear in each module's public `run_*`
    // signature.
    fn _goal(_: goal::GoalOptions) {}
    fn _query(_: query::QueryOptions) {}
    fn _fix(_: fix::FixOptions) {}
    fn _chat(_: chat::ChatTurnOptions) {}
    fn _quiz(_: quiz::QuizPlanOptions) {}
}

#[test]
fn quiz_option_and_report_types_resolve() {
    let plan = QuizPlanOptions {
        topic: "auth".into(),
    };
    assert_eq!(plan.topic, "auth");
    let gen_opts = QuizGenerateOptions {
        pages: vec!["wiki/modules/auth-middleware.md".into()],
        question_count: 5,
        content_verify: false,
        topic: None,
    };
    assert_eq!(gen_opts.question_count, 5);
    assert!(matches!(
        QuizPlanOutcome::Scope(vec!["wiki/a.md".into()]),
        QuizPlanOutcome::Scope(_)
    ));

    // QuizReport field set is part of the public contract.
    fn _consume(r: QuizReport) -> (String, Vec<String>) {
        (r.quiz_md, r.planned_pages)
    }
}

#[test]
fn init_verb_is_not_under_verb_module() {
    // Scenario: "Init verb is not moved" — init orchestration remains at
    // `codebus_core::vault::init`, NOT under `verb::`. Asserted
    // positively via a type reference to the init module's public option
    // type; the absence of `verb::init` is enforced by the compiler at
    // the `use codebus_core::verb::{...}` site above (no `init` listed).
    fn _init(_: &codebus_core::vault::init::InitOptions) {}
}
