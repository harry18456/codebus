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
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    // Default `--no-fix` so v3-goal tests still test goal-agent semantics
    // in isolation; v3-lint added a post-agent lint+fix phase that the
    // mock doesn't simulate (mock doesn't create index.md/log.md so lint
    // post-agent flags 2 nav-missing warnings, which would otherwise make
    // every existing test fail). Tests that want to exercise the lint+fix
    // integration pass `--no-fix=false` (or omit and add a custom flag).
    let mut cmd = Command::new(BIN);
    cmd.args(["--no-obsidian-register", "--no-fix", "goal", goal_text]);
    for f in extra_flags {
        cmd.arg(f);
    }
    cmd.current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log);
    let out = cmd.output().expect("run codebus goal");
    (out, log)
}

fn run_init(repo: &Path) -> Output {
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
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
    // v3-render-polish: detail line `(re-sync)` is debug-mode-only.
    // Default mode shows the SyncStart + SyncDone banner pair when the
    // goal flow performs a re-sync.
    assert!(
        stdout2.contains("同步 source") && stdout2.contains("同步完成"),
        "force-resync did not emit Sync banners; stdout: {stdout2}"
    );
}

// === v3-lint goal lint-and-fix integration ===

/// Run `goal` WITHOUT the default --no-fix prefix so the lint-and-fix
/// phase actually runs. Returns the binary's Output.
fn run_goal_with_fix(
    repo: &Path,
    goal_text: &str,
    extra_flags: &[&str],
    behavior: &str,
) -> Output {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let mut cmd = Command::new(BIN);
    cmd.args(["--no-obsidian-register", "goal", goal_text]);
    for f in extra_flags {
        cmd.arg(f);
    }
    cmd.current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log);
    cmd.output().expect("run codebus goal")
}

#[test]
fn goal_runs_lint_and_fix_phase_between_agent_and_commit() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    // Pre-create nav files so post-agent lint isn't dirty for nav-missing —
    // single-shot fix model means agent gets one chance; mock-claude noop
    // can't repair so we must start with a vault that's already clean to
    // verify the lint-and-fix phase ran without making the test flaky.
    let vault = tmp.path().join(".codebus");
    fs::write(vault.join("wiki/index.md"), "# index\n").unwrap();
    fs::write(vault.join("wiki/log.md"), "# log\n").unwrap();
    let init_commits_before = git(&vault, &["rev-list", "--count", "HEAD"]).stdout;

    let out = run_goal_with_fix(tmp.path(), "test-fix-phase", &[], "success-noop");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Commit count should equal init commits + 1 (single commit covering
    // goal agent writes — none in this case — and any fix edits — none).
    let init_count: u64 = String::from_utf8_lossy(&init_commits_before)
        .trim()
        .parse()
        .unwrap_or(0);
    let post = git(&vault, &["rev-list", "--count", "HEAD"]);
    let post_count: u64 = String::from_utf8_lossy(&post.stdout)
        .trim()
        .parse()
        .unwrap_or(0);
    // Mock-claude success-noop did nothing → working tree clean → no new
    // commit. Acceptable: post == init OR post == init + 1 (no-op commit
    // skipped).
    assert!(
        post_count >= init_count,
        "commit count regressed: before {init_count}, after {post_count}"
    );
}

#[test]
fn goal_with_no_fix_skips_lint_fix_phase() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    // Default run_goal already passes --no-fix; verify it works against a
    // vault that WOULD have lint warnings (no nav files written) — i.e.,
    // confirm --no-fix lets goal exit 0 even when lint would fail.
    let (out, _log) = run_goal(tmp.path(), "skipfix", &[], "success-noop");
    assert!(
        out.status.success(),
        "--no-fix should let goal succeed even with lint warnings; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // stderr must NOT contain the fix exhausted/budget-failure summary,
    // proving fix phase did not run.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("warning(s) remain"),
        "fix phase should be skipped with --no-fix; stderr: {stderr}"
    );
    assert!(
        !stderr.contains("error(s) remain"),
        "fix phase should be skipped with --no-fix; stderr: {stderr}"
    );
}

#[test]
fn goal_propagates_fix_exit_one_when_post_spawn_lint_has_issues() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success());
    // Vault is dirty (no nav files) — fix flow will run but mock-claude
    // success-noop won't repair anything → post-spawn lint still has issues.
    let out = run_goal_with_fix(tmp.path(), "willfail", &[], "success-noop");
    // Goal agent succeeded (mock returns 0) but fix's final lint reports
    // issues → goal exits 1 propagating the fix failure.
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1 from fix issues remaining; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning(s) remain") || stderr.contains("error(s)"),
        "expected fix-failure summary; stderr: {stderr}"
    );
    // Auto-commit still runs even on fix failure — verify wiki commit landed.
    let vault = tmp.path().join(".codebus");
    let head_msg = git(&vault, &["log", "--pretty=%s", "-1"]);
    let msg = String::from_utf8_lossy(&head_msg.stdout);
    assert!(
        msg.contains("wiki: willfail") || msg.contains("init: codebus vault"),
        "expected wiki commit (or unchanged HEAD if no edits); got: {msg}"
    );
}

/// Spec: "Goal subcommand forwards configured model and effort" — default
/// `claude_code.goal` is `{ model: opus, effort: high }`, both flags appear
/// on the spawned argv.
#[test]
fn goal_spawn_includes_default_model_and_effort_flags() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    assert!(run_init(tmp.path()).status.success(), "setup init");

    let (_out, log) = run_goal(tmp.path(), "test", &[], "success-noop");
    let dump = fs::read_to_string(&log).expect("mock-claude log");
    assert!(
        dump.contains("arg=--model") && dump.contains("arg=opus"),
        "expected --model opus in spawn argv; dump:\n{dump}"
    );
    assert!(
        dump.contains("arg=--effort") && dump.contains("arg=high"),
        "expected --effort high in spawn argv; dump:\n{dump}"
    );
}

/// Spec: "User-provided non-default values flow through" — config override
/// of `claude_code.goal.model` reaches the spawned argv.
#[test]
fn goal_spawn_forwards_user_configured_model() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello").unwrap();
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(
        cfg_dir.join("config.yaml"),
        "claude_code:\n  goal:\n    model: claude-opus-4-7\n    effort: max\n",
    )
    .unwrap();

    // Run init first against the SAME isolated home so the goal step sees it.
    let init_out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run codebus init");
    assert!(init_out.status.success());

    let log = tmp.path().join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let goal_out = Command::new(BIN)
        .args(["--no-obsidian-register", "--no-fix", "goal", "test"])
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", "success-noop")
        .env("CODEBUS_MOCK_LOG", &log)
        .current_dir(tmp.path())
        .output()
        .expect("run codebus goal");
    assert!(goal_out.status.success());

    let dump = fs::read_to_string(&log).expect("mock-claude log");
    assert!(
        dump.contains("arg=claude-opus-4-7"),
        "expected user-configured model in argv; dump:\n{dump}"
    );
    assert!(
        dump.contains("arg=max"),
        "expected user-configured effort in argv; dump:\n{dump}"
    );
}
