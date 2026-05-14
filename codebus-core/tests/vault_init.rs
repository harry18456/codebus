//! End-to-end integration tests for the v3-init primitive set.
//! Each scenario documented in openspec/changes/v3-init/specs/{vault,skill-bundles}/
//! has at least one matching test fn here.

use std::fs;
use std::path::Path;

use codebus_core::pii::provider::OnHit;
use codebus_core::pii::scanners::null_scanner::NullScanner;
use codebus_core::pii::scanners::regex_basic::RegexBasicScanner;
use codebus_core::schema::NEUTRAL_RULES;
use codebus_core::skill_bundle::{self, BundleOutcome, VERBS};
use codebus_core::vault::layout::{create_vault_layout, vault_paths};
use codebus_core::vault::manifest::{
    ManifestOutcome, SourceSignal, compute_source_signal, write_or_update_manifest,
};
use codebus_core::vault::obsidian_register::{RegisterOutcome, register_at};
use codebus_core::vault::raw_sync::SyncSummary;
use codebus_core::vault::raw_sync::{sync_with_scanner, sync_with_scanner_into};
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use codebus_core::vault::source_gitignore::{GitignoreOutcome, ensure_codebus_in_gitignore};
use tempfile::TempDir;

fn write(p: &Path, content: &[u8]) {
    if let Some(par) = p.parent() {
        fs::create_dir_all(par).unwrap();
    }
    fs::write(p, content).unwrap();
}

// ===== Vault Layout =====

#[test]
fn vault_layout_creates_seven_required_subdirs_and_no_legacy_paths() {
    let tmp = TempDir::new().unwrap();
    let p = create_vault_layout(tmp.path()).unwrap();
    for sub in [
        "wiki/concepts",
        "wiki/entities",
        "wiki/modules",
        "wiki/processes",
        "wiki/synthesis",
        "raw/code",
        "log",
    ] {
        assert!(p.root.join(sub).is_dir(), "missing {sub}");
    }
    assert!(!p.root.join("output").exists());
    assert!(!p.root.join("goals.jsonl").exists());
    assert!(!p.root.join(".git").exists());
}

#[test]
fn vault_layout_idempotent_re_run() {
    let tmp = TempDir::new().unwrap();
    create_vault_layout(tmp.path()).unwrap();
    let sentinel = tmp.path().join(".codebus/wiki/concepts/keep.txt");
    fs::write(&sentinel, "stay").unwrap();
    create_vault_layout(tmp.path()).unwrap();
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "stay");
}

// ===== Sanity Check Inside Vault =====

#[test]
fn sanity_check_refuses_directory_named_dot_codebus() {
    let tmp = TempDir::new().unwrap();
    let nested = tmp.path().join(".codebus");
    fs::create_dir_all(&nested).unwrap();
    assert!(check_repo_is_not_vault(&nested).is_err());
}

#[test]
fn sanity_check_refuses_vault_root() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("wiki")).unwrap();
    fs::write(tmp.path().join("manifest.yaml"), "x: 1").unwrap();
    assert!(check_repo_is_not_vault(tmp.path()).is_err());
}

#[test]
fn sanity_check_accepts_normal_repo() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), "hi").unwrap();
    assert!(check_repo_is_not_vault(tmp.path()).is_ok());
}

// ===== Raw Mirror with NullScanner =====

#[test]
fn raw_mirror_preserves_structure_and_skips_top_level_dot_dirs() {
    let tmp = TempDir::new().unwrap();
    let raw = TempDir::new().unwrap();
    write(&tmp.path().join("src/main.rs"), b"fn main(){}");
    write(&tmp.path().join("nested/lib.rs"), b"// b");
    write(&tmp.path().join(".git/config"), b"[core]");
    write(&tmp.path().join(".env"), b"X=Y");
    write(&tmp.path().join(".codebus/manifest.yaml"), b"v: 1");

    sync_with_scanner(tmp.path(), raw.path(), &NullScanner::new(), OnHit::Warn).unwrap();
    assert!(raw.path().join("src/main.rs").exists());
    assert!(raw.path().join("nested/lib.rs").exists());
    assert!(!raw.path().join(".git").exists());
    assert!(!raw.path().join(".env").exists());
    assert!(!raw.path().join(".codebus").exists());
}

