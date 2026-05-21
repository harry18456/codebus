//! Integration tests for `codebus quiz` end-to-end flow (v3-app-quiz
//! task 4.2 — design D8 makes the CLI layer the owner of end-to-end
//! mock_claude spawn tests for the quiz verb).
//!
//! Drives the real codebus binary against tempdir repos with the
//! test-only mock-claude wired in via `CODEBUS_CLAUDE_BIN`. The mock's
//! `quiz-*` behaviors inspect the prompt to distinguish the plan spawn
//! from the generate spawn (codebus spawns claude once per phase).
//!
//! CLI surface note: `codebus quiz "<topic>"` is Goal-scope only (cli
//! spec Quiz Subcommand Behavior). Page-scope has no CLI entry point —
//! its branch logic is unit-tested in `codebus-core` (task 2.3) and its
//! end-to-end path is exercised through the library by the GUI
//! (task 5.3), so it is intentionally absent here.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

fn run_init(repo: &Path) -> Output {
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo)
        .output()
        .expect("run codebus init")
}

/// Run `codebus quiz <topic> [--count N]` with mock-claude wired in.
/// Returns the binary Output and the mock argv/cwd dump path.
fn run_quiz(
    repo: &Path,
    topic: &str,
    behavior: &str,
    count: Option<u8>,
) -> (Output, std::path::PathBuf) {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let mut args: Vec<String> = vec!["quiz".into(), topic.into()];
    if let Some(c) = count {
        args.push("--count".into());
        args.push(c.to_string());
    }
    let out = Command::new(BIN)
        .args(&args)
        .current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log)
        .output()
        .expect("run codebus quiz");
    (out, log)
}

fn quiz_dir(repo: &Path) -> std::path::PathBuf {
    repo.join(".codebus").join("quiz")
}

fn find_quiz_file(repo: &Path) -> Option<std::path::PathBuf> {
    let qd = quiz_dir(repo);
    let mut stack = vec![qd];
    while let Some(d) = stack.pop() {
        let entries = fs::read_dir(&d).ok()?;
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                return Some(p);
            }
        }
    }
    None
}

#[test]
fn quiz_goal_match_writes_file_with_caller_frontmatter() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "how does auth work", "quiz-goal-match", None);
    assert!(
        out.status.success(),
        "quiz goal-match should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let file = find_quiz_file(tmp.path()).expect("a quiz .md file must be written");
    let body = fs::read_to_string(&file).unwrap();

    // Caller-injected frontmatter (design D4).
    assert!(body.starts_with("---\n"), "must start with frontmatter");
    assert!(body.contains("trigger: ai_planned"));
    assert!(body.contains("topic: \"how does auth work\""));
    assert!(body.contains("quiz_id: "));
    assert!(
        body.contains("planned_pages:")
            && body.contains("wiki/concepts/jwt-token-lifecycle.md"),
        "planned_pages must list the planned scope:\n{body}"
    );
    assert!(
        body.contains("generation_token_usage:"),
        "token usage frontmatter must be present"
    );
    assert!(body.contains("events_log:"), "events_log pointer present");
    // Question body present, no agent-authored frontmatter leaked.
    assert!(body.contains("## Q1."));
    assert!(body.contains("## Answer: B"));
}

// --- quiz-content-verify task 3.1: caller-orchestrated verify→repair
// loop (design D1/D3/D4/D6; spec quiz / Quiz Content Verification and
// Repair). content_verify is gated by the shared config key.

/// Like `run_quiz` but seeds `<home>/.codebus/config.yaml` so the CLI
/// resolves `quiz.content_verify` from config.
fn run_quiz_cfg(
    repo: &Path,
    topic: &str,
    behavior: &str,
    cfg_body: &str,
) -> Output {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let cb = home.path().join(".codebus");
    fs::create_dir_all(&cb).unwrap();
    fs::write(cb.join("config.yaml"), cfg_body).unwrap();
    Command::new(BIN)
        .args(["quiz", topic])
        .current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log)
        .output()
        .expect("run codebus quiz (cfg)")
}

#[test]
fn content_verify_off_omits_content_review() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());
    // default config (no quiz.content_verify) → stage skipped entirely.
    let (out, _l) = run_quiz(tmp.path(), "auth", "quiz-clean-body", None);
    assert!(out.status.success());
    let body = fs::read_to_string(find_quiz_file(tmp.path()).unwrap()).unwrap();
    assert!(
        frontmatter_value(&body, "content_review").is_none(),
        "content_verify off MUST NOT write content_review:\n{body}"
    );
}

#[test]
fn content_verify_clean_marks_ok() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());
    let out = run_quiz_cfg(
        tmp.path(),
        "auth",
        "quiz-verify-clean",
        "quiz:\n  content_verify: true\n",
    );
    assert!(
        out.status.success(),
        "exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = fs::read_to_string(find_quiz_file(tmp.path()).unwrap()).unwrap();
    assert_eq!(
        frontmatter_value(&body, "content_review").as_deref(),
        Some("ok"),
        "clean verify must persist content_review: ok:\n{body}"
    );
}

