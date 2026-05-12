use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// Run `codebus init --no-obsidian-register` against `repo` with
/// `CODEBUS_HOME` set to `home` so the global-config write step targets an
/// isolated path instead of the real `~/.codebus/`.
fn run_init_with_home(repo: &Path, home: &Path) -> Output {
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home)
        .current_dir(repo)
        .output()
        .expect("run codebus init")
}

/// Convenience wrapper: run init with a fresh per-call `CODEBUS_HOME` so the
/// test never touches the real user's `~/.codebus/`. Returns just the Output —
/// callers that need to inspect the home dir should use `run_init_with_home`
/// directly.
fn run_init(repo: &Path) -> Output {
    let home = TempDir::new().expect("create isolated CODEBUS_HOME tempdir");
    run_init_with_home(repo, home.path())
}

/// Run `git -C <vault> <args>` and return its Output. Caller asserts.
fn git(vault: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(vault)
        .output()
        .expect("run git")
}

// === Subcommand Registration (six subcommands; `claude-code-endpoint-profiles`) ===

#[test]
fn help_lists_exactly_the_six_subcommands() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("run binary");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for verb in ["init", "goal", "query", "lint", "fix", "config"] {
        assert!(
            combined.contains(verb),
            "help missing `{verb}`:\n{combined}"
        );
    }
    for forbidden in ["mcp", "ingest"] {
        assert!(
            !combined.contains(&format!(" {forbidden} ")),
            "help unexpectedly contains `{forbidden}`:\n{combined}"
        );
    }
}

#[test]
fn config_help_lists_three_sub_actions() {
    let out = Command::new(BIN)
        .args(["config", "--help"])
        .output()
        .expect("run binary");
    assert!(out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for action in ["set-key", "get-key", "delete-key"] {
        assert!(
            combined.contains(action),
            "config --help missing `{action}`:\n{combined}"
        );
    }
}

#[test]
fn version_flag_prints_cargo_pkg_version() {
    let out = Command::new(BIN)
        .arg("--version")
        .output()
        .expect("run binary");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn unknown_subcommand_is_rejected_by_clap() {
    let out = Command::new(BIN)
        .arg("randomverb")
        .output()
        .expect("run binary");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized")
            || stderr.contains("invalid")
            || stderr.contains("subcommand"),
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
    // Default mode emits banner sequence (Start / SyncDone / PiiSummary /
    // CommitDone / Done). In the subprocess test runner stdout is piped so
    // emoji are off — the symbol fallback ▶ leads the Start banner. We
    // verify the codebus brand identity via the literal `駛入` token from
    // the Start banner body; `下車` from the Done banner; per-step progress
    // lines like `vault layout:` are debug-mode-only now.
    assert!(stdout.contains("駛入"), "missing Start banner: {stdout}");
    assert!(stdout.contains("下車"), "missing Done banner: {stdout}");
    assert!(tmp.path().join(".codebus").is_dir());
    // v3-lint: skill bundles are written at BOTH locations (vault-internal
    // + repo-root) so both CLI-spawn-with-cwd-vault AND user-direct-from-
    // repo-root paths discover the skill.
    for verb in ["goal", "query", "fix"] {
        let vault_path = tmp
            .path()
            .join(".codebus/.claude/skills")
            .join(format!("codebus-{verb}"))
            .join("SKILL.md");
        assert!(
            vault_path.exists(),
            "missing vault-internal bundle for {verb}: {vault_path:?}"
        );
        let repo_path = tmp
            .path()
            .join(".claude/skills")
            .join(format!("codebus-{verb}"))
            .join("SKILL.md");
        assert!(
            repo_path.exists(),
            "missing repo-root bundle for {verb}: {repo_path:?}"
        );
    }
    // v3-fix-trust-agent: vault-internal settings.json with PreToolUse Bash hook.
    let settings_path = tmp.path().join(".codebus/.claude/settings.json");
    assert!(
        settings_path.exists(),
        "missing vault-internal settings.json"
    );
    let body = std::fs::read_to_string(&settings_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&body).expect("settings.json must parse as JSON");
    let entries = parsed["hooks"]["PreToolUse"].as_array().unwrap();
    assert!(!entries.is_empty(), "PreToolUse hook must be configured");
    assert_eq!(entries[0]["matcher"], "Bash");
    // v3-fix-trust-agent: settings.json NOT written to source repo root.
    let bad = tmp.path().join(".claude/settings.json");
    assert!(
        !bad.exists(),
        "settings.json must not be written to repo root: {bad:?}"
    );
    // v3-fix-trust-agent: vault internal .gitignore includes settings.local.json.
    let internal_gi = std::fs::read_to_string(tmp.path().join(".codebus/.gitignore")).unwrap();
    assert!(
        internal_gi
            .lines()
            .any(|l| l == ".claude/settings.local.json"),
        "vault internal .gitignore missing `.claude/settings.local.json` line:\n{internal_gi}"
    );
}

#[test]
fn explicit_init_and_bare_invocation_have_structurally_identical_output() {
    let tmp_bare = TempDir::new().unwrap();
    let tmp_explicit = TempDir::new().unwrap();
    let home_bare = TempDir::new().unwrap();
    let home_explicit = TempDir::new().unwrap();
    let bare = Command::new(BIN)
        .arg("--no-obsidian-register")
        .env("CODEBUS_HOME", home_bare.path())
        .current_dir(tmp_bare.path())
        .output()
        .expect("run bare");
    let explicit = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home_explicit.path())
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
        // Compare leading glyph only — content past the lead carries
        // run-specific data (commit SHA, elapsed_ms, file counts) that
        // legitimately differs across runs even when the structural shape
        // is identical.
        let b_prefix: String = b.chars().take(2).collect();
        let e_prefix: String = e.chars().take(2).collect();
        assert_eq!(b_prefix, e_prefix);
    }
}