#[test]
fn raw_mirror_honors_source_gitignore() {
    let tmp = TempDir::new().unwrap();
    let raw = TempDir::new().unwrap();
    write(&tmp.path().join(".gitignore"), b"target/\n");
    write(&tmp.path().join("src/foo.rs"), b"fn foo(){}");
    write(
        &tmp.path().join("target/debug/foo.rs"),
        b"// build artifact",
    );
    sync_with_scanner(tmp.path(), raw.path(), &NullScanner::new(), OnHit::Warn).unwrap();
    assert!(raw.path().join("src/foo.rs").exists());
    assert!(!raw.path().join("target").exists());
}

// ===== Raw Mirror with PII Scanner — integration =====

#[test]
fn raw_sync_emits_warnings_for_known_pii_patterns() {
    let src = TempDir::new().unwrap();
    let raw = TempDir::new().unwrap();
    write(
        &src.path().join("src/aws.py"),
        b"AWS_KEY=AKIAIOSFODNN7EXAMPLE\n",
    );
    write(
        &src.path().join("docs/contact.md"),
        b"contact alice@example.com please",
    );
    write(&src.path().join("docs/net.md"), b"server at 192.168.1.42");

    let scanner = RegexBasicScanner::new(&[]).expect("builtin patterns must compile");
    let mut warn_buf: Vec<u8> = Vec::new();
    let summary =
        sync_with_scanner_into(src.path(), raw.path(), &scanner, OnHit::Warn, &mut warn_buf)
            .unwrap();

    let warn_text = String::from_utf8(warn_buf).expect("warn output should be valid UTF-8");
    let warn_lines: Vec<&str> = warn_text.lines().filter(|l| !l.is_empty()).collect();

    // Three input matches → three warning lines, all with the canonical prefix.
    assert_eq!(
        warn_lines.len(),
        3,
        "expected 3 pii warn lines, got: {warn_lines:?}"
    );
    for line in &warn_lines {
        assert!(
            line.starts_with("pii warn:"),
            "line should start with 'pii warn:': {line}"
        );
    }

    // Each pattern_name appears in at least one line.
    assert!(
        warn_lines.iter().any(|l| l.contains("aws-access-key")),
        "missing aws-access-key in {warn_lines:?}"
    );
    assert!(
        warn_lines.iter().any(|l| l.contains("email")),
        "missing email in {warn_lines:?}"
    );
    assert!(
        warn_lines.iter().any(|l| l.contains("ipv4")),
        "missing ipv4 in {warn_lines:?}"
    );

    // Each path appears in its respective warning line.
    assert!(
        warn_lines.iter().any(|l| l.contains("src/aws.py")),
        "missing src/aws.py path in {warn_lines:?}"
    );
    assert!(
        warn_lines.iter().any(|l| l.contains("docs/contact.md")),
        "missing docs/contact.md path in {warn_lines:?}"
    );
    assert!(
        warn_lines.iter().any(|l| l.contains("docs/net.md")),
        "missing docs/net.md path in {warn_lines:?}"
    );

    // matched_text MUST NOT appear in warning output (redaction intent).
    assert!(
        !warn_text.contains("AKIAIOSFODNN7EXAMPLE"),
        "AWS key literal leaked into warn output: {warn_text}"
    );
    assert!(
        !warn_text.contains("alice@example.com"),
        "email literal leaked into warn output: {warn_text}"
    );
    assert!(
        !warn_text.contains("192.168.1.42"),
        "ipv4 literal leaked into warn output: {warn_text}"
    );

    // Files are still mirrored unchanged (Warn on-hit policy).
    assert!(raw.path().join("src/aws.py").exists());
    assert!(raw.path().join("docs/contact.md").exists());
    assert!(raw.path().join("docs/net.md").exists());

    // Summary aggregates total match count.
    assert_eq!(summary.pii_matches, 3);
}

