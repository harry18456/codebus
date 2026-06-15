//! Integration tests for the `codebus hook check-read` PreToolUse Read
//! hook (spec `lint-feedback-loop` / PII Image Read Hook Installation).
//!
//! These tests spawn the real `codebus` binary, feed it a PreToolUse
//! JSON body on stdin (matching Claude Code's hook input schema), and
//! assert the decision-JSON contract on stdout. Unit tests for the
//! pure predicate / decision logic live alongside the implementation
//! in `codebus-cli/src/commands/hook.rs`; this file pins the
//! subprocess-level contract end-to-end.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Run `codebus hook check-read` with `body` on stdin; return (exit code,
/// stdout, stderr).
fn run_check_read(body: &str) -> (Option<i32>, String, String) {
    run_check_read_with_home(body, None)
}

/// Run `codebus hook check-read` with `body` on stdin and an optional
/// `CODEBUS_HOME` override pointing at a temp dir containing the yaml
/// config to load. When `home` is None, no override is set (the binary
/// reads the real `~/.codebus/config.yaml`, if any).
fn run_check_read_with_home(body: &str, home: Option<&Path>) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", "check-read"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(h) = home {
        cmd.env("CODEBUS_HOME", h);
    }
    let mut child = cmd.spawn().expect("spawn codebus hook check-read");
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

/// Write a yaml body at `<home>/.codebus/config.yaml`.
fn write_codebus_config(home: &Path, yaml: &str) {
    let cfg_dir = home.join(".codebus");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("config.yaml"), yaml).unwrap();
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
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/modules/uv-lib.md"}}"#;
    let (code, stdout, _) = run_check_read(body);
    assert_eq!(code, Some(0));
    assert_allow(&stdout);
}

#[test]
fn allows_rust_source() {
    let body =
        r#"{"tool_name":"Read","tool_input":{"file_path":"codebus-core/src/agent/claude_cli.rs"}}"#;
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
fn blocks_sensitive_key_basename_variants() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(
        home.path(),
        "hooks:\n  read_image_block: true\n  read_path_containment: true\n",
    );
    for path in [
        "raw/code/server.pem",
        "raw/code/server.PEM",
        "raw/code/private.key",
        "raw/code/private.KEY",
        "raw/code/id_rsa",
        "raw/code/ID_RSA",
        "raw/code/backup-ID_RSA.txt",
    ] {
        let body = format!(r#"{{"tool_name":"Read","tool_input":{{"file_path":"{path}"}}}}"#);
        let (code, stdout, _) = run_check_read_with_home(&body, Some(home.path()));
        assert_eq!(code, Some(0), "exit code for {path}");
        let parsed = assert_block(&stdout);
        assert!(
            parsed["reason"]
                .as_str()
                .unwrap_or_default()
                .contains("basename"),
            "reason must name basename rule for {path}: {stdout}"
        );
    }
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

// --- verify-stage-independent-model-toggle task 2.4 ---
// Spec scenarios on the runtime gate `hooks.read_image_block`:
//
// 1. Disabled config: hook ALWAYS allows regardless of stdin.
// 2. Missing config file: hook falls back to default true (block).
// 3. Absent hooks section: hook falls back to default true (block).

#[test]
fn config_false_allows_image_extension() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_image_block: false\n");
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert!(
        stdout.trim().is_empty(),
        "hooks.read_image_block=false must produce empty stdout (allow); got `{stdout}`"
    );
}

#[test]
fn config_false_allows_non_image() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_image_block: false\n");
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/modules/uv-lib.md"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert!(stdout.trim().is_empty());
}

#[test]
fn config_false_short_circuits_malformed_stdin() {
    // check-read-vault-containment: the denylist (read_image_block) AND the
    // containment (read_path_containment) gates are independent. To fully
    // short-circuit malformed stdin (which either gate would otherwise
    // fail-closed → block), BOTH gates must be off.
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(
        home.path(),
        "hooks:\n  read_image_block: false\n  read_path_containment: false\n",
    );
    let (code, stdout, _) = run_check_read_with_home("{not valid json", Some(home.path()));
    assert_eq!(code, Some(0));
    assert!(stdout.trim().is_empty());
}

// --- check-read-vault-containment: end-to-end containment via the real
// binary. The vault root comes from the PreToolUse `cwd` field supplied in
// the body. read_path_containment defaults on (explicit true here).

#[test]
fn containment_blocks_out_of_vault_read_via_binary() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_path_containment: true\n");
    let vault = TempDir::new().expect("vault");
    let vault_fs = vault.path().to_string_lossy().replace('\\', "/");
    let outside = vault.path().parent().unwrap().join("secret.txt");
    let outside_fs = outside.to_string_lossy().replace('\\', "/");
    let body = format!(
        r#"{{"tool_name":"Read","cwd":"{vault_fs}","tool_input":{{"file_path":"{outside_fs}"}}}}"#
    );
    let (code, stdout, _) = run_check_read_with_home(&body, Some(home.path()));
    assert_eq!(code, Some(0));
    let v = assert_block(&stdout);
    assert!(
        v["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("vault-containment"),
        "block reason must name vault-containment: {stdout}"
    );
}

#[test]
fn containment_allows_in_vault_read_via_binary() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_path_containment: true\n");
    let vault = TempDir::new().expect("vault");
    let vault_fs = vault.path().to_string_lossy().replace('\\', "/");
    let body = format!(
        r#"{{"tool_name":"Read","cwd":"{vault_fs}","tool_input":{{"file_path":"raw/code/x.rs"}}}}"#
    );
    let (code, stdout, _) = run_check_read_with_home(&body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert!(
        stdout.trim().is_empty(),
        "in-vault relative read must allow: {stdout}"
    );
}

#[test]
fn containment_blocks_out_of_vault_grep_path_via_binary() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_path_containment: true\n");
    let vault = TempDir::new().expect("vault");
    let vault_fs = vault.path().to_string_lossy().replace('\\', "/");
    let outside_fs = vault
        .path()
        .parent()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let body = format!(
        r#"{{"tool_name":"Grep","cwd":"{vault_fs}","tool_input":{{"pattern":"x","path":"{outside_fs}"}}}}"#
    );
    let (code, stdout, _) = run_check_read_with_home(&body, Some(home.path()));
    assert_eq!(code, Some(0));
    let v = assert_block(&stdout);
    assert!(
        v["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("vault-containment"),
        "Grep out-of-vault must block: {stdout}"
    );
}

#[test]
fn config_true_blocks_image_extension() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  read_image_block: true\n");
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"docs/manual.pdf"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn absent_hooks_section_falls_back_to_block() {
    // Yaml exists, parses fine, but has no hooks section. Per spec,
    // this resolves to the default (read_image_block=true → block).
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "lint:\n  fix:\n    enabled: true\n");
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/img.png"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn missing_config_file_falls_back_to_block() {
    // CODEBUS_HOME points at a directory with no .codebus/config.yaml.
    // The hook subcommand SHALL still block image reads (fail-safe).
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    // Intentionally do NOT call write_codebus_config — no config file.
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/img.png"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}

#[test]
fn malformed_yaml_falls_back_to_block() {
    let home = TempDir::new().expect("tmp CODEBUS_HOME");
    write_codebus_config(home.path(), "hooks:\n  : :: not yaml\n");
    let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/img.png"}}"#;
    let (code, stdout, _) = run_check_read_with_home(body, Some(home.path()));
    assert_eq!(code, Some(0));
    assert_block(&stdout);
}
