//! Integration tests for the `codebus hook check-bash` PreToolUse Bash
//! hook (spec `lint-feedback-loop` / Fix Bash Hook Installation).
//!
//! These tests spawn the real `codebus` binary, feed it a PreToolUse
//! JSON body on stdin (matching Claude Code's hook input schema), and
//! assert the decision-JSON contract on stdout end-to-end. Pure
//! predicate / decision unit tests (`is_allowed_bash_command`,
//! `is_codebus_lint_command`, heredoc recognizer, metacharacter set)
//! live alongside the implementation in
//! `codebus-cli/src/commands/hook.rs`; this file pins the
//! subprocess-level contract — the same end-to-end coverage that
//! `hook_check_read.rs` gives the Read hook.
//!
//! The check-bash hook reads NO config (unlike check-read's runtime
//! `hooks.read_image_block` gate), so no `CODEBUS_HOME` override is
//! needed here.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Run `codebus hook check-bash` with `body` on stdin; return (exit code,
/// stdout, stderr).
fn run_check_bash(body: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(BIN)
        .args(["hook", "check-bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codebus hook check-bash");
    {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        stdin.write_all(body.as_bytes()).expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait for child");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// Build the standard PreToolUse Bash JSON body around a command string.
/// The command is JSON-escaped (backslashes, quotes, newlines, CRs) so a
/// command containing those bytes still produces valid JSON on the wire —
/// the hook must then see the decoded command.
fn bash_body(command: &str) -> String {
    let escaped = command
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("{{\"tool_name\":\"Bash\",\"tool_input\":{{\"command\":\"{escaped}\"}}}}")
}

/// Assert the allow path: exit 0 AND empty stdout (no decision JSON).
fn assert_allow(code: Option<i32>, stdout: &str) {
    assert_eq!(code, Some(0), "exit code must be 0 (decision via stdout)");
    assert!(
        stdout.trim().is_empty(),
        "allow path must produce empty stdout, got `{stdout}`"
    );
}

/// Assert a block decision-JSON contract: exit 0, parseable JSON with
/// `decision == "block"` and a non-empty `reason`. Returns the parsed
/// value for further reason inspection.
fn assert_block(code: Option<i32>, stdout: &str) -> serde_json::Value {
    assert_eq!(code, Some(0), "exit code must be 0 (decision via stdout)");
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("stdout must be valid JSON, got `{stdout}`: {e}"));
    assert_eq!(
        parsed["decision"], "block",
        "decision must be `block`, got `{stdout}`"
    );
    assert!(
        parsed["reason"].as_str().is_some_and(|s| !s.is_empty()),
        "reason must be a non-empty string, got `{stdout}`"
    );
    parsed
}

/// Assert a metacharacter-rejection block: the reason names the
/// metacharacter rule AND echoes the specific metacharacter display
/// (`` `;` ``, `` `\n` ``, etc.).
fn assert_block_metachar(code: Option<i32>, stdout: &str, expected_display: &str) {
    let parsed = assert_block(code, stdout);
    let reason = parsed["reason"].as_str().unwrap();
    assert!(
        reason.contains("metacharacter"),
        "reason must identify the metacharacter rule, got: {reason}"
    );
    assert!(
        reason.contains(expected_display),
        "reason must name the rejected metacharacter {expected_display}, got: {reason}"
    );
}

// --- allow forms (exit 0, empty stdout) ---

#[test]
fn allows_codebus_lint_bare() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint"));
    assert_allow(code, &stdout);
}

#[test]
fn allows_codebus_lint_with_flags() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint --format json"));
    assert_allow(code, &stdout);
}

#[test]
fn allows_codebus_lint_with_vault_path() {
    // `codebus lint <vault>` — the canonical fix-sandbox invocation. The
    // path contains no shell metacharacter, so it passes the screen.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint --repo /some/vault"));
    assert_allow(code, &stdout);
}

#[test]
fn allows_codebus_quiz_validate() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus quiz validate -"));
    assert_allow(code, &stdout);
}

#[test]
fn allows_codebus_quiz_validate_with_args() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus quiz validate draft.md --json"));
    assert_allow(code, &stdout);
}