// ===== Source Repo .gitignore Mutation =====

#[test]
fn source_gitignore_creates_when_missing_in_git_repo() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
    assert_eq!(outcome, GitignoreOutcome::Created);
    let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    // v3-lint added 4 required lines: .codebus/ + 3 skill bundle paths
    assert!(body.contains(".codebus/\n"));
    assert!(body.contains(".claude/skills/codebus-goal/"));
    assert!(body.contains(".claude/skills/codebus-query/"));
    assert!(body.contains(".claude/skills/codebus-fix/"));
}

#[test]
fn source_gitignore_appends_to_existing() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
    ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
    let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(body.starts_with("node_modules\n"));
    assert!(body.contains(".codebus/\n"));
    assert!(body.contains(".claude/skills/codebus-goal/"));
}

#[test]
fn source_gitignore_skips_non_git_directory() {
    let tmp = TempDir::new().unwrap();
    let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
    assert_eq!(outcome, GitignoreOutcome::NotAGitRepo);
    assert!(!tmp.path().join(".gitignore").exists());
}

// ===== Per-Repo Schema File =====

#[test]
fn schema_file_write_if_missing_writes_taxonomy_content() {
    let tmp = TempDir::new().unwrap();
    let p = create_vault_layout(tmp.path()).unwrap();
    fs::write(&p.schema_md, NEUTRAL_RULES).unwrap();
    let body = fs::read_to_string(&p.schema_md).unwrap();
    for folder in ["concepts", "entities", "modules", "processes", "synthesis"] {
        assert!(body.contains(folder), "missing {folder}");
    }
}

#[test]
fn schema_content_is_vendor_neutral() {
    let lower = NEUTRAL_RULES.to_lowercase();
    for token in [
        "claude",
        "anthropic",
        "stream-json",
        "--tools",
        "codex",
        "gemini",
        "cursor",
    ] {
        assert!(!lower.contains(token), "vendor token leaked: {token}");
    }
}

// ===== Vault Manifest Records Sync State =====

fn dummy_signal(file_count: usize, total_bytes: u64) -> SourceSignal {
    SourceSignal {
        git_head: None,
        file_count,
        total_bytes,
    }
}

#[test]
fn manifest_records_meta_and_sync_state_on_first_init() {
    let tmp = TempDir::new().unwrap();
    let p = create_vault_layout(tmp.path()).unwrap();
    let outcome =
        write_or_update_manifest(tmp.path(), &p.root, "0.3.0-test", dummy_signal(42, 1234))
            .unwrap();
    assert_eq!(outcome, ManifestOutcome::Written);

    let body = fs::read_to_string(&p.manifest_yaml).unwrap();
    let yaml: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
    let map = yaml.as_mapping().unwrap();
    assert_eq!(map.len(), 5);
    for key in [
        "codebus_version",
        "created_at",
        "repo_root",
        "last_sync_at",
        "source_signal",
    ] {
        assert!(
            map.contains_key(serde_yaml::Value::String(key.into())),
            "missing top-level key `{key}`"
        );
    }
    let sig = map
        .get(serde_yaml::Value::String("source_signal".into()))
        .and_then(|v| v.as_mapping())
        .unwrap();
    assert_eq!(sig.len(), 3);
    for key in ["git_head", "file_count", "total_bytes"] {
        assert!(sig.contains_key(serde_yaml::Value::String(key.into())));
    }
    assert_eq!(
        sig.get(serde_yaml::Value::String("file_count".into()))
            .and_then(|v| v.as_u64()),
        Some(42)
    );
    let created = map
        .get(serde_yaml::Value::String("created_at".into()))
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(created.ends_with('Z') && created.contains('T'));
}

