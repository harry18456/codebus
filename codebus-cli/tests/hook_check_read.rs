//! Integration tests for the `codebus hook check-read` PreToolUse Read
//! hook (spec `lint-feedback-loop` / PII Image Read Hook Installation).
//!
//! These tests spawn the real `codebus` binary, feed it a PreToolUse
//! JSON body on stdin (matching Claude Code's hook input schema), and
//! assert the decision-JSON contract on stdout. Unit tests for the
//! pure predicate / decision logic live alongside the implementation
//! in `codebus-cli/src/commands/hook.rs`; this file pins the
//! subprocess-level contract end-to-end.

use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Run `codebus hook check-read` with `body` on stdin; return (exit code,
/// stdout, stderr).
fn run_check_read(body: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(BIN)
        .args(["hook", "check-read"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn codebus hook check-read");
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

fn assert_block(stdout: &str) -> serde_json::Value {
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

fn assert_allow(stdout: &str) {
    assert!(
        stdout.trim().is_empty(),
        "allow path must produce empty stdout, got `{stdout}`"
    );
}

#[test]
fn blocks_png_image() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
    let (code, stdout, _stderr) = run_check_read(body);
    assert_eq!(code, Some(0), "exit code must be 0 (decision via stdout)");
    let parsed = assert_block(&stdout);
    assert!(
        parsed["reason"].as_str().unwrap().contains("flow.png"),
        "reason must echo the blocked path"
    );
}

#[test]
fn blocks_uppercase_jpg() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/logo.JPG"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn blocks_pdf() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"docs/manual.pdf"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn blocks_heic_uppercase_iphone_format() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"photos/IMG_001.HEIC"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn blocks_windows_path_with_backslashes() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"C:\\repo\\assets\\img.png"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn allows_markdown_text() {
    let body =
        r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/modules/uv-lib.md"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_allow(&stdout);
}

#[test]
fn allows_rust_source() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"codebus-core/src/agent/claude_cli.rs"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_allow(&stdout);
}

#[test]
fn allows_svg() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagram.svg"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_allow(&stdout);
}

#[test]
fn allows_no_extension() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"Makefile"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_allow(&stdout);
}

#[test]
fn fail_closed_on_empty_stdin() {
    let (code, stdout, _) = run_check_read("");
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn fail_closed_on_malformed_json() {
    let (code, stdout, _) = run_check_read("{not valid json");
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn fail_closed_on_missing_file_path() {
    let body = r#"{"tool_name":"Read","tool_input":{}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn fail_closed_on_empty_file_path() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":""}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn fail_closed_on_non_string_file_path() {
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":42}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}