#[test]
fn allows_single_quoted_quiz_validate_heredoc() {
    // The 2026-05-29 quiz-heredoc-selfvalidate-unblock allow form: the
    // codebus-quiz Mode B agent self-validates by piping its draft into
    // `codebus quiz validate -` via a single-quoted here-document. The `<`
    // and LF bytes are part of the raw command string yet MUST be allowed
    // because the body is opaque stdin.
    let cmd = "codebus quiz validate - <<'CBQZ'\n\
               ## Q1. What is a vault?\n\
               A) a folder\n\
               B) a database\n\
               ## Answer: A\n\
               ## Explanation: see [[vault]].\n\
               CBQZ";
    let (code, stdout, _) = run_check_bash(&bash_body(cmd));
    assert_allow(code, &stdout);
}

#[test]
fn allows_single_quoted_quiz_validate_heredoc_json_flag() {
    let cmd = "codebus quiz validate --json - <<'CBQZ'\n\
               ## Q1. stem\n\
               ## Answer: A\n\
               CBQZ";
    let (code, stdout, _) = run_check_bash(&bash_body(cmd));
    assert_allow(code, &stdout);
}

// --- block forms: non-whitelisted commands ---

#[test]
fn blocks_non_codebus_command() {
    // `echo hi` carries no metacharacter, so it hits the sandbox-allowlist
    // reason, NOT the metacharacter reason.
    let (code, stdout, _) = run_check_bash(&bash_body("echo hi"));
    let parsed = assert_block(code, &stdout);
    let reason = parsed["reason"].as_str().unwrap();
    assert!(
        reason.contains("permitted by the codebus agent sandbox"),
        "non-allowlist command must surface the sandbox-allowlist reason, got: {reason}"
    );
}

#[test]
fn blocks_codebus_non_whitelisted_subcommand() {
    // `codebus fix` is a real codebus verb but NOT on the hook allowlist
    // (only `lint` and `quiz validate`). No metacharacter present.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus fix --no-fix"));
    let parsed = assert_block(code, &stdout);
    assert!(
        parsed["reason"]
            .as_str()
            .unwrap()
            .contains("permitted by the codebus agent sandbox")
    );
}

#[test]
fn blocks_codebus_quiz_generate_form() {
    // `codebus quiz "topic"` (generate) is NOT the validate sub-action.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus quiz topic"));
    assert_block(code, &stdout);
}

// --- block forms: shell metacharacter bypass attempts ---
// One test per metacharacter in SHELL_METACHARACTERS. Each command is
// crafted so the targeted metacharacter is the FIRST one scanned, so the
// hook's reason names it (find_shell_metacharacter returns the first hit).

#[test]
fn blocks_metachar_semicolon() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint; whoami"));
    assert_block_metachar(code, &stdout, "`;`");
}

#[test]
fn blocks_metachar_ampersand() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint && rm -rf /tmp/x"));
    assert_block_metachar(code, &stdout, "`&`");
}

#[test]
fn blocks_metachar_pipe() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint | tee /tmp/leak"));
    assert_block_metachar(code, &stdout, "`|`");
}

#[test]
fn blocks_metachar_dollar() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint $(whoami)"));
    assert_block_metachar(code, &stdout, "`$`");
}

#[test]
fn blocks_metachar_backtick() {
    // The backtick metachar's display is wrapped in backticks too
    // (`` `\u{60}` ``), so assert the rule + the bare backtick byte rather
    // than a backtick-delimited display string to avoid escaping confusion.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint `whoami`"));
    let parsed = assert_block(code, &stdout);
    let reason = parsed["reason"].as_str().unwrap();
    assert!(
        reason.contains("metacharacter"),
        "reason must identify the metacharacter rule, got: {reason}"
    );
    assert!(
        reason.contains('`'),
        "backtick metachar reason must contain a backtick, got: {reason}"
    );
}

#[test]
fn blocks_metachar_redirect_out() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint > /tmp/out"));
    assert_block_metachar(code, &stdout, "`>`");
}