#[test]
fn manifest_re_init_preserves_write_once_and_updates_sync_state() {
    let tmp = TempDir::new().unwrap();
    let p = create_vault_layout(tmp.path()).unwrap();
    write_or_update_manifest(tmp.path(), &p.root, "0.3.0-first", dummy_signal(10, 1000)).unwrap();
    let body_first = fs::read_to_string(&p.manifest_yaml).unwrap();
    let parsed_first: serde_yaml::Value = serde_yaml::from_str(&body_first).unwrap();

    let outcome2 =
        write_or_update_manifest(tmp.path(), &p.root, "0.4.0-second", dummy_signal(20, 2000))
            .unwrap();
    assert_eq!(outcome2, ManifestOutcome::Updated);

    let body_second = fs::read_to_string(&p.manifest_yaml).unwrap();
    let parsed_second: serde_yaml::Value = serde_yaml::from_str(&body_second).unwrap();

    // Write-once fields preserved: codebus_version stays at "0.3.0-first" not "0.4.0-second"
    assert_eq!(
        parsed_second
            .get("codebus_version")
            .and_then(|v| v.as_str()),
        Some("0.3.0-first")
    );
    assert_eq!(
        parsed_second.get("created_at"),
        parsed_first.get("created_at")
    );
    assert_eq!(
        parsed_second.get("repo_root"),
        parsed_first.get("repo_root")
    );

    // Sync state updated
    let sig = parsed_second.get("source_signal").unwrap();
    assert_eq!(sig.get("file_count").and_then(|v| v.as_u64()), Some(20));
    assert_eq!(sig.get("total_bytes").and_then(|v| v.as_u64()), Some(2000));
}

#[test]
fn manifest_source_signal_handles_git_repo() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    let head_content = "ref: refs/heads/main\n";
    fs::write(tmp.path().join(".git/HEAD"), head_content).unwrap();

    let summary = SyncSummary {
        files: 5,
        bytes: 500,
        pii_matches: 0,
        pii_skipped_files: 0,
        pii_masked_matches: 0,
    };
    let signal = compute_source_signal(tmp.path(), &summary);
    assert_eq!(signal.git_head.as_deref(), Some(head_content));
    assert_eq!(signal.file_count, 5);
    assert_eq!(signal.total_bytes, 500);
}

#[test]
fn manifest_source_signal_handles_non_git_dir() {
    let tmp = TempDir::new().unwrap();
    let summary = SyncSummary {
        files: 3,
        bytes: 100,
        pii_matches: 0,
        pii_skipped_files: 0,
        pii_masked_matches: 0,
    };
    let signal = compute_source_signal(tmp.path(), &summary);
    assert!(signal.git_head.is_none());
    assert_eq!(signal.file_count, 3);
    assert_eq!(signal.total_bytes, 100);
}

#[test]
fn manifest_null_git_head_serializes_to_yaml_null() {
    let tmp = TempDir::new().unwrap();
    let p = create_vault_layout(tmp.path()).unwrap();
    write_or_update_manifest(tmp.path(), &p.root, "0.3", dummy_signal(1, 1)).unwrap();
    let body = fs::read_to_string(&p.manifest_yaml).unwrap();
    let v: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
    let head = v.get("source_signal").unwrap().get("git_head").unwrap();
    assert!(head.is_null(), "expected null git_head, got {head:?}");
}

// ===== Obsidian Vault Auto-Registration (fail-soft) =====

#[test]
fn obsidian_register_fail_soft_handles_missing_config_dir() {
    let tmp = TempDir::new().unwrap();
    let json = tmp.path().join("nonexistent_dir/obsidian.json");
    let wiki = tmp.path().join("repo/.codebus/wiki");
    fs::create_dir_all(&wiki).unwrap();
    // register_at creates parent dirs and writes the config file. The
    // public entry register_vault returns ObsidianNotInstalled when the
    // ~/.config/obsidian directory is absent (covered by the public API
    // contract; here we just confirm register_at does not panic and does
    // not propagate I/O errors as panic).
    let outcome = register_at(&wiki, &json);
    assert!(matches!(
        outcome,
        RegisterOutcome::Registered { .. } | RegisterOutcome::IoError { .. }
    ));
}

