//! `codebus quiz "<topic>"` — CLI wrapper over
//! `codebus_core::verb::quiz::run_quiz` (`QuizScope::Goal`).
//!
//! Resolves `question_count` (`--count` flag → shared
//! `quiz.default_length` config → 5; the CLI never reads the app-only
//! `app.*` namespace), drives the two-shot plan/generate flow, prints
//! the planned scope (no interactive confirm gate — that is GUI-only),
//! and on success persists the quiz markdown with caller-injected
//! frontmatter under `<vault>/.codebus/quiz/<slug>/<quiz_id>.md`. A
//! no-match outcome prints the reason, writes no file, and exits zero.
//! Read-only: no auto-commit.
//!
//! See spec `cli` (Quiz Subcommand Behavior) + `quiz` capability +
//! design D2/D4/D8. End-to-end mock_claude tests: `tests/quiz_flow.rs`.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::config::{ConfigLoadError, default_config_path, load_quiz_config};
use codebus_core::render::{RenderOptions, print_banner, print_event};
use codebus_core::verb::quiz::{
    QuizGenerateOptions, QuizPlanOptions, QuizPlanOutcome, QuizTrigger, persist_quiz,
    run_quiz_generate, run_quiz_plan,
};
use codebus_core::verb::{VerbError, VerbEvent};

/// `codebus quiz "<topic>"` — Goal-scope quiz generation.
#[derive(Args, Debug)]
pub struct QuizArgs {
    /// Free-text topic to build the quiz from. The plan spawn turns this
    /// into a concrete wiki-page scope (or a no-match).
    pub topic: String,

    /// Number of questions (3–10). When omitted, the shared
    /// `quiz.default_length` config key is used, defaulting to 5 when
    /// that key is absent. The codebus CLI never reads the app-only
    /// `app.*` namespace.
    #[arg(long)]
    pub count: Option<u8>,
}

/// Resolve `question_count`: explicit `--count` wins; otherwise the
/// shared `quiz.default_length` config key; otherwise 5. A config load
/// error (e.g. out-of-range value) warns to stderr and falls back to 5,
/// mirroring the warn-and-default contract the config loader documents.
fn resolve_count(flag: Option<u8>) -> u8 {
    if let Some(v) = flag {
        return v;
    }
    match default_config_path() {
        Some(p) => match load_quiz_config(&p) {
            Ok(cfg) => cfg.default_length,
            Err(e) => {
                eprintln!("warning: quiz.default_length config invalid ({e}); using 5");
                5
            }
        },
        None => 5,
    }
}

// slug + frontmatter persistence moved to codebus_core::verb::quiz
// (`quiz_slug` / `persist_quiz` / `QuizTrigger`) so the CLI and GUI
// share one source of truth (v3-app-quiz task 5.5 / design D4/D7).

pub async fn run(
    repo: &Path,
    args: QuizArgs,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    if debug {
        eprintln!("[debug] quiz: repo={}", repo.display());
    }

    let question_count = resolve_count(args.count);

    // Shared stream renderer for both spawns. The CLI has NO interactive
    // confirm gate (design D1 / cli spec Quiz Subcommand Behavior — the
    // gate is GUI-only): after the plan returns a scope we print it and
    // proceed straight to generation. Lifecycle scope/no-match are acted
    // on via the returned `QuizPlanReport.outcome`, not the event.
    let render_for_closure = render_opts.clone();
    let render_stream = move |event: VerbEvent| match event {
        VerbEvent::Banner(b) => print_banner(b.as_banner(), &render_for_closure),
        VerbEvent::Stream(s) => print_event(&s, &render_for_closure),
        VerbEvent::Lifecycle(_) => {}
    };

    // --- plan spawn ---
    let plan = match run_quiz_plan(
        repo,
        QuizPlanOptions {
            topic: args.topic.clone(),
        },
        render_stream.clone(),
        None,
    ) {
        Ok(p) => p,
        Err(e) => return translate_error("quiz", &e, render_opts),
    };

    let pages = match plan.outcome {
        QuizPlanOutcome::NoMatch(reason) => {
            // No-match: print the reason, write no file, exit zero.
            println!("no matching wiki pages: {reason}");
            return ExitCode::SUCCESS;
        }
        QuizPlanOutcome::Scope(pages) => {
            println!("planned quiz scope:");
            for p in &pages {
                println!("  - {p}");
            }
            pages
        }
    };

    // --- generate spawn (CLI proceeds with no confirm gate) ---
    match run_quiz_generate(
        repo,
        QuizGenerateOptions {
            pages,
            question_count,
        },
        render_stream,
        None,
    ) {
        Ok(report) => match persist_quiz(
            repo,
            &QuizTrigger::AiPlanned {
                topic: args.topic.clone(),
            },
            &report,
        ) {
            Ok(path) => {
                println!("quiz written: {}", path.display());
                let exit: u8 = report
                    .agent_exit_code
                    .and_then(|c| u8::try_from(c).ok())
                    .unwrap_or(0);
                ExitCode::from(exit)
            }
            Err(e) => {
                eprintln!("error: quiz: failed to persist quiz file: {e}");
                ExitCode::from(1)
            }
        },
        Err(e) => translate_error("quiz", &e, render_opts),
    }
}


fn translate_error(verb: &str, err: &VerbError, _render_opts: &RenderOptions) -> ExitCode {
    match err {
        VerbError::VaultMissing { path } => {
            eprintln!(
                "error: {verb}: vault not found at {}; run `codebus init` first",
                path.display()
            );
            ExitCode::from(2)
        }
        VerbError::ConfigParse { which, source } => {
            let path_disp = default_config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            match source {
                ConfigLoadError::Io(_) | ConfigLoadError::YamlParse(_) => {
                    eprintln!("error: {which} config parse failed at {path_disp}: {source}");
                }
            }
            ExitCode::from(2)
        }
        VerbError::KeyringMissing { source } => {
            eprintln!("error: {verb}: {source}");
            ExitCode::from(3)
        }
        VerbError::Spawn { source } => {
            eprintln!("error: spawn claude: {source}");
            ExitCode::from(1)
        }
        VerbError::Cancelled => ExitCode::from(0),
        VerbError::Internal { message } => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