// === Init Progress Line — PII Match Count ===

#[test]
fn init_progress_line_includes_zero_pii_count_for_clean_repo() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    // Repo with one harmless file → no PII matches expected.
    std::fs::write(tmp.path().join("README.md"), b"# hello\nplain text").unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(
        out.status.success(),
        "init should succeed; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // v3-render-polish: per-step `✓ raw mirror:` line moved to debug mode;
    // PII match count surfaces via the PiiSummary banner instead.
    let pii_line = stdout
        .lines()
        .find(|l| l.contains("PII：") || l.contains("PII:"))
        .expect("missing PiiSummary banner line");
    assert!(
        pii_line.contains("hits 0"),
        "PiiSummary banner should report hits 0; got: {pii_line}"
    );
}

#[test]
fn init_progress_line_reports_nonzero_pii_count_for_repo_with_secrets() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("aws.py"), b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n").unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pii_line = stdout
        .lines()
        .find(|l| l.contains("PII：") || l.contains("PII:"))
        .expect("missing PiiSummary banner line");
    assert!(
        !pii_line.contains("hits 0"),
        "expected non-zero hits in PiiSummary banner; got: {pii_line}"
    );
    assert!(
        pii_line.contains("hits "),
        "PiiSummary banner missing 'hits' field: {pii_line}"
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
    let set = git(&vault, &["config", "user.email", "alice@example.com"]);
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
    // v3-run-log: line is `log/` (singular, aligns with vault::layout's
    // `log/` directory) — was `logs/` plural until v3-run-log corrected
    // the typo.
    assert!(lines.contains(&"log/"), "missing log/ line: {body:?}");
    // User-added `notes/` not duplicated.
    assert_eq!(
        lines.iter().filter(|l| **l == "notes/").count(),
        1,
        "notes/ duplicated: {body:?}"
    );
}

// === Stub Verb Exit Behavior (REMOVED: all verbs implemented as of v3-lint) ===
// v3-lint REMOVES the catch-all "Stub Verb Exit Behavior" spec. All four
// subcommands (goal, query, lint, fix) now have explicit Subcommand
// Behavior requirements. This block previously asserted lint/fix were
// stubs; that assertion is now obsolete.

/// v3-run-log: explicit assertion that the gitignore line is `log/`
/// (singular, aligned with `vault::layout` which creates `log/`), NOT
/// `logs/` (plural, the typo carried by earlier changes). Sister test
/// to `internal_gitignore_appends_missing_required_lines` — this one
/// fails loud if anyone reverts the typo fix.
#[test]
fn init_internal_gitignore_lists_log_singular_not_logs_plural() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success(), "init failed: {:?}", out);

    let body = std::fs::read_to_string(tmp.path().join(".codebus/.gitignore"))
        .expect("internal .gitignore exists after init");
    let lines: Vec<&str> = body.lines().collect();
    assert!(
        lines.contains(&"log/"),
        "expected `log/` (singular) in {body:?}"
    );
    assert!(
        !lines.contains(&"logs/"),
        "must NOT contain `logs/` (plural) — that's the typo v3-run-log fixed: {body:?}"
    );
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

// `stub_verbs_do_not_panic_or_block` and `stub_verbs_accept_debug_flag_silently`
// removed — all four verbs now have explicit non-stub behavior with their own
// dedicated tests under tests/{init,goal,query,lint,fix}_flow.rs.

// === Debug Flag Output ===

#[test]
fn debug_flag_at_top_level_emits_debug_lines() {
    let tmp = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["--debug", "init", "--no-obsidian-register"])
        .current_dir(tmp.path())
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
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
    assert!(
        has_debug(&top_level),
        "top-level --debug missing [debug] lines"
    );
    assert!(
        has_debug(&subcommand),
        "subcommand --debug missing [debug] lines"
    );
}

