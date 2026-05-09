//! Integration tests for `codebus fix` end-to-end flow (v3-lint Section 9).
//!
//! Tests drive the real codebus binary against tempdir vaults but substitute
//! a mock claude binary for the agent CLI via CODEBUS_CLAUDE_BIN, so they
//! don't depend on a real claude install / network / token quota.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

fn run_init(repo: &Path) -> Output {
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(repo)
        .output()
        .expect("run init")
}

fn run_fix(repo: &Path, extra_flags: &[&str], behavior: &str) -> Output {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let mut cmd = Command::new(BIN);
    cmd.arg("fix");
    for f in extra_flags {
        cmd.arg(f);
    }
    cmd.current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log);
    cmd.output().expect("run codebus fix")
}

fn write_page(vault: &Path, rel: &str, frontmatter: &str, body: &str) {
    let full = vault.join("wiki").join(rel);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    let content = format!("---\n{frontmatter}---\n{body}");
    fs::write(full, content).unwrap();
}

fn fm_clean() -> &'static str {
    "title: foo\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-09'\nupdated: '2026-05-09'\nrelated: []\nstale: false\n"
}

#[test]
fn fix_refuses_when_vault_is_missing() {
    let tmp = TempDir::new().unwrap();
    let out = run_fix(tmp.path(), &[], "success-noop");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no codebus vault"));
    assert!(stderr.contains("codebus init"));
}

#[test]
fn fix_exits_zero_on_clean_vault_without_spawning_agent() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success(), "init failed");
    // Fresh init produces nav-missing warnings (index.md / log.md absent).
    // Plant them so the vault is genuinely lint-clean before fix runs.
    let vault = tmp.path().join(".codebus");
    fs::write(vault.join("wiki/index.md"), "# index\n").unwrap();
    fs::write(vault.join("wiki/log.md"), "# log\n").unwrap();
    let out = run_fix(tmp.path(), &[], "success-noop");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("vault already clean"),
        "expected initial-clean short-circuit message, got stdout: {stdout}"
    );
    // Mock log should NOT exist — agent was never spawned.
    let log = tmp.path().join("mock-claude.log");
    assert!(!log.exists(), "agent unexpectedly spawned on clean vault");
}

#[test]
fn fix_no_fix_flag_short_circuits() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Add lint issue, but --no-fix should still exit 0 without spawning agent.
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = run_fix(tmp.path(), &["--no-fix"], "success-noop");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("disabled"));
    let log = tmp.path().join("mock-claude.log");
    assert!(!log.exists(), "agent should not spawn under --no-fix");
}

#[test]
fn fix_max_iter_flag_overrides_default() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Plant an unrepairable issue so the loop pings until budget exhausted.
    let fm = "title: foo\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-09'\nupdated: '2026-05-09'\nrelated:\n  - '[[ghost]]'\nstale: false\n";
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm, "# foo");
    // mock-claude success-noop: agent exits 0 without modifying anything.
    // With --fix-max-iter 0, the loop spawns once initially, then since
    // ping cap is 0 it terminates immediately on next lint check.
    let out = run_fix(tmp.path(), &["--fix-max-iter", "0"], "success-noop");
    // Issues remain → exit 1
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exhausted ping budget"),
        "expected ping-budget exhaustion message, got: {stderr}"
    );
}

#[test]
fn fix_inherits_session_id_flag_in_spawn() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Plant a lint issue → triggers agent spawn.
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = run_fix(tmp.path(), &["--fix-max-iter", "0"], "success-noop");
    let _ = out;
    let log = tmp.path().join("mock-claude.log");
    assert!(log.exists(), "agent should have been spawned");
    let body = fs::read_to_string(&log).unwrap();
    let arg_lines: Vec<&str> = body.lines().filter_map(|l| l.strip_prefix("arg=")).collect();
    // First spawn passes --session-id (Start session).
    assert!(
        arg_lines.contains(&"--session-id"),
        "fix initial spawn missing --session-id; args: {arg_lines:?}"
    );
}

#[test]
fn fix_spawn_uses_bare_bash_in_tools_and_restricted_in_allowed_tools() {
    // Per claude --help v2.1.137 spike: --tools needs the bare `Bash`
    // token (toolset hard-gate) for the agent to have Bash at all; the
    // fine-grained `Bash(codebus lint *)` pattern belongs in --allowedTools
    // (auto-approval scope). Mixing them up results in "no Bash tool"
    // from the agent OR unrestricted Bash auto-approval.
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = run_fix(tmp.path(), &["--fix-max-iter", "0"], "success-noop");
    let _ = out;
    let log = tmp.path().join("mock-claude.log");
    assert!(log.exists(), "agent should have been spawned");
    let body = fs::read_to_string(&log).unwrap();
    let arg_lines: Vec<String> = body.lines().filter_map(|l| l.strip_prefix("arg=").map(String::from)).collect();

    // Find the value that follows --tools and the value that follows --allowedTools.
    let tools_idx = arg_lines.iter().position(|a| a == "--tools").expect("--tools flag missing");
    let allowed_idx = arg_lines.iter().position(|a| a == "--allowedTools").expect("--allowedTools flag missing");
    let tools_val = arg_lines.get(tools_idx + 1).expect("--tools value missing");
    let allowed_val = arg_lines.get(allowed_idx + 1).expect("--allowedTools value missing");

    // --tools must contain bare Bash and NOT contain the restriction.
    assert!(tools_val.contains(",Bash") || tools_val == "Bash" || tools_val.starts_with("Bash,"),
        "--tools value missing bare `Bash`: `{tools_val}`");
    assert!(!tools_val.contains("Bash("),
        "--tools value should not contain the Bash(...) restriction: `{tools_val}`");

    // --allowedTools must contain the restriction pattern.
    assert!(allowed_val.contains("Bash(codebus lint *)"),
        "--allowedTools missing Bash(codebus lint *) restriction: `{allowed_val}`");
}
