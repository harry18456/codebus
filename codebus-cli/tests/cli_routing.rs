use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Run `codebus init --no-obsidian-register` against `repo` and return the
/// captured Output plus the resolved vault root path. Used by v3-vault-history
/// integration tests to drive end-to-end init and inspect resulting vault.
fn run_init(repo: &Path) -> Output {
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(repo)
        .output()
        .expect("run codebus init")
}

/// Run `git -C <vault> <args>` and return its Output. Caller asserts.
fn git(vault: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(vault)
        .output()
        .expect("run git")
}

// === Subcommand Registration ===

#[test]
fn help_lists_exactly_the_five_subcommands() {
    let out = Command::new(BIN).arg("--help").output().expect("run binary");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for verb in ["init", "goal", "query", "lint", "fix"] {
        assert!(combined.contains(verb), "help missing `{verb}`:\n{combined}");
    }
    for forbidden in ["mcp", "ingest"] {
        assert!(
            !combined.contains(&format!(" {forbidden} ")),
            "help unexpectedly contains `{forbidden}`:\n{combined}"
        );
    }
}

#[test]
fn version_flag_prints_cargo_pkg_version() {
    let out = Command::new(BIN).arg("--version").output().expect("run binary");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_subcommand_is_rejected_by_clap() {
    let out = Command::new(BIN).arg("randomverb").output().expect("run binary");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized") || stderr.contains("invalid") || stderr.contains("subcommand"),
        "stderr should mention rejection: {stderr}"
    );
}

#[test]
fn mcp_subcommand_is_rejected_specifically() {
    let out = Command::new(BIN).arg("mcp").output().expect("run binary");
    assert!(!out.status.success());
}

// === No-Arg Defaults to Init Dispatch ===

#[test]
fn bare_invocation_routes_to_init_handler_and_creates_per_project_bundles() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run binary bare");
    assert!(
        out.status.success(),
        "bare invocation should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("vault layout"));
    assert!(stdout.contains("codebus init complete"));
    assert!(tmp.path().join(".codebus").is_dir());
    // Vault-internal skill bundle locations: <repo>/.codebus/.claude/skills/codebus-{verb}/
    for verb in ["goal", "query", "fix"] {
        let p = tmp
            .path()
            .join(".codebus/.claude/skills")
            .join(format!("codebus-{verb}"))
            .join("SKILL.md");
        assert!(p.exists(), "missing vault-internal bundle for {verb}: {p:?}");
    }
    // And NOT at repo root
    for verb in ["goal", "query", "fix"] {
        let wrong = tmp
            .path()
            .join(".claude/skills")
            .join(format!("codebus-{verb}"))
            .join("SKILL.md");
        assert!(!wrong.exists(), "skill should not be at repo-root .claude: {wrong:?}");
    }
}

#[test]
fn explicit_init_and_bare_invocation_have_structurally_identical_output() {
    let tmp_bare = TempDir::new().unwrap();
    let tmp_explicit = TempDir::new().unwrap();
    let bare = Command::new(BIN)
        .arg("--no-obsidian-register")
        .current_dir(tmp_bare.path())
        .output()
        .expect("run bare");
    let explicit = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(tmp_explicit.path())
        .output()
        .expect("run init");

    assert_eq!(bare.status.code(), explicit.status.code());

    let bare_stdout = String::from_utf8_lossy(&bare.stdout);
    let explicit_stdout = String::from_utf8_lossy(&explicit.stdout);
    let bare_lines: Vec<&str> = bare_stdout.lines().collect();
    let explicit_lines: Vec<&str> = explicit_stdout.lines().collect();
    assert_eq!(
        bare_lines.len(),
        explicit_lines.len(),
        "stdout line count differs"
    );
    for (b, e) in bare_lines.iter().zip(explicit_lines.iter()) {
        let b_prefix: String = b.chars().take(12).collect();
        let e_prefix: String = e.chars().take(12).collect();
        assert_eq!(b_prefix, e_prefix);
    }
}

// === Init Progress Line — PII Match Count ===