#[test]
fn blocks_metachar_redirect_in_non_heredoc() {
    // A single `<` input redirection (NOT a here-document) MUST block.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint < /etc/passwd"));
    assert_block_metachar(code, &stdout, "`<`");
}

#[test]
fn blocks_metachar_open_paren() {
    let (code, stdout, _) = run_check_bash(&bash_body("(codebus lint)"));
    assert_block_metachar(code, &stdout, "`(`");
}

#[test]
fn blocks_metachar_close_paren() {
    // Craft so `)` is the first metacharacter scanned (no `(` before it).
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint foo)"));
    assert_block_metachar(code, &stdout, "`)`");
}

#[test]
fn blocks_metachar_newline() {
    // Embedded LF could split into two commands under shell eval; the hook
    // renders it as the escape sequence `\n` in the reason.
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint\nrm -rf /tmp"));
    assert_block_metachar(code, &stdout, "`\\n`");
}

#[test]
fn blocks_metachar_carriage_return() {
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint\rrm -rf /tmp"));
    assert_block_metachar(code, &stdout, "`\\r`");
}

#[test]
fn blocks_metachar_inside_quotes() {
    // Quote-awareness is deliberately NOT implemented — a metachar inside
    // quotes still blocks (byte-level scan of the raw command string).
    let (code, stdout, _) = run_check_bash(&bash_body("codebus lint --filter \"foo;bar\""));
    assert_block_metachar(code, &stdout, "`;`");
}

#[test]
fn blocks_unquoted_heredoc_marker() {
    // An unquoted marker permits shell expansion in the body — the heredoc
    // exception MUST NOT apply, and the `<` metacharacter blocks it.
    let cmd = "codebus quiz validate - <<CBQZ\n\
               ## Q1. stem\n\
               CBQZ";
    let (code, stdout, _) = run_check_bash(&bash_body(cmd));
    assert_block(code, &stdout);
}

#[test]
fn blocks_heredoc_with_trailing_command() {
    // Well-formed heredoc but a command follows the closing delimiter.
    let cmd = "codebus quiz validate - <<'CBQZ'\n\
               ## Q1. stem\n\
               CBQZ\n\
               rm -rf ~";
    let (code, stdout, _) = run_check_bash(&bash_body(cmd));
    assert_block(code, &stdout);
}

// --- fail-closed branches (exit 0, block) ---

#[test]
fn fail_closed_on_empty_stdin() {
    let (code, stdout, _) = run_check_bash("");
    let parsed = assert_block(code, &stdout);
    assert!(
        parsed["reason"].as_str().unwrap().contains("empty stdin"),
        "empty stdin must surface the empty-stdin fail-closed reason"
    );
}

#[test]
fn fail_closed_on_whitespace_only_stdin() {
    let (code, stdout, _) = run_check_bash("   \n\t  ");
    assert_block(code, &stdout);
}

#[test]
fn fail_closed_on_malformed_json() {
    let (code, stdout, _) = run_check_bash("{not valid json");
    let parsed = assert_block(code, &stdout);
    assert!(
        parsed["reason"].as_str().unwrap().contains("malformed"),
        "malformed JSON must surface the malformed-JSON fail-closed reason"
    );
}

#[test]
fn fail_closed_on_missing_command() {
    // tool_input present but no `command` key.
    let body = r#"{"tool_name":"Bash","tool_input":{}}"#;
    let (code, stdout, _) = run_check_bash(body);
    let parsed = assert_block(code, &stdout);
    assert!(
        parsed["reason"].as_str().unwrap().contains("command"),
        "missing command must surface the absent-command fail-closed reason"
    );
}

#[test]
fn fail_closed_on_missing_tool_input() {
    let body = r#"{"tool_name":"Bash"}"#;
    let (code, stdout, _) = run_check_bash(body);
    assert_block(code, &stdout);
}

#[test]
fn fail_closed_on_empty_command() {
    let body = r#"{"tool_name":"Bash","tool_input":{"command":""}}"#;
    let (code, stdout, _) = run_check_bash(body);
    let parsed = assert_block(code, &stdout);
    assert!(parsed["reason"].as_str().unwrap().contains("command"));
}