#[test]
fn without_debug_no_debug_lines() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
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

// === v3-config: Global Config Starter Writer (init step) ===

#[test]
fn init_writes_global_config_starter_when_missing() {
    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(
        out.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cfg_path = home.path().join(".codebus").join("config.yaml");
    assert!(
        cfg_path.exists(),
        "starter config not written at {}",
        cfg_path.display()
    );
    // v3-render-polish: per-step `✓ global config: wrote` line moved to
    // debug mode. Default mode no longer surfaces it; the side-effect
    // (file existence above) is what we assert in default mode.
}

#[test]
fn init_does_not_overwrite_existing_global_config() {
    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let cfg_path = cfg_dir.join("config.yaml");
    let custom = "pii:\n  on_hit: warn\n";
    std::fs::write(&cfg_path, custom).unwrap();

    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(out.status.success());

    let preserved = std::fs::read_to_string(&cfg_path).unwrap();
    assert_eq!(preserved, custom, "init clobbered user config");

    // v3-render-polish: per-step `already present` line moved to debug
    // mode. Default mode validates the contract via the file's preserved
    // content (asserted above) — no progress-line assertion needed.
}

#[test]
fn init_writes_default_parseable_global_config() {
    use codebus_core::config::{
        ClaudeCodeConfig, PiiConfig, load_claude_code_config, load_lint_fix_config, load_pii_config,
    };

    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(out.status.success());

    let cfg_path = home.path().join(".codebus").join("config.yaml");

    // Each section parses to its respective `Default::default()`.
    let pii = load_pii_config(&cfg_path).expect("parse pii section");
    assert_eq!(pii, PiiConfig::default());

    let cc = load_claude_code_config(&cfg_path).expect("parse claude_code section");
    assert_eq!(cc, ClaudeCodeConfig::default());

    let lf = load_lint_fix_config(&cfg_path).expect("parse lint.fix section");
    assert!(lf.enabled);
}

#[test]
fn init_with_null_scanner_config_skips_pii_warnings() {
    // Pre-populate ~/.codebus/config.yaml with `pii.scanner: none` so the
    // raw mirror uses NullScanner. Source contains a fake credential that
    // RegexBasic would catch — under `none` scanner, zero warnings emitted.
    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(cfg_dir.join("config.yaml"), "pii:\n  scanner: none\n").unwrap();
    std::fs::write(
        repo.path().join("src.rs"),
        b"const KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("pii warn:"),
        "expected 0 pii warn lines under scanner: none, got: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pii_line = stdout
        .lines()
        .find(|l| l.contains("PII：") || l.contains("PII:"))
        .unwrap_or("");
    assert!(
        pii_line.contains("hits 0") && pii_line.contains("none"),
        "expected PiiSummary banner with scanner=none and hits=0: {pii_line}"
    );
}

#[test]
fn init_with_bad_patterns_extra_falls_back_to_builtin() {
    // Malformed regex in patterns_extra → init emits stderr warning and uses
    // built-in pattern set only (does NOT degrade to NullScanner).
    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(
        cfg_dir.join("config.yaml"),
        "pii:\n  patterns_extra:\n    - '[unclosed bracket'\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("src.rs"),
        b"const KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(out.status.success());

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning: pii config patterns_extra"),
        "expected stderr fallback warning: {stderr}"
    );
    // Built-in scanner still runs — at least one PII hit reported via the
    // PiiSummary banner. Under the default `on_hit=mask`, the matched
    // substring is replaced inside the mirrored file.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let pii_line = stdout
        .lines()
        .find(|l| l.contains("PII：") || l.contains("PII:"))
        .expect("missing PiiSummary banner line");
    assert!(
        !pii_line.contains("hits 0"),
        "built-in scanner should still detect at least one match: {pii_line}"
    );
}

// === v3-render-polish: Environment-Aware Output Styling ===

/// Spec scenario: "Non-TTY pipe disables emoji and color and hyperlinks".
/// Subprocess invocation pipes stdout, so emoji glyphs and ANSI escapes
/// SHALL NOT appear. Banner uses ASCII fallback symbols (`▶`, `ok`, `~`,
/// `!`, `.`, `✓`, `i`).
#[test]
fn init_pipe_disables_emoji_and_color() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for emoji in ["🚌", "🎉", "🛡", "🔍", "🎯", "🔄", "📌"] {
        assert!(
            !stdout.contains(emoji),
            "emoji {emoji} unexpectedly present under non-TTY: {stdout}"
        );
    }
    assert!(
        !stdout.contains("\x1b["),
        "ANSI escape unexpectedly present under non-TTY: {stdout}"
    );
    // ASCII fallback for Start banner is `▶`.
    assert!(
        stdout.contains("▶"),
        "missing ASCII Start lead `▶`: {stdout}"
    );
}

/// Spec scenario: "--emoji flag is rejected by clap" — the flag was
/// deliberately NOT registered in v3-render-polish (no 5-level emoji
/// priority chain). clap MUST reject it.
#[test]
fn emoji_flag_rejected_by_clap() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register", "--emoji", "on"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(
        !out.status.success(),
        "init should reject --emoji flag; got success"
    );
}