#[test]
fn init_progress_line_includes_zero_pii_count_for_clean_repo() {
    let tmp = TempDir::new().unwrap();
    // Repo with one harmless file → no PII matches expected.
    std::fs::write(tmp.path().join("README.md"), b"# hello\nplain text").unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(
        out.status.success(),
        "init should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let raw_line = stdout
        .lines()
        .find(|l| l.starts_with("✓ raw mirror:"))
        .expect("missing raw mirror progress line");
    assert!(
        raw_line.contains("0 PII matches"),
        "raw mirror line should contain '0 PII matches', got: {raw_line}"
    );
}

#[test]
fn init_progress_line_reports_nonzero_pii_count_for_repo_with_secrets() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("aws.py"),
        b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n",
    )
    .unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let raw_line = stdout
        .lines()
        .find(|l| l.starts_with("✓ raw mirror:"))
        .expect("missing raw mirror progress line");
    // Expect the count column to be present and non-zero.
    assert!(
        raw_line.contains("PII matches"),
        "raw mirror line missing 'PII matches' label: {raw_line}"
    );
    assert!(
        !raw_line.contains("0 PII matches"),
        "expected non-zero PII match count, got: {raw_line}"
    );
    // stderr should carry at least one warning for the AWS key (does not
    // include the literal key text — verified separately at the lib level).
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("pii warn:") && stderr.contains("aws-access-key"),
        "stderr should contain pii warn for aws-access-key, got: {stderr}"
    );
}

// === Vault Nested Git + First Commit (v3-vault-history) ===

#[test]
fn nested_git_repo_present_with_codebus_identity_after_init() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), b"hello").unwrap();
    let out = run_init(tmp.path());
    assert!(
        out.status.success(),
        "init should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let vault = tmp.path().join(".codebus");
    assert!(vault.join(".git").is_dir(), "missing .codebus/.git/");

    let email = git(&vault, &["config", "--get", "user.email"]);
    assert!(email.status.success());
    assert_eq!(
        String::from_utf8_lossy(&email.stdout).trim(),
        "codebus@local"
    );

    let name = git(&vault, &["config", "--get", "user.name"]);
    assert!(name.status.success());
    assert_eq!(String::from_utf8_lossy(&name.stdout).trim(), "codebus");
}

#[test]
fn init_produces_canonical_init_commit() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), b"hello").unwrap();
    let out = run_init(tmp.path());
    assert!(out.status.success());

    let vault = tmp.path().join(".codebus");

    // Latest commit message exactly matches the canonical init message.
    let log = git(&vault, &["log", "--pretty=%s", "-1"]);
    assert!(log.status.success());
    assert_eq!(
        String::from_utf8_lossy(&log.stdout).trim(),
        "init: codebus vault"
    );

    // Working tree clean after init's auto_commit.
    let porcelain = git(&vault, &["status", "--porcelain"]);
    assert!(porcelain.status.success());
    assert_eq!(
        String::from_utf8_lossy(&porcelain.stdout).trim(),
        "",
        "expected clean working tree after init auto_commit"
    );

    // Tree contains CLAUDE.md and manifest.yaml.
    let ls_tree = git(&vault, &["ls-tree", "-r", "HEAD", "--name-only"]);
    assert!(ls_tree.status.success());
    let tree = String::from_utf8_lossy(&ls_tree.stdout);
    let entries: Vec<&str> = tree.lines().collect();
    assert!(
        entries.iter().any(|l| *l == "CLAUDE.md"),
        "tree missing CLAUDE.md: {entries:?}"
    );
    assert!(
        entries.iter().any(|l| *l == "manifest.yaml"),
        "tree missing manifest.yaml: {entries:?}"
    );

    // raw/code/ paths excluded by internal .gitignore.
    assert!(
        !entries.iter().any(|l| l.starts_with("raw/code/")),
        "raw/code/ leaked into nested commit: {entries:?}"
    );
}

#[test]
fn re_init_preserves_user_modified_git_config() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), b"hello").unwrap();
    assert!(run_init(tmp.path()).status.success());

    let vault = tmp.path().join(".codebus");

    // Simulate user override of nested repo identity.
    let set = git(
        &vault,
        &["config", "user.email", "alice@example.com"],
    );
    assert!(set.status.success());

    // Re-init: nested repo already exists, init_nested_repo SHALL be a no-op.
    let out2 = run_init(tmp.path());
    assert!(out2.status.success(), "re-init should succeed");

    let email = git(&vault, &["config", "--get", "user.email"]);
    assert_eq!(
        String::from_utf8_lossy(&email.stdout).trim(),
        "alice@example.com",
        "re-init must not overwrite user-modified user.email"
    );
}

