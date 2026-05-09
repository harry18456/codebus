//! Integration tests for `codebus query` end-to-end flow (v3-query #6).
//!
//! Drives the real codebus binary against tempdir repos but substitutes
//! the test-only mock-claude binary (built from tests/bins/mock_claude.rs
//! by v3-goal #5) via the `CODEBUS_CLAUDE_BIN` env override hook on
//! `agent::claude_cli::invoke`.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Run `codebus query <text>` against `repo`, with mock-claude wired in.
/// Returns the binary's Output and the path the mock wrote its argv/cwd
/// dump to.
fn run_query(repo: &Path, query_text: &str, behavior: &str) -> (Output, std::path::PathBuf) {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let out = Command::new(BIN)
        .args(["query", query_text])
        .current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log)
        .output()
        .expect("run codebus query");
    (out, log)
}

fn run_init(repo: &Path) -> Output {
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(repo)
        .output()
        .expect("run codebus init")
}

fn git(vault: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(vault)
        .output()
        .expect("run git")
}

#[test]
fn query_spawns_agent_with_read_only_toolset() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, log) = run_query(tmp.path(), "what is this", "success-noop");
    assert!(
        out.status.success(),
        "query should propagate mock success-noop exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let body = fs::read_to_string(&log).expect("mock log written");
    let lines: Vec<&str> = body.lines().collect();

    // cwd is the vault root.
    let vault_str = tmp.path().join(".codebus").to_string_lossy().to_string();
    let cwd_line = lines.iter().find(|l| l.starts_with("cwd=")).expect("cwd line");
    let cwd_value = cwd_line.strip_prefix("cwd=").unwrap();
    assert!(
        cwd_value.ends_with(vault_str.trim_start_matches("\\\\?\\")),
        "cwd `{cwd_value}` should end with vault path `{vault_str}`"
    );

    let arg_lines: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("arg="))
        .collect();
    assert!(arg_lines.contains(&"-p"), "missing -p");
    assert!(
        arg_lines
            .iter()
            .any(|a| a.starts_with("/codebus-query ")),
        "missing slash command in {arg_lines:?}"
    );
    assert!(arg_lines.contains(&"--tools"), "missing --tools");
    assert!(arg_lines.contains(&"--allowedTools"), "missing --allowedTools");
    assert!(
        arg_lines.contains(&"--permission-mode"),
        "missing --permission-mode"
    );
    assert!(
        arg_lines.contains(&"acceptEdits"),
        "missing acceptEdits value"
    );

    // Read-only toolset: exact CSV match `Read,Glob,Grep` appears twice
    // (once for --tools, once for --allowedTools). Exact equality is the
    // assertion — it implicitly excludes Write / Edit / Bash because any
    // such inclusion would change the CSV literal and the count would
    // drop below 2. Substring `contains("Edit")` is unsafe because
    // `acceptEdits` (the permission-mode value) trivially contains it.
    let toolset_count = arg_lines
        .iter()
        .filter(|a| **a == "Read,Glob,Grep")
        .count();
    assert_eq!(
        toolset_count, 2,
        "read-only toolset CSV `Read,Glob,Grep` should appear twice (--tools + --allowedTools), got {toolset_count}; arg_lines={arg_lines:?}"
    );
    // No CSV arg containing Write or Bash (Edit would be inside acceptEdits;
    // Write / Bash never appear in the permission-mode value).
    for csv_arg in arg_lines.iter().filter(|a| a.contains(',')) {
        assert!(
            !csv_arg.contains("Write") && !csv_arg.contains("Bash"),
            "toolset CSV `{csv_arg}` must not contain Write or Bash"
        );
    }
}

#[test]
fn query_refuses_when_vault_missing() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    // Note: NO init. Vault does not exist.
    assert!(!tmp.path().join(".codebus").exists());

    let (out, log) = run_query(tmp.path(), "ignored", "success-noop");

    assert!(
        !out.status.success(),
        "query against missing vault should fail; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing vault SHALL exit with status 2"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("vault not found"),
        "stderr should mention vault not found, got: {stderr}"
    );
    assert!(
        stderr.contains("codebus init"),
        "stderr should instruct user to run codebus init, got: {stderr}"
    );

    // Mock log must NOT exist — agent should not have been spawned.
    assert!(
        !log.exists(),
        "mock-claude log {} should not exist when vault is missing",
        log.display()
    );
}

#[test]
fn query_does_not_auto_commit() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    let vault = tmp.path().join(".codebus");

    // Capture nested git rev-list count BEFORE query.
    let before_out = git(&vault, &["rev-list", "--count", "HEAD"]);
    let count_before: usize = String::from_utf8_lossy(&before_out.stdout)
        .trim()
        .parse()
        .expect("rev-list count parse");

    // Run query — mock noop, no writes.
    let (out, _log) = run_query(tmp.path(), "test", "success-noop");
    assert!(out.status.success());

    // Count AFTER query SHALL equal count before — no auto-commit happened.
    let after_out = git(&vault, &["rev-list", "--count", "HEAD"]);
    let count_after: usize = String::from_utf8_lossy(&after_out.stdout)
        .trim()
        .parse()
        .expect("rev-list count parse");
    assert_eq!(
        count_before, count_after,
        "query must not produce a new commit (count: {count_before} -> {count_after})"
    );

    // Working tree clean — query must not leave dirty state.
    let porcelain = git(&vault, &["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
        "query must leave vault working tree clean"
    );
}

#[test]
fn query_propagates_agent_exit_code() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    // mock behavior `failure-write-then-exit-1`: mock attempts to write
    // and exits with code 1. The actual write may or may not succeed
    // depending on whether the read-only sandbox in production blocks it
    // (mock-claude itself is a separate process not gated by --tools, so
    // the write side-effect lands in the vault for the test to observe).
    // The test focus here is: codebus binary propagates the child's
    // non-zero exit code regardless of what the mock did.
    let (out, _log) =
        run_query(tmp.path(), "test", "failure-write-then-exit-1");

    assert!(
        !out.status.success(),
        "query should propagate mock exit 1; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(out.status.code(), Some(1));
}
