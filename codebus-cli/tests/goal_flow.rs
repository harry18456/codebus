//! Integration tests for `codebus goal` end-to-end flow (v3-goal #5).
//!
//! These tests drive the real codebus binary against tempdir repos but
//! substitute a Rust-built mock binary (tests/bins/mock_claude.rs) for the
//! claude CLI via the `CODEBUS_CLAUDE_BIN` env override hook on
//! `agent::claude_cli::invoke`. This keeps tests deterministic, fast, and
//! independent of any real claude installation / token cost / network.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Run `codebus --no-obsidian-register goal <goal_text> [<extra_flags>...]`
/// against `repo`, with mock-claude wired in via env. Returns the binary's
/// Output and the path the mock wrote its argv/cwd dump to.
fn run_goal(
    repo: &Path,
    goal_text: &str,
    extra_flags: &[&str],
    behavior: &str,
) -> (Output, std::path::PathBuf) {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let mut cmd = Command::new(BIN);
    cmd.args(["--no-obsidian-register", "goal", goal_text]);
    for f in extra_flags {
        cmd.arg(f);
    }
    cmd.current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log);
    let out = cmd.output().expect("run codebus goal");
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
fn goal_spawns_agent_with_canonical_sandbox_args() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (out, log) = run_goal(tmp.path(), "test", &[], "success-noop");
    assert!(
        out.status.success(),
        "goal should propagate mock success-noop exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let body = fs::read_to_string(&log).expect("mock log written");
    let lines: Vec<&str> = body.lines().collect();

    // cwd is the vault root.
    let vault_str = tmp.path().join(".codebus").to_string_lossy().to_string();
    let cwd_line = lines.iter().find(|l| l.starts_with("cwd=")).expect("cwd line");
    let cwd_value = cwd_line.strip_prefix("cwd=").unwrap();
    // tolerate Windows extended-length prefix (`\\?\D:\...` etc.) by ends_with check
    assert!(
        cwd_value.ends_with(vault_str.trim_start_matches("\\\\?\\")),
        "cwd `{cwd_value}` should end with vault path `{vault_str}`"
    );

    // Triple-flag sandbox args present in declared order around their values.
    let arg_lines: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("arg="))
        .collect();
    assert!(arg_lines.contains(&"-p"), "missing -p in {arg_lines:?}");
    assert!(
        arg_lines.iter().any(|a| a.starts_with("/codebus-goal ")),
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
    assert!(
        arg_lines.contains(&"Read,Glob,Grep,Write,Edit"),
        "missing toolset CSV in {arg_lines:?}"
    );
    // Toolset CSV appears twice (once for --tools, once for --allowedTools).
    let toolset_count = arg_lines
        .iter()
        .filter(|a| **a == "Read,Glob,Grep,Write,Edit")
        .count();
    assert_eq!(
        toolset_count, 2,
        "toolset CSV should appear twice (--tools + --allowedTools), got {toolset_count}"
    );
}

#[test]
fn goal_auto_inits_when_vault_missing() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    // Note: NO prior init. Vault does not exist.
    assert!(!tmp.path().join(".codebus").exists());

    let (out, log) = run_goal(tmp.path(), "first", &[], "success-noop");
    assert!(
        out.status.success(),
        "goal should auto-init then succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Auto-init has run: vault, nested git, init commit all present.
    let vault = tmp.path().join(".codebus");
    assert!(vault.is_dir(), "vault not created by auto-init");
    assert!(vault.join(".git").is_dir(), "nested .git/ not created");

    let log_out = git(&vault, &["log", "--pretty=%s"]);
    let messages = String::from_utf8_lossy(&log_out.stdout);
    assert!(
        messages.lines().any(|l| l == "init: codebus vault"),
        "missing init commit: {messages}"
    );

    // Mock was still spawned afterwards (auto-init was not the only thing).
    let body = fs::read_to_string(&log).expect("mock log");
    assert!(body.contains("/codebus-goal \"first\""), "mock log: {body}");
}

#[test]
fn goal_auto_commits_partial_writes_on_agent_failure() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    let (out, _log) =
        run_goal(tmp.path(), "test", &[], "failure-write-then-exit-1");

    // codebus propagates the agent's non-zero exit.
    assert!(
        !out.status.success(),
        "goal should propagate mock exit 1; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(out.status.code(), Some(1));

    let vault = tmp.path().join(".codebus");

    // Partial write from mock landed in the vault.
    assert!(
        vault.join("wiki/concepts/partial.md").is_file(),
        "expected partial wiki page to exist after mock-claude failure"
    );

    // Latest nested-git commit message reflects the goal even though the
    // child failed (v2 carry: commit on failure preserves partial work).
    let log_out = git(&vault, &["log", "--pretty=%s"]);
    let messages = String::from_utf8_lossy(&log_out.stdout);
    let latest = messages.lines().next().unwrap_or("");
    assert_eq!(
        latest, "wiki: test",
        "expected latest commit `wiki: test`, got `{latest}` (full log: {messages})"
    );

    // Working tree clean after auto_commit captured the partial write.
    let porcelain = git(&vault, &["status", "--porcelain"]);
    assert!(
        String::from_utf8_lossy(&porcelain.stdout).trim().is_empty(),
        "expected clean working tree after partial-failure auto_commit"
    );
}

#[test]
fn goal_force_resync_bypasses_detection() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    // First goal without --force-resync. Source signal matches what init wrote
    // → detection reports unchanged → no re-sync line in stdout.
    let (out1, _log1) = run_goal(tmp.path(), "first", &[], "success-noop");
    assert!(out1.status.success());
    let stdout1 = String::from_utf8_lossy(&out1.stdout);
    assert!(
        !stdout1.contains("(re-sync)"),
        "first goal w/o force-resync unexpectedly re-synced; stdout: {stdout1}"
    );

    // Second goal with --force-resync. SHALL re-sync regardless of detection.
    let (out2, _log2) =
        run_goal(tmp.path(), "second", &["--force-resync"], "success-noop");
    assert!(
        out2.status.success(),
        "second goal stderr: {}",
        String::from_utf8_lossy(&out2.stderr)
    );
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    assert!(
        stdout2.contains("(re-sync)"),
        "force-resync did not produce re-sync progress line; stdout: {stdout2}"
    );
}
