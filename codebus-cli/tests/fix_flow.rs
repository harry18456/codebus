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
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo)
        .output()
        .expect("run init")
}

fn run_fix(repo: &Path, extra_flags: &[&str], behavior: &str) -> Output {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let mut cmd = Command::new(BIN);
    cmd.arg("fix");
    for f in extra_flags {
        cmd.arg(f);
    }
    cmd.current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
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
    // v3-render-polish: detail line `✓ fix: vault already clean` is now
    // debug-mode-only. Default mode emits banner sequence; clean vault
    // shows LintDone with `0 errors, 0 warnings` instead.
    assert!(
        stdout.contains("0 errors") && stdout.contains("0 warnings"),
        "expected LintDone banner with zero counts on clean vault, got: {stdout}"
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
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
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

/// v3-fix-trust-agent: --fix-max-iter is no longer a recognized flag.
#[test]
fn fix_max_iter_flag_rejected_by_clap() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["fix", "--fix-max-iter", "5"])
        .current_dir(tmp.path())
        .output()
        .expect("run codebus fix");
    assert!(!out.status.success(), "--fix-max-iter should be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // clap emits "unexpected argument" or similar for unknown flags
    assert!(
        stderr.contains("unexpected")
            || stderr.contains("unrecognized")
            || stderr.contains("error"),
        "expected clap rejection of --fix-max-iter; stderr: {stderr}"
    );
}

/// v3-fix-trust-agent Fix Single-Shot Verification scenario:
/// "Fix spawn arguments contain no session continuity flags"
#[test]
fn fix_spawn_does_not_pass_session_continuity_flags() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
    let _ = run_fix(tmp.path(), &[], "success-noop");
    let log = tmp.path().join("mock-claude.log");
    assert!(log.exists(), "agent should have been spawned");
    let body = fs::read_to_string(&log).unwrap();
    let arg_lines: Vec<&str> = body
        .lines()
        .filter_map(|l| l.strip_prefix("arg="))
        .collect();
    assert!(
        !arg_lines.contains(&"--session-id"),
        "fix spawn must not pass --session-id; args: {arg_lines:?}"
    );
    assert!(
        !arg_lines.contains(&"--resume"),
        "fix spawn must not pass --resume; args: {arg_lines:?}"
    );
    assert!(
        !arg_lines.contains(&"--continue"),
        "fix spawn must not pass --continue; args: {arg_lines:?}"
    );
}

/// v3-fix-trust-agent Fix Single-Shot Verification scenario:
/// "Fix spawns the agent exactly once on dirty vault"
#[test]
fn fix_spawns_agent_exactly_once() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Plant lint issue → triggers spawn. mock-claude success-noop won't
    // repair anything → final lint still has issue. Verify single spawn.
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
    let _ = run_fix(tmp.path(), &[], "success-noop");
    let log = tmp.path().join("mock-claude.log");
    let body = fs::read_to_string(&log).unwrap();
    // mock-claude.log uses one block per invocation; count distinct cwd= lines.
    let cwd_count = body.lines().filter(|l| l.starts_with("cwd=")).count();
    assert_eq!(
        cwd_count, 1,
        "fix should spawn agent exactly once; mock log has {cwd_count} invocations:\n{body}"
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
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
    let _ = run_fix(tmp.path(), &[], "success-noop");
    let log = tmp.path().join("mock-claude.log");
    assert!(log.exists(), "agent should have been spawned");
    let body = fs::read_to_string(&log).unwrap();
    let arg_lines: Vec<String> = body
        .lines()
        .filter_map(|l| l.strip_prefix("arg=").map(String::from))
        .collect();

    // Find the value that follows --tools and the value that follows --allowedTools.
    let tools_idx = arg_lines
        .iter()
        .position(|a| a == "--tools")
        .expect("--tools flag missing");
    let allowed_idx = arg_lines
        .iter()
        .position(|a| a == "--allowedTools")
        .expect("--allowedTools flag missing");
    let tools_val = arg_lines.get(tools_idx + 1).expect("--tools value missing");
    let allowed_val = arg_lines
        .get(allowed_idx + 1)
        .expect("--allowedTools value missing");

    // --tools must contain bare Bash and NOT contain the restriction.
    assert!(
        tools_val.contains(",Bash") || tools_val == "Bash" || tools_val.starts_with("Bash,"),
        "--tools value missing bare `Bash`: `{tools_val}`"
    );
    assert!(
        !tools_val.contains("Bash("),
        "--tools value should not contain the Bash(...) restriction: `{tools_val}`"
    );

    // --allowedTools must contain the restriction pattern.
    assert!(
        allowed_val.contains("Bash(codebus lint *)"),
        "--allowedTools missing Bash(codebus lint *) restriction: `{allowed_val}`"
    );
}

