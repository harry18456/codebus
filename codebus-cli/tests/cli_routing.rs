use std::process::Command;

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
    // Negative: ensure no stray subcommand leaked in
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
    let expected = env!("CARGO_PKG_VERSION");
    assert!(
        stdout.contains(expected),
        "version output `{stdout}` missing pkg version `{expected}`"
    );
}

#[test]
fn unknown_subcommand_is_rejected_by_clap() {
    let out = Command::new(BIN).arg("randomverb").output().expect("run binary");
    assert!(!out.status.success(), "unknown subcommand should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized") || stderr.contains("invalid") || stderr.contains("subcommand"),
        "stderr should mention rejection: {stderr}"
    );
}

#[test]
fn mcp_subcommand_is_rejected_specifically() {
    // Strategy memo defers MCP. Path D doesn't reintroduce the subcommand.
    let out = Command::new(BIN).arg("mcp").output().expect("run binary");
    assert!(!out.status.success(), "`mcp` should not be a registered subcommand");
}

// === No-Arg Defaults to Init Dispatch ===

#[test]
fn bare_invocation_routes_to_init_handler() {
    let out = Command::new(BIN).output().expect("run binary");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("init: not yet implemented"),
        "bare invocation did not dispatch to init stub. stderr: {stderr}"
    );
}

#[test]
fn explicit_init_invocation_produces_identical_behavior_to_bare() {
    let bare = Command::new(BIN).output().expect("run bare");
    let explicit = Command::new(BIN).arg("init").output().expect("run init");
    assert_eq!(
        bare.status.code(),
        explicit.status.code(),
        "exit code differs: bare={:?} explicit={:?}",
        bare.status.code(),
        explicit.status.code()
    );
    assert_eq!(
        String::from_utf8_lossy(&bare.stderr),
        String::from_utf8_lossy(&explicit.stderr),
        "stderr differs"
    );
    assert_eq!(
        String::from_utf8_lossy(&bare.stdout),
        String::from_utf8_lossy(&explicit.stdout),
        "stdout differs"
    );
}

// === Stub Verb Exit Behavior ===

#[test]
fn each_verb_stub_exits_non_zero_with_not_yet_implemented_message() {
    for verb in ["init", "goal", "query", "lint", "fix"] {
        let out = Command::new(BIN).arg(verb).output().expect("run binary");
        assert!(
            !out.status.success(),
            "verb `{verb}` should exit non-zero, got status {:?}",
            out.status.code()
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("not yet implemented"),
            "verb `{verb}` stderr missing `not yet implemented`: {stderr}"
        );
        assert!(
            stderr.contains(verb),
            "verb `{verb}` stderr missing verb name in message: {stderr}"
        );
    }
}

#[test]
fn stub_verbs_do_not_panic_or_block() {
    // No timeout primitive in std::process; instead rely on the command
    // returning quickly. If a stub blocks (e.g., reads stdin), this test
    // would hang the test runner — failing CI rather than producing a
    // misleading green. The non-zero-exit assertion implicitly proves the
    // process terminated under normal control flow rather than panicking
    // (which would still terminate but with a panic-shaped stderr we can
    // detect).
    for verb in ["init", "goal", "query", "lint", "fix"] {
        let out = Command::new(BIN).arg(verb).output().expect("run binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("panicked at"),
            "verb `{verb}` panicked: {stderr}"
        );
        assert!(
            !stderr.contains("RUST_BACKTRACE"),
            "verb `{verb}` produced panic backtrace hint: {stderr}"
        );
    }
}