// ===== Skill Bundle Layout / Content / Write-If-Missing =====

/// Helper: build distinct (vault, repo) paths under one TempDir for v3-lint
/// dual-location skill bundle write semantics.
fn dual_layout(tmp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let repo = tmp.path().to_path_buf();
    let vault = repo.join(".codebus");
    (vault, repo)
}

#[test]
fn skill_bundles_creates_eight_outcomes_no_lint_at_either_location() {
    let tmp = TempDir::new().unwrap();
    let (vault, repo) = dual_layout(&tmp);
    let outcomes = skill_bundle::write_bundles_if_missing(&vault, &repo, true).unwrap();
    // v3-chat-verb: 4 verbs (goal/query/fix/chat) × 2 locations = 8 outcomes.
    assert_eq!(outcomes.len(), 8);
    for outcome in &outcomes {
        assert_eq!(*outcome, BundleOutcome::Written);
    }
    for verb in VERBS {
        assert!(
            vault
                .join(format!(".claude/skills/codebus-{verb}/SKILL.md"))
                .exists(),
            "missing vault bundle for {verb}"
        );
        assert!(
            repo.join(format!(".claude/skills/codebus-{verb}/SKILL.md"))
                .exists(),
            "missing repo-root bundle for {verb}"
        );
    }
    assert!(!vault.join(".claude/skills/codebus-lint").exists());
    assert!(!repo.join(".claude/skills/codebus-lint").exists());
}

#[test]
fn skill_bundle_stub_content_has_required_format_at_both_locations() {
    let tmp = TempDir::new().unwrap();
    let (vault, repo) = dual_layout(&tmp);
    skill_bundle::write_bundles_if_missing(&vault, &repo, true).unwrap();
    for verb in VERBS {
        for base in [&vault, &repo] {
            let path = base.join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
            let body = fs::read_to_string(&path).unwrap();
            assert!(body.starts_with("---\n"));
            assert!(body.contains(&format!("name: codebus-{verb}")));
            assert!(body.contains("description:"));
            assert!(body.contains("CLAUDE.md"));
            assert!(!body.contains(".codebus/CLAUDE.md"));
            // chat SKILL is intentionally longer than goal/query/fix; widen
            // the line cap accordingly.
            let line_cap = if *verb == "chat" { 120 } else { 80 };
            assert!(
                body.lines().count() <= line_cap,
                "verb `{verb}` SKILL.md too long ({} > {line_cap})",
                body.lines().count()
            );
        }
    }
}

#[test]
fn skill_bundle_stub_body_declares_hard_scope() {
    let tmp = TempDir::new().unwrap();
    let (vault, repo) = dual_layout(&tmp);
    skill_bundle::write_bundles_if_missing(&vault, &repo, true).unwrap();
    for verb in VERBS {
        let path = vault.join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("`raw/code/`"),
            "verb `{verb}` missing cwd-relative read scope `raw/code/`"
        );
        assert!(
            body.contains("`wiki/`"),
            "verb `{verb}` missing cwd-relative wiki scope `wiki/`"
        );
        assert!(
            !body.contains(".codebus/raw/code/") && !body.contains(".codebus/wiki/"),
            "verb `{verb}` should not use `.codebus/`-prefixed paths in scope"
        );
        // chat is read-only and phrases the prohibition as
        // "MUST NOT read any path that escapes the cwd" (no write half);
        // the other three verbs share the "read or write" form. Assert on
        // the common substring instead of the exact phrase.
        assert!(
            body.contains("MUST NOT") && body.contains("escapes the cwd"),
            "verb `{verb}` missing hard-scope prohibition"
        );
    }
}

#[test]
fn skill_bundle_stub_body_declares_path_translation_rule() {
    let tmp = TempDir::new().unwrap();
    let (vault, repo) = dual_layout(&tmp);
    skill_bundle::write_bundles_if_missing(&vault, &repo, true).unwrap();
    // Path translation is meaningful only for write-capable verbs; chat is
    // multi-turn read-only and never cites a source path in wiki frontmatter,
    // so the rule does not apply.
    for verb in VERBS.iter().filter(|v| **v != "chat") {
        let path = vault.join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("repo-relative logical path"));
        assert!(body.contains("NOT the mirrored path"));
    }
}

