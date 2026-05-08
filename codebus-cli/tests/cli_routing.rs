use std::process::Command;

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

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

// === Stub Verb Exit Behavior (4 verbs) ===

#[test]
fn remaining_stub_verbs_exit_non_zero_with_not_yet_implemented_message() {
    for verb in ["goal", "query", "lint", "fix"] {
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
    for verb in ["goal", "query", "lint", "fix"] {
        let out = Command::new(BIN).arg(verb).output().expect("run binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!stderr.contains("panicked at"));
        assert!(!stderr.contains("RUST_BACKTRACE"));
    }
}

#[test]
fn stub_verbs_accept_debug_flag_silently() {
    for verb in ["goal", "query", "lint", "fix"] {
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
