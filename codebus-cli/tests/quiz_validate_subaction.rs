//! Integration tests for the `codebus quiz validate <file> [--json]`
//! sub-action (spec `cli` / Quiz Validate Sub-Action Behavior +
//! Subcommand Registration). The sub-action runs the deterministic
//! quiz validator and shares the same validator function the library
//! final-verify uses.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// A vault whose wiki catalog contains `concepts/jwt-pitfalls.md`, plus
/// the requested quiz files written under `.codebus/quiz/t/`.
fn vault_with(quiz_files: &[(&str, &str)]) -> TempDir {
    let d = TempDir::new().unwrap();
    let cb = d.path().join(".codebus");
    let concepts = cb.join("wiki").join("concepts");
    fs::create_dir_all(&concepts).unwrap();
    fs::write(cb.join("wiki").join("index.md"), "# index\n").unwrap();
    fs::write(concepts.join("jwt-pitfalls.md"), "# jwt\n").unwrap();
    let qdir = cb.join("quiz").join("t");
    fs::create_dir_all(&qdir).unwrap();
    for (name, body) in quiz_files {
        fs::write(qdir.join(name), body).unwrap();
    }
    d
}

/// Run `codebus quiz validate ...` with cwd at the vault root
/// (`<tmp>/.codebus`) — the agent-from-vault scenario.
fn quiz_validate(vault: &Path, args: &[&str]) -> Output {
    let mut a = vec!["quiz", "validate"];
    a.extend_from_slice(args);
    Command::new(BIN)
        .args(&a)
        .current_dir(vault.join(".codebus"))
        .output()
        .expect("run codebus quiz validate")
}

const GOOD: &str = "## Q1. What does AuthMiddleware return on an expired token?\n\n\
- A) 200\n- B) 301\n- C) 401\n- D) 500\n\n## Answer: C\n\n\
## Explanation: expired tokens 401, see [[jwt-pitfalls]].";

const BAD_MISSING_ANSWER: &str =
    "## Q1. stem?\n- A) a\n- B) b\n- C) c\n- D) d\n## Explanation: e";

#[test]
fn clean_file_exits_zero() {
    let v = vault_with(&[("good.md", GOOD)]);
    let out = quiz_validate(v.path(), &["quiz/t/good.md"]);
    assert!(
        out.status.success(),
        "expected exit 0; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn findings_exit_one_with_details() {
    let v = vault_with(&[("bad.md", BAD_MISSING_ANSWER)]);
    let out = quiz_validate(v.path(), &["quiz/t/bad.md"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 for a file with findings"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("quiz-schema-answer") && combined.contains("Q1"),
        "human output must name the rule and question; got:\n{combined}"
    );
}

#[test]
fn json_output_is_machine_readable() {
    let v = vault_with(&[("bad.md", BAD_MISSING_ANSWER)]);
    let out = quiz_validate(v.path(), &["quiz/t/bad.md", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout must be valid JSON");
    let arr = parsed.as_array().expect("JSON findings array");
    assert!(
        arr.iter().any(|f| f
            .get("rule_id")
            .and_then(|r| r.as_str())
            .map(|r| r.contains("quiz-schema-answer"))
            .unwrap_or(false)
            && f.get("severity").is_some()),
        "JSON entries must carry rule_id + severity; got: {stdout}"
    );
}

/// Pipe `body` to `codebus quiz validate -` (cwd at the vault root).
fn quiz_validate_stdin(vault: &Path, body: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(["quiz", "validate", "-"])
        .current_dir(vault.join(".codebus"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codebus quiz validate -");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    child.wait_with_output().expect("wait quiz validate -")
}

#[test]
fn stdin_clean_body_exits_zero() {
    let v = vault_with(&[]);
    let out = quiz_validate_stdin(v.path(), GOOD);
    assert!(
        out.status.success(),
        "stdin clean body must exit 0; stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

#[test]
fn stdin_findings_exit_one() {
    let v = vault_with(&[]);
    let out = quiz_validate_stdin(v.path(), BAD_MISSING_ANSWER);
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdin body with findings must exit 1"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("quiz-schema-answer"),
        "stdin human output must name the rule; got:\n{combined}"
    );
}

#[test]
fn quiz_help_lists_validate_subaction() {
    let out = Command::new(BIN)
        .args(["quiz", "--help"])
        .output()
        .expect("run codebus quiz --help");
    assert!(out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("validate"),
        "`codebus quiz --help` must document the `validate` sub-action:\n{combined}"
    );
}

#[test]
fn top_level_help_still_lists_exactly_eight_subcommands() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("run codebus --help");
    assert!(out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for verb in [
        "init", "goal", "query", "lint", "fix", "config", "chat", "quiz",
    ] {
        assert!(
            combined.contains(verb),
            "top-level --help missing `{verb}`:\n{combined}"
        );
    }
    // `validate` must NOT appear as a ninth top-level subcommand. It is a
    // sub-action under `quiz`, so the top-level help SHALL NOT list it as
    // its own command line entry.
    assert!(
        !combined
            .lines()
            .any(|l| l.trim_start().starts_with("validate")),
        "`validate` must not be a ninth top-level subcommand:\n{combined}"
    );
}