#[test]
fn content_verify_residual_flagged_best_effort() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());
    let out = run_quiz_cfg(
        tmp.path(),
        "auth",
        "quiz-verify-flag",
        "quiz:\n  content_verify: true\n",
    );
    // best-effort: residual content defects do NOT change exit code.
    assert!(out.status.success(), "best-effort exit 0");
    let body = fs::read_to_string(find_quiz_file(tmp.path()).unwrap()).unwrap();
    let cr = frontmatter_value(&body, "content_review").unwrap_or_default();
    assert!(
        cr.starts_with("flagged"),
        "residual must be content_review: flagged (+Q1):\n{body}"
    );
    assert!(cr.contains('1'), "flagged question number 1 must be listed: {cr}");
    assert!(body.contains("## Q1."), "questions must not be dropped");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("content"),
        "a non-fatal content warning must be surfaced; stderr:\n{stderr}"
    );
}

// --- verify-stage-independent-model task 3.1 (RED) ---
// Quiz verify spawn SHALL use the model resolved via `Verb::Verify`,
// NOT `Verb::Quiz`. The mock_claude binary overwrites the log per
// invocation, so for `quiz-verify-clean` (sequence: plan → generate →
// verify → exit) the final log captures the verify spawn's argv.

#[test]
fn quiz_verify_spawn_uses_verb_verify_model_not_quiz_model() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());

    // Configure quiz (= reuses query) to haiku-4-5, verify to opus-4-6.
    // After the change, the verify spawn must invoke claude with
    // --model claude-opus-4-6 (NOT claude-haiku-4-5 which is what
    // Verb::Quiz resolves to).
    let cfg_body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: high   }\n        query:  { model: haiku-4-5,  effort: low    }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high   }\nquiz:\n  content_verify: true\n";
    let out = run_quiz_cfg(tmp.path(), "auth", "quiz-verify-clean", cfg_body);
    assert!(
        out.status.success(),
        "quiz-verify-clean should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The mock log was overwritten by each spawn; the final write is
    // the verify spawn (last invocation before exit).
    let log_path = tmp.path().join("mock-claude.log");
    let log = fs::read_to_string(&log_path)
        .expect("mock-claude.log must exist after the verify spawn");
    assert!(
        log.contains("arg=claude-opus-4-6"),
        "verify spawn must use Verb::Verify resolved model (claude-opus-4-6); \
         got log:\n{log}"
    );
    assert!(
        !log.contains("arg=claude-haiku-4-5"),
        "verify spawn must NOT use Verb::Quiz resolved model (claude-haiku-4-5); \
         got log:\n{log}"
    );
}

// --- quiz-validate-repair task 4.1: run_quiz_generate final-verify ---
// (design D1/D3/D4; spec quiz / Quiz Output Validation and Repair).

#[test]
fn quiz_clean_body_marks_validation_ok() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "validator topic", "quiz-clean-body", None);
    assert!(
        out.status.success(),
        "clean quiz should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file = find_quiz_file(tmp.path()).expect("quiz file written");
    let body = fs::read_to_string(&file).unwrap();
    assert_eq!(
        frontmatter_value(&body, "validation").as_deref(),
        Some("ok"),
        "structurally valid, citation-free quiz must persist validation: ok:\n{body}"
    );
}

#[test]
fn quiz_bad_body_marks_validation_failed_best_effort() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "validator topic", "quiz-bad-body", None);
    // Best-effort: residual validation failure does NOT change exit code.
    assert!(
        out.status.success(),
        "bad quiz must still exit 0 (best-effort persist); stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file = find_quiz_file(tmp.path()).expect("quiz file still written best-effort");
    let body = fs::read_to_string(&file).unwrap();
    assert_eq!(
        frontmatter_value(&body, "validation").as_deref(),
        Some("failed"),
        "residual finding must persist validation: failed:\n{body}"
    );
    // Question NOT dropped — the malformed block is still in the file.
    assert!(
        body.contains("## Q1."),
        "the question block must not be dropped:\n{body}"
    );
    // Non-fatal warning surfaced to stderr.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("validation"),
        "a non-fatal validation warning must be surfaced; stderr:\n{stderr}"
    );
}

