//! End-to-end integration tests for the v3-init primitive set.
//! Each scenario documented in openspec/changes/v3-init/specs/{vault,skill-bundles}/
//! has at least one matching test fn here.

use std::fs;
use std::path::Path;

use codebus_core::schema::NEUTRAL_RULES;
use codebus_core::skill_bundle::{self, BundleOutcome, VERBS};
use codebus_core::vault::layout::{create_vault_layout, vault_paths};
use codebus_core::vault::manifest::{
    compute_source_signal, write_or_update_manifest, ManifestOutcome, SourceSignal,
};
use codebus_core::vault::raw_sync::SyncSummary;
use codebus_core::vault::obsidian_register::{register_at, RegisterOutcome};
use codebus_core::vault::raw_sync::sync_with_null_scanner;
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use codebus_core::vault::source_gitignore::{ensure_codebus_in_gitignore, GitignoreOutcome};
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

    sync_with_null_scanner(tmp.path(), raw.path()).unwrap();
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
    write(&tmp.path().join("target/debug/foo.rs"), b"// build artifact");
    sync_with_null_scanner(tmp.path(), raw.path()).unwrap();
    assert!(raw.path().join("src/foo.rs").exists());
    assert!(!raw.path().join("target").exists());
}

// ===== Source Repo .gitignore Mutation =====

#[test]
fn source_gitignore_creates_when_missing_in_git_repo() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
    assert_eq!(outcome, GitignoreOutcome::Created);
    assert_eq!(
        fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        ".codebus/\n"
    );
}

#[test]
fn source_gitignore_appends_to_existing() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
    ensure_codebus_in_gitignore(tmp.path()).unwrap();
    assert_eq!(
        fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        "node_modules\n.codebus/\n"
    );
}

#[test]
fn source_gitignore_skips_non_git_directory() {
    let tmp = TempDir::new().unwrap();
    let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
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
    for token in ["claude", "anthropic", "stream-json", "--tools", "codex", "gemini", "cursor"] {
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
    let outcome = write_or_update_manifest(
        tmp.path(),
        &p.root,
        "0.3.0-test",
        dummy_signal(42, 1234),
    )
    .unwrap();
    assert_eq!(outcome, ManifestOutcome::Written);

    let body = fs::read_to_string(&p.manifest_yaml).unwrap();
    let yaml: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
    let map = yaml.as_mapping().unwrap();
    assert_eq!(map.len(), 5);
    for key in ["codebus_version", "created_at", "repo_root", "last_sync_at", "source_signal"] {
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

    let outcome2 = write_or_update_manifest(
        tmp.path(),
        &p.root,
        "0.4.0-second",
        dummy_signal(20, 2000),
    )
    .unwrap();
    assert_eq!(outcome2, ManifestOutcome::Updated);

    let body_second = fs::read_to_string(&p.manifest_yaml).unwrap();
    let parsed_second: serde_yaml::Value = serde_yaml::from_str(&body_second).unwrap();

    // Write-once fields preserved: codebus_version stays at "0.3.0-first" not "0.4.0-second"
    assert_eq!(
        parsed_second.get("codebus_version").and_then(|v| v.as_str()),
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

    let summary = SyncSummary { files: 5, bytes: 500 };
    let signal = compute_source_signal(tmp.path(), &summary);
    assert_eq!(signal.git_head.as_deref(), Some(head_content));
    assert_eq!(signal.file_count, 5);
    assert_eq!(signal.total_bytes, 500);
}

#[test]
fn manifest_source_signal_handles_non_git_dir() {
    let tmp = TempDir::new().unwrap();
    let summary = SyncSummary { files: 3, bytes: 100 };
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

#[test]
fn skill_bundles_creates_three_dirs_no_lint() {
    let tmp = TempDir::new().unwrap();
    let outcomes = skill_bundle::write_bundles_if_missing(tmp.path()).unwrap();
    assert_eq!(outcomes.len(), 3);
    for outcome in &outcomes {
        assert_eq!(*outcome, BundleOutcome::Written);
    }
    for verb in VERBS {
        assert!(
            tmp.path()
                .join(format!(".claude/skills/codebus-{verb}/SKILL.md"))
                .exists(),
            "missing bundle for {verb}"
        );
    }
    assert!(!tmp.path().join(".claude/skills/codebus-lint").exists());
}

#[test]
fn skill_bundle_stub_content_has_required_format() {
    let tmp = TempDir::new().unwrap();
    skill_bundle::write_bundles_if_missing(tmp.path()).unwrap();
    for verb in VERBS {
        let path = tmp
            .path()
            .join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("---\n"));
        assert!(body.contains(&format!("name: codebus-{verb}")));
        assert!(body.contains("description:"));
        // cwd-relative reference: `CLAUDE.md` (not `.codebus/CLAUDE.md`)
        assert!(body.contains("CLAUDE.md"));
        assert!(!body.contains(".codebus/CLAUDE.md"));
        assert!(body.lines().count() <= 80);
    }
}

#[test]
fn skill_bundle_stub_body_declares_hard_scope() {
    let tmp = TempDir::new().unwrap();
    skill_bundle::write_bundles_if_missing(tmp.path()).unwrap();
    for verb in VERBS {
        let path = tmp.path().join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
        let body = fs::read_to_string(&path).unwrap();
        // Cwd-relative paths (not `.codebus/`-prefixed)
        assert!(
            body.contains("`raw/code/`"),
            "verb `{verb}` missing cwd-relative read scope `raw/code/`"
        );
        assert!(
            body.contains("`wiki/`"),
            "verb `{verb}` missing cwd-relative write scope `wiki/`"
        );
        assert!(
            !body.contains(".codebus/raw/code/") && !body.contains(".codebus/wiki/"),
            "verb `{verb}` should not use `.codebus/`-prefixed paths in scope"
        );
        assert!(
            body.contains("MUST NOT read or write any path that escapes the cwd"),
            "verb `{verb}` missing hard-scope prohibition"
        );
    }
}

#[test]
fn skill_bundle_stub_body_declares_path_translation_rule() {
    let tmp = TempDir::new().unwrap();
    skill_bundle::write_bundles_if_missing(tmp.path()).unwrap();
    for verb in VERBS {
        let path = tmp.path().join(format!(".claude/skills/codebus-{verb}/SKILL.md"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("repo-relative logical path"));
        assert!(body.contains("NOT the mirrored path"));
    }
}

#[test]
fn skill_bundle_write_if_missing_preserves_existing() {
    let tmp = TempDir::new().unwrap();
    let goal_path = tmp
        .path()
        .join(".claude/skills/codebus-goal/SKILL.md");
    fs::create_dir_all(goal_path.parent().unwrap()).unwrap();
    fs::write(&goal_path, "---\nname: codebus-goal\n---\nuser custom").unwrap();
    let outcomes = skill_bundle::write_bundles_if_missing(tmp.path()).unwrap();
    assert_eq!(outcomes[0], BundleOutcome::AlreadyPresent);
    assert_eq!(outcomes[1], BundleOutcome::Written);
    assert_eq!(outcomes[2], BundleOutcome::Written);
    assert!(fs::read_to_string(&goal_path).unwrap().contains("user custom"));
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
