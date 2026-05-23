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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use codebus_core::config::{ConfigLoadError, default_config_path, load_quiz_config};
use codebus_core::render::{RenderOptions, print_banner, print_event};
use codebus_core::verb::quiz::{
    QuizGenerateOptions, QuizPlanOptions, QuizPlanOutcome, QuizTrigger, persist_quiz,
    run_quiz_generate, run_quiz_plan,
};
use codebus_core::verb::quiz_validate::validate_quiz_body;
use codebus_core::verb::{VerbError, VerbEvent};
use codebus_core::wiki::lint::{LocateError, locate_vault_root};

/// `codebus quiz` — Goal-scope quiz generation (`codebus quiz "<topic>"`)
/// plus the `validate` sub-action (`codebus quiz validate <file>`). The
/// eight top-level subcommands are unchanged; `validate` is a sub-action
/// under `quiz`, mirroring the `config` sub-action precedent.
#[derive(Args, Debug)]
#[command(args_conflicts_with_subcommands = true)]
pub struct QuizArgs {
    #[command(subcommand)]
    pub action: Option<QuizAction>,

    /// Free-text topic to build the quiz from (generate mode). The plan
    /// spawn turns this into a concrete wiki-page scope (or a no-match).
    /// Required unless a sub-action is given.
    pub topic: Option<String>,

    /// Number of questions (3–10). When omitted, the shared
    /// `quiz.default_length` config key is used, defaulting to 5 when
    /// that key is absent. The codebus CLI never reads the app-only
    /// `app.*` namespace.
    #[arg(long)]
    pub count: Option<u8>,
}

#[derive(Subcommand, Debug)]
pub enum QuizAction {
    /// Validate a generated quiz markdown file (deterministic schema +
    /// `[[slug]]` wikilink-existence checks). Shares the same validator
    /// the library final-verify uses. Exits 0 when clean, 1 when the
    /// file has findings.
    Validate(QuizValidateArgs),
}

#[derive(Args, Debug)]
pub struct QuizValidateArgs {
    /// Quiz markdown source: a file path, OR `-` / omitted to read the
    /// body from standard input (the codebus-quiz agent pipes its
    /// in-context draft this way to self-validate).
    pub file: Option<String>,

    /// Emit a machine-readable JSON findings array instead of human text.
    #[arg(long)]
    pub json: bool,
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

/// Resolve `quiz.content_verify` from the shared `quiz.*` config
/// (default `false`; the CLI never reads the app-only `app.*`
/// namespace). A config load error warns and falls back to `false`
/// (conservative: do not silently enable extra spawns).
fn resolve_content_verify() -> bool {
    match default_config_path() {
        Some(p) => match load_quiz_config(&p) {
            Ok(cfg) => cfg.content_verify,
            Err(e) => {
                eprintln!("warning: quiz config invalid ({e}); content_verify=false");
                false
            }
        },
        None => false,
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

    if let Some(QuizAction::Validate(v)) = &args.action {
        return run_validate(v, debug);
    }

    let topic = match args.topic.clone() {
        Some(t) => t,
        None => {
            eprintln!(
                "error: quiz: a topic is required (usage: `codebus quiz \"<topic>\"`), \
                 or use a sub-action (`codebus quiz validate <file>`)"
            );
            return ExitCode::from(2);
        }
    };

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
            topic: topic.clone(),
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
            content_verify: resolve_content_verify(),
            // CLI is always the Goal flow → supply the originating topic
            // so the off-topic content check (design D6) can run.
            topic: Some(topic.clone()),
        },
        render_stream,
        None,
    ) {
        Ok(report) => match persist_quiz(
            repo,
            &QuizTrigger::AiPlanned {
                topic: topic.clone(),
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


/// `codebus quiz validate <file> [--json]` — run the deterministic quiz
/// validator (shared with the library final-verify) over `file`. Locates
/// the vault wiki catalog from cwd so `[[slug]]` existence resolves.
/// Exits 0 when there are no findings, 1 when findings exist, 2 on a
/// setup error (no vault / unreadable file).
fn run_validate(args: &QuizValidateArgs, debug: bool) -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let vault_root = match locate_vault_root(&cwd, None) {
        Ok(p) => p,
        Err(LocateError::NoVaultFound) => {
            eprintln!(
                "error: quiz validate: no codebus vault found at cwd or under .codebus/ — run `codebus init` first"
            );
            return ExitCode::from(2);
        }
    };
    let wiki_root = vault_root.join("wiki");

    // Source = file path, OR stdin when the arg is `-` or omitted. The
    // codebus-quiz generate agent pipes its in-context draft via stdin
    // (design «Bash sandbox»), avoiding a scratch-file lifecycle.
    let from_stdin = match args.file.as_deref() {
        None | Some("-") => true,
        Some(_) => false,
    };
    if debug {
        eprintln!(
            "[debug] quiz validate: wiki_root={} source={}",
            wiki_root.display(),
            if from_stdin {
                "<stdin>".to_string()
            } else {
                args.file.clone().unwrap_or_default()
            }
        );
    }

    let body = if from_stdin {
        use std::io::Read;
        let mut s = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut s) {
            eprintln!("error: quiz validate: cannot read stdin: {e}");
            return ExitCode::from(2);
        }
        s
    } else {
        let path = args.file.clone().unwrap();
        match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: quiz validate: cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        }
    };

    let issues = validate_quiz_body(&body, &wiki_root);

    if args.json {
        // Machine-readable: the LintIssue array, the same shape the
        // library final-verify and the agent Bash self-check consume.
        match serde_json::to_string(&issues) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: quiz validate: serialize findings: {e}");
                return ExitCode::from(2);
            }
        }
    } else if issues.is_empty() {
        println!("0 issues — quiz is structurally valid and all citations resolve");
    } else {
        let src = if from_stdin {
            "<stdin>".to_string()
        } else {
            args.file.clone().unwrap_or_default()
        };
        println!("{} issue(s) in {src}:", issues.len());
        for i in &issues {
            println!("  [{}] {}: {}", i.rule_id, i.path, i.message);
        }
    }

    if issues.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
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
        VerbError::AgentFailed { exit_code } => {
            // Defensive arm: neither run_quiz_plan nor run_quiz_generate
            // emits AgentFailed (per spec verb-library §Verb Error Enum —
            // one-shot verbs propagate child exit via
            // Ok(report).agent_exit_code). Generic fallback is used
            // instead of unreachable!() so a future regression does NOT
            // panic the binary.
            match exit_code {
                Some(code) => eprintln!("error: {verb}: agent exited with code {code}"),
                None => eprintln!("error: {verb}: agent exited without a recorded exit code"),
            }
            ExitCode::from(err.cli_exit_code())
        }
        VerbError::Internal { message } => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