/// Extract a scalar frontmatter value (`key: value`, optional quotes)
/// from a persisted quiz markdown body.
fn frontmatter_value(body: &str, key: &str) -> Option<String> {
    for line in body.lines() {
        if line == "---" && !body.starts_with(&format!("---\n{key}")) {
            // keep scanning; frontmatter block only
        }
        if let Some(rest) = line.strip_prefix(&format!("{key}:")) {
            let v = rest.trim().trim_matches('"').trim().to_string();
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

/// fix-app-quiz task 3.1/3.2 — the `events_log` frontmatter pointer MUST
/// resolve to a real on-disk file containing that generate spawn's
/// events (not a mock/placeholder path). Design D3 / spec `quiz`
/// Quiz Storage Layout.
#[test]
fn quiz_events_log_points_to_real_generate_events_file() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "how does auth work", "quiz-goal-match", None);
    assert!(
        out.status.success(),
        "quiz goal-match should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let file = find_quiz_file(tmp.path()).expect("a quiz .md file must be written");
    let body = fs::read_to_string(&file).unwrap();

    let events_log =
        frontmatter_value(&body, "events_log").expect("events_log frontmatter value");
    let events_path = Path::new(&events_log);
    assert!(
        events_path.is_absolute(),
        "events_log should be an absolute path, got: {events_log}"
    );
    assert!(
        events_path.exists(),
        "events_log must point to a real on-disk file, missing: {events_log}"
    );
    let events_body = fs::read_to_string(events_path).unwrap();
    let lines: Vec<&str> = events_body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(
        !lines.is_empty(),
        "events.jsonl must contain this generate spawn's events, was empty: {events_log}"
    );
    assert!(
        lines.iter().all(|l| serde_json::from_str::<serde_json::Value>(l).is_ok()),
        "every events.jsonl line must be valid JSON (an event envelope)"
    );
    assert!(
        events_body.contains("\"event\""),
        "events.jsonl lines must be EventEnvelope records (have an \"event\" field)"
    );
}

#[test]
fn quiz_no_match_exits_zero_without_file() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "quantum mechanics", "quiz-no-match", None);
    assert!(
        out.status.success(),
        "no-match must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("no matching wiki pages"),
        "no-match reason must be printed:\n{stdout}"
    );
    assert!(
        find_quiz_file(tmp.path()).is_none(),
        "no quiz file may be written on a no-match"
    );
}

#[test]
fn quiz_explicit_count_passes_through_to_generate_prompt() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, log) = run_quiz(tmp.path(), "auth", "quiz-goal-match", Some(7));
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    // The mock overwrites the log per spawn; the surviving dump is the
    // generate spawn (last). Its prompt must carry count=7.
    let dump = fs::read_to_string(&log).expect("mock log");
    assert!(
        dump.contains("count=7"),
        "generate prompt must carry the explicit --count 7:\n{dump}"
    );
}

#[test]
fn quiz_count_falls_back_to_default_five() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    // No --count, isolated CODEBUS_HOME → no quiz.default_length config.
    let (out, log) = run_quiz(tmp.path(), "auth", "quiz-goal-match", None);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let dump = fs::read_to_string(&log).expect("mock log");
    assert!(
        dump.contains("count=5"),
        "missing --count and config must default to 5:\n{dump}"
    );
}

#[test]
fn quiz_does_not_auto_commit() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(tmp.path())
            .output()
            .expect("git")
    };
    let _ = git(&["init"]);
    let _ = git(&["config", "user.email", "t@t.t"]);
    let _ = git(&["config", "user.name", "t"]);
    let _ = git(&["add", "-A"]);
    let _ = git(&["commit", "-m", "base"]);
    let before = String::from_utf8_lossy(&git(&["rev-list", "--count", "HEAD"]).stdout)
        .trim()
        .to_string();

    let (out, _log) = run_quiz(tmp.path(), "auth", "quiz-goal-match", None);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let after = String::from_utf8_lossy(&git(&["rev-list", "--count", "HEAD"]).stdout)
        .trim()
        .to_string();
    assert_eq!(
        before, after,
        "quiz is read-only and MUST NOT create a commit"
    );
}

#[test]
fn quiz_fenced_body_is_stripped_before_persist() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, _log) = run_quiz(tmp.path(), "auth", "quiz-fenced", None);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let file = find_quiz_file(tmp.path()).expect("quiz file written");
    let body = fs::read_to_string(&file).unwrap();
    // Frontmatter then question body — the ```markdown fence the mock
    // wrapped the body in must have been stripped.
    assert!(body.starts_with("---\n"));
    assert!(
        !body.contains("```"),
        "code fence must be stripped before persist:\n{body}"
    );
    assert!(body.contains("## Q1."));
}

fn count_quiz_md(repo: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![quiz_dir(repo)];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                n += 1;
            }
        }
    }
    n
}

// task 6.2 / spec quiz § Quiz Storage Layout and Retry Semantics +
// design D5: retry is a plain re-spawn. Two runs of the same topic
// produce two non-destructive timestamped files; the prior attempt is
// untouched. "No prior-stem injection" is structurally guaranteed —
// `QuizGenerateOptions { pages, question_count }` has no stems field.
#[test]
fn retry_same_topic_writes_two_non_destructive_files() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (o1, _) = run_quiz(tmp.path(), "auth", "quiz-goal-match", None);
    assert!(o1.status.success(), "first run");
    let first = find_quiz_file(tmp.path()).expect("first attempt file");
    let first_body = fs::read_to_string(&first).unwrap();

    // quiz_id is a second-precision timestamp; space the runs so the
    // second attempt cannot collide with / overwrite the first.
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let (o2, _) = run_quiz(tmp.path(), "auth", "quiz-goal-match", None);
    assert!(o2.status.success(), "retry run");

    assert_eq!(
        count_quiz_md(tmp.path()),
        2,
        "retry must create a second, non-destructive attempt file"
    );
    assert_eq!(
        fs::read_to_string(&first).unwrap(),
        first_body,
        "the prior attempt file must be unchanged by the retry"
    );
}