/// Spec scenario: "NO_EMOJI env is silently ignored" — since v3-render-
/// polish drops the 5-level priority chain, NO_EMOJI has no observable
/// effect. In subprocess context emoji are off anyway (non-TTY), so we
/// assert that the binary does not error out when NO_EMOJI is set.
#[test]
fn no_emoji_env_silently_ignored() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .env("NO_EMOJI", "1")
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(
        out.status.success(),
        "init should succeed with NO_EMOJI set; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Spec scenario: "NO_COLOR disables ANSI color but keeps emoji".
/// Verified at the lib-level for the lint formatter (emoji vs color flags
/// are independent there). Process-level test is constrained because in
/// subprocess context stdout is not a TTY → emoji are off regardless. We
/// assert the weaker invariant: NO_COLOR set + non-TTY produces no ANSI.
#[test]
fn init_no_color_produces_no_ansi() {
    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .env("NO_COLOR", "1")
        .current_dir(tmp.path())
        .output()
        .expect("run init");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "ANSI escape leaked under NO_COLOR: {stdout}"
    );
}

// === v3-bug-fixes: source-signal stability across init invocations ===

/// v3-bug-fixes Bug 1 — `init` SHALL write a manifest signal whose
/// `total_bytes` matches a fresh `walk_source_for_signal` immediately after
/// init terminates. Otherwise subsequent verb invocations (`goal` / `query`)
/// would falsely conclude that source has drifted and trigger a redundant
/// raw_sync re-sync.
///
/// We verify by running init twice in a row against the same repo: the
/// second run's manifest must report identical `total_bytes` to the first.
/// If the source `.gitignore` mutation sat AFTER raw_sync (the pre-fix
/// state), the first init's signal would lag the post-mutation state and
/// the second init would record a different number — exposing the drift.
#[test]
fn init_followed_by_repeat_init_does_not_drift() {
    use std::fs;

    let tmp = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# hello\n").unwrap();
    // Make the source a real git repo so the source `.gitignore` mutation
    // path is actually triggered (it short-circuits when not a git repo).
    let git_init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    assert!(git_init.status.success());

    let first = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("first init");
    assert!(
        first.status.success(),
        "first init stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let manifest_path = tmp.path().join(".codebus").join("manifest.yaml");
    let manifest_after_first = fs::read_to_string(&manifest_path).expect("manifest exists");
    let bytes_first = extract_total_bytes(&manifest_after_first);

    let second = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(tmp.path())
        .output()
        .expect("second init");
    assert!(
        second.status.success(),
        "second init stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let manifest_after_second = fs::read_to_string(&manifest_path).expect("manifest exists");
    let bytes_second = extract_total_bytes(&manifest_after_second);

    assert_eq!(
        bytes_first, bytes_second,
        "manifest total_bytes drifted between identical init runs: {bytes_first} → {bytes_second}"
    );
}

fn extract_total_bytes(manifest_yaml: &str) -> u64 {
    for line in manifest_yaml.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("total_bytes:") {
            return rest.trim().parse().expect("total_bytes parses as u64");
        }
    }
    panic!("manifest missing total_bytes field:\n{manifest_yaml}")
}

/// Verification for task 5.1 (Fix Loop Library Invocation Entry Point):
/// the CLI binary crate SHALL NOT call `wiki::fix::run_fix_loop` directly
/// — the only callers SHALL be inside `codebus_core::verb::{goal,fix}`.
/// This guard scans the CLI source tree for the forbidden symbol so a
/// future contributor cannot accidentally regress the delegation contract.
#[test]
fn cli_does_not_call_run_fix_loop_directly() {
    use std::fs;
    fn scan(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut hits = Vec::new();
        for entry in fs::read_dir(dir).expect("read cli src dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                hits.extend(scan(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                let body = fs::read_to_string(&path).expect("read .rs file");
                if body.contains("wiki::fix::run_fix_loop")
                    || body.contains("use codebus_core::wiki::fix")
                {
                    hits.push(path);
                }
            }
        }
        hits
    }
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let hits = scan(&src_dir);
    assert!(
        hits.is_empty(),
        "CLI binary must not call run_fix_loop directly; \
         delegate via codebus_core::verb::{{goal,fix}}::run_*. \
         Forbidden references found in: {hits:?}"
    );
}