#[test]
fn internal_gitignore_appends_missing_required_lines() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("README.md"), b"hello").unwrap();
    // Pre-seed .codebus/.gitignore with two lines (one required, one user-added).
    let vault = tmp.path().join(".codebus");
    std::fs::create_dir_all(&vault).unwrap();
    std::fs::write(vault.join(".gitignore"), b".lock\nnotes/\n").unwrap();

    let out = run_init(tmp.path());
    assert!(
        out.status.success(),
        "init should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let body = std::fs::read_to_string(vault.join(".gitignore")).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    // Original two lines preserved at front in original order.
    assert_eq!(lines[0], ".lock", "first line preserved: {body:?}");
    assert_eq!(lines[1], "notes/", "second line preserved: {body:?}");
    // Three missing required lines appended in declared order
    // (raw/code/, **/.obsidian/, logs/).
    assert!(
        lines.contains(&"raw/code/"),
        "missing raw/code/ line: {body:?}"
    );
    assert!(
        lines.contains(&"**/.obsidian/"),
        "missing **/.obsidian/ line: {body:?}"
    );
    assert!(
        lines.contains(&"logs/"),
        "missing logs/ line: {body:?}"
    );
    // User-added `notes/` not duplicated.
    assert_eq!(
        lines.iter().filter(|l| **l == "notes/").count(),
        1,
        "notes/ duplicated: {body:?}"
    );
}

// === Stub Verb Exit Behavior (4 verbs) ===

#[test]
fn remaining_stub_verbs_exit_non_zero_with_not_yet_implemented_message() {
    for verb in ["query", "lint", "fix"] {
        let out = Command::new(BIN).arg(verb).output().expect("run binary");
        assert!(!out.status.success(), "verb `{verb}` should fail");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("not yet implemented"));
        assert!(stderr.contains(verb));
    }
}

#[test]
fn init_no_longer_matches_stub_behavior() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("not yet implemented"));
}

#[test]
fn stub_verbs_do_not_panic_or_block() {
    for verb in ["query", "lint", "fix"] {
        let out = Command::new(BIN).arg(verb).output().expect("run binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("panicked at"));
        assert!(!stderr.contains("RUST_BACKTRACE"));
    }
}

#[test]
fn stub_verbs_accept_debug_flag_silently() {
    for verb in ["query", "lint", "fix"] {
        let out = Command::new(BIN).args([verb, "--debug"]).output().expect("run");
        assert!(!out.status.success(), "stub verb `{verb}` --debug should still exit non-zero");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("not yet implemented"));
        // Stub verbs do not emit [debug] content
        assert!(
            !stderr.contains("[debug]") && !String::from_utf8_lossy(&out.stdout).contains("[debug]"),
            "stub verb `{verb}` should not emit [debug] lines: stdout={} stderr={stderr}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
}

// === Debug Flag Output ===

#[test]
fn debug_flag_at_top_level_emits_debug_lines() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["--debug", "init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("[debug]"),
        "expected [debug] line in output:\n{combined}"
    );
}

#[test]
fn debug_flag_at_subcommand_position_behaves_identically() {
    let tmp_a = TempDir::new().unwrap();
    let tmp_b = TempDir::new().unwrap();

    let top_level = Command::new(BIN)
        .args(["--debug", "init", "--no-obsidian-register"])
        .current_dir(tmp_a.path())
        .output()
        .expect("top-level");
    let subcommand = Command::new(BIN)
        .args(["init", "--debug", "--no-obsidian-register"])
        .current_dir(tmp_b.path())
        .output()
        .expect("subcommand");

    assert_eq!(top_level.status.code(), subcommand.status.code());

    let has_debug = |o: &std::process::Output| {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        combined.contains("[debug]")
    };
    assert!(has_debug(&top_level), "top-level --debug missing [debug] lines");
    assert!(has_debug(&subcommand), "subcommand --debug missing [debug] lines");
}

#[test]
fn without_debug_no_debug_lines() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains("[debug]"),
        "stdout unexpectedly contains [debug]: {stdout}"
    );
    assert!(
        !stderr.contains("[debug]"),
        "stderr unexpectedly contains [debug]: {stderr}"
    );
}
