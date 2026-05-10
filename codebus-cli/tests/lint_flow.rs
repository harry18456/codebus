//! Integration tests for `codebus lint` subcommand (v3-lint Section 3).
//!
//! Covers Lint Subcommand Behavior scenarios: vault auto-detection,
//! exit codes (0 clean, 0 warn-only, 1 error, 2 no-vault), text + JSON
//! output formats, and `--repo` override.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

fn lint(cwd: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .arg("lint")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run codebus lint")
}

fn init_vault(repo: &Path) {
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    let out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo)
        .output()
        .expect("run init");
    assert!(out.status.success(), "init failed: {}", String::from_utf8_lossy(&out.stderr));
}

fn write_page(vault: &Path, rel: &str, frontmatter: &str, body: &str) {
    let full = vault.join("wiki").join(rel);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    let content = format!("---\n{frontmatter}---\n{body}");
    std::fs::write(full, content).unwrap();
}

fn fm_clean() -> &'static str {
    "title: foo\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-09'\nupdated: '2026-05-09'\nrelated: []\nstale: false\n"
}

#[test]
fn lint_exits_two_when_no_vault_locatable() {
    let tmp = TempDir::new().unwrap();
    let out = lint(tmp.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no codebus vault"));
    assert!(stderr.contains("codebus init"));
}

#[test]
fn lint_exits_zero_on_clean_vault() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "# foo");
    let out = lint(tmp.path(), &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&out.stderr));
}

#[test]
fn lint_exits_zero_with_warnings_only() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    // Page with broken body wikilink → warn (not error)
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = lint(tmp.path(), &[]);
    assert_eq!(out.status.code(), Some(0), "warning-only must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ghost"));
}

#[test]
fn lint_exits_one_on_errors() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    // Page with broken related → error
    let fm = "title: foo\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-09'\nupdated: '2026-05-09'\nrelated:\n  - '[[ghost]]'\nstale: false\n";
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm, "# foo");
    let out = lint(tmp.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn lint_text_format_emits_vault_relative_paths() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = lint(tmp.path(), &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("wiki/concepts/foo.md") || stdout.contains("wiki\\concepts\\foo.md"));
    // Text format must not contain absolute path leakage
    assert!(!stdout.contains(tmp.path().to_string_lossy().as_ref()), "text leaked abs path: {stdout}");
}

#[test]
fn lint_json_format_emits_single_valid_json() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = lint(tmp.path(), &["--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("JSON output must parse");
    assert!(parsed["vault_root"].is_string());
    assert!(parsed["issues"].is_array());
    let issues = parsed["issues"].as_array().unwrap();
    assert!(!issues.is_empty());
    let abs_path = issues[0]["path"].as_str().unwrap();
    let normalized = abs_path.replace('\\', "/");
    // Absolute path must contain vault prefix
    assert!(normalized.contains(".codebus"));
    assert!(normalized.contains("wiki"));
}

#[test]
fn lint_with_explicit_repo_flag_targets_specified_directory() {
    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();
    init_vault(tmp1.path());
    init_vault(tmp2.path());
    // Plant an issue in tmp2, run from tmp1 with --repo tmp2
    write_page(&tmp2.path().join(".codebus"), "concepts/foo.md", fm_clean(), "see [[ghost]]");
    let out = Command::new(BIN)
        .args([
            "--repo",
            tmp2.path().to_str().unwrap(),
            "lint",
        ])
        .current_dir(tmp1.path())
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ghost"));
}

#[test]
fn lint_does_not_modify_vault_files() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());
    write_page(&tmp.path().join(".codebus"), "concepts/foo.md", fm_clean(), "# foo");
    let snap_before = snapshot(&tmp.path().join(".codebus").join("wiki"));
    let _ = lint(tmp.path(), &[]);
    let snap_after = snapshot(&tmp.path().join(".codebus").join("wiki"));
    assert_eq!(snap_before, snap_after, "lint must not modify vault");
}

fn snapshot(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut snap = Vec::new();
    fn recurse(d: &Path, snap: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                recurse(&p, snap);
            } else {
                let data = std::fs::read(&p).unwrap_or_default();
                snap.push((p, data));
            }
        }
    }
    recurse(dir, &mut snap);
    snap.sort_by(|a, b| a.0.cmp(&b.0));
    snap
}

/// v3-bug-fixes Bug 2: `lint --repo <vault-root>` and `lint --repo <source-repo>`
/// SHALL produce identical stdout against the same vault. Pre-fix the
/// `<vault-root>` form silently scanned 0 pages because `locate_vault_root`
/// joined `.codebus` again, producing `.codebus/.codebus` (non-existent).
#[test]
fn lint_repo_pointing_at_vault_root_works_same_as_source_repo() {
    let tmp = TempDir::new().unwrap();
    init_vault(tmp.path());

    // Plant a broken-wikilink warning so both invocations have a real
    // finding to surface (not just the empty "no issues" baseline).
    let vault = tmp.path().join(".codebus");
    write_page(&vault, "concepts/foo.md", fm_clean(), "see [[ghost-page]]\n");

    let source_form = lint(tmp.path(), &["--repo", tmp.path().to_str().unwrap()]);
    let vault_form = lint(
        tmp.path(),
        &["--repo", vault.to_str().unwrap()],
    );

    assert_eq!(
        source_form.status.code(),
        vault_form.status.code(),
        "exit codes differ; source stdout: {:?} vault stdout: {:?}",
        String::from_utf8_lossy(&source_form.stdout),
        String::from_utf8_lossy(&vault_form.stdout),
    );

    let source_stdout = String::from_utf8_lossy(&source_form.stdout);
    let vault_stdout = String::from_utf8_lossy(&vault_form.stdout);
    assert_eq!(
        source_stdout, vault_stdout,
        "lint stdout differs between --repo <source> and --repo <vault-root>"
    );
    // Both forms must surface the planted broken-wikilink warning.
    assert!(
        source_stdout.contains("ghost-page"),
        "expected broken-wikilink warning in stdout, got: {source_stdout}"
    );
}