/// Spec: "Fix subcommand forwards configured model and effort" — default
/// `claude_code.system.fix` is `{ model: sonnet-4-6, effort: medium }` and
/// the SystemModel enum translates `sonnet-4-6` to `claude-sonnet-4-6`.
#[test]
fn fix_spawn_includes_default_model_and_effort_flags() {
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Plant lint issue → triggers fix spawn (clean vault would short-circuit).
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
    let _ = run_fix(tmp.path(), &[], "success-noop");
    let log = tmp.path().join("mock-claude.log");
    let body = fs::read_to_string(&log).expect("mock-claude log");
    assert!(
        body.contains("arg=--model") && body.contains("arg=claude-sonnet-4-6"),
        "expected --model claude-sonnet-4-6 in fix spawn argv:\n{body}"
    );
    assert!(
        body.contains("arg=--effort") && body.contains("arg=medium"),
        "expected --effort medium in fix spawn argv:\n{body}"
    );
}

// === v3-run-log: stream rendering + RunLog persistence ===

/// Spec: "Fix verb that actually spawns the agent SHALL persist a RunLog
/// entry; the InitialClean short-circuit path SHALL NOT."
#[test]
fn fix_writes_run_log_when_agent_spawns_and_skips_when_clean() {
    // Case A — agent spawns: dirty vault, agent runs, RunLog gets written.
    let tmp = TempDir::new().unwrap();
    assert!(run_init(tmp.path()).status.success());
    write_page(
        &tmp.path().join(".codebus"),
        "concepts/foo.md",
        fm_clean(),
        "see [[ghost]]",
    );
    let _ = run_fix(tmp.path(), &[], "success-stream-json");

    let log_dir = tmp.path().join(".codebus/log");
    let entries: Vec<_> = fs::read_dir(&log_dir)
        .expect("log dir exists")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("runs-"))
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "fix with agent spawn should write 1 runs-*.jsonl, got {entries:?}"
    );
    let body = fs::read_to_string(entries[0].path()).unwrap();
    let line = body.lines().last().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
    assert_eq!(parsed["mode"], "fix");
    assert_eq!(parsed["tokens"]["input_tokens"], 100);

    // Case B — InitialClean short-circuit: fresh init + planted nav files
    // → lint clean → fix exits 0 without spawning agent → no RunLog file.
    let tmp2 = TempDir::new().unwrap();
    assert!(run_init(tmp2.path()).status.success());
    let vault2 = tmp2.path().join(".codebus");
    fs::write(vault2.join("wiki/index.md"), "# index\n").unwrap();
    fs::write(vault2.join("wiki/log.md"), "# log\n").unwrap();
    let out = run_fix(tmp2.path(), &[], "success-noop");
    assert_eq!(out.status.code(), Some(0));

    let log_dir2 = tmp2.path().join(".codebus/log");
    let entries2: Vec<_> = fs::read_dir(&log_dir2)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("runs-"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        entries2.is_empty(),
        "InitialClean fix MUST NOT write a RunLog entry; found {entries2:?}"
    );
}