#[test]
fn skill_bundle_write_if_missing_preserves_existing_at_each_location() {
    let tmp = TempDir::new().unwrap();
    let (vault, repo) = dual_layout(&tmp);
    let goal_vault_path = vault.join(".claude/skills/codebus-goal/SKILL.md");
    fs::create_dir_all(goal_vault_path.parent().unwrap()).unwrap();
    fs::write(
        &goal_vault_path,
        "---\nname: codebus-goal\n---\nuser custom",
    )
    .unwrap();
    let outcomes = skill_bundle::write_bundles_if_missing(&vault, &repo, true).unwrap();
    // Vault: goal preserved (idx 0), query/fix written (idx 1, 2)
    assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
    assert_eq!(outcomes[1], BundleOutcome::Written);
    assert_eq!(outcomes[2], BundleOutcome::Written);
    // Repo-root: all written independently (idx 3, 4, 5)
    assert_eq!(outcomes[3], BundleOutcome::Written);
    assert_eq!(outcomes[4], BundleOutcome::Written);
    assert_eq!(outcomes[5], BundleOutcome::Written);
    assert!(
        fs::read_to_string(&goal_vault_path)
            .unwrap()
            .contains("user custom")
    );
}

// ===== Sanity wiring: vault_paths agrees with create_vault_layout =====

#[test]
fn vault_paths_resolves_consistently_with_create_layout() {
    let tmp = TempDir::new().unwrap();
    let p1 = vault_paths(tmp.path());
    let p2 = create_vault_layout(tmp.path()).unwrap();
    assert_eq!(p1.root, p2.root);
    assert_eq!(p1.wiki_concepts, p2.wiki_concepts);
    assert_eq!(p1.raw_code, p2.raw_code);
    assert_eq!(p1.schema_md, p2.schema_md);
    assert_eq!(p1.manifest_yaml, p2.manifest_yaml);
}

// ===== v3-fix-trust-agent: settings.json + hook installation =====

use codebus_core::vault::settings::{self, SettingsOutcome};

#[test]
fn settings_json_writer_creates_at_vault_internal_path() {
    let tmp = TempDir::new().unwrap();
    let outcome = settings::write_settings_if_missing(tmp.path()).unwrap();
    assert_eq!(outcome, SettingsOutcome::Written);
    let p = settings::settings_json_path(tmp.path());
    assert!(p.exists(), "settings.json missing at {p:?}");
}

#[test]
fn settings_json_content_has_pretooluse_bash_hook_invoking_codebus_hook_check_bash() {
    let tmp = TempDir::new().unwrap();
    settings::write_settings_if_missing(tmp.path()).unwrap();
    let body = fs::read_to_string(settings::settings_json_path(tmp.path())).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("settings.json must parse");
    let entries = parsed["hooks"]["PreToolUse"]
        .as_array()
        .expect("PreToolUse must be array");
    assert!(!entries.is_empty(), "PreToolUse must have entries");
    assert_eq!(entries[0]["matcher"], "Bash");
    let nested = entries[0]["hooks"].as_array().unwrap();
    assert_eq!(nested[0]["type"], "command");
    assert_eq!(nested[0]["command"], "codebus hook check-bash");
}

#[test]
fn settings_json_writer_preserves_existing_user_customization() {
    let tmp = TempDir::new().unwrap();
    let custom = r#"{"hooks":{"PreToolUse":[]},"my_field":"keep me"}"#;
    let p = settings::settings_json_path(tmp.path());
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, custom).unwrap();
    let outcome = settings::write_settings_if_missing(tmp.path()).unwrap();
    assert_eq!(outcome, SettingsOutcome::AlreadyPresent);
    assert_eq!(fs::read_to_string(&p).unwrap(), custom, "byte-identical");
}
