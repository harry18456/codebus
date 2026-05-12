//! `.codebus/` vault layout — 7 directories.
//!
//! Path D drops v2's `output/`, `goals.jsonl`, and nested `.git/`. See
//! `docs/v3-roadmap.md` §4 #2 and the `Vault Layout` requirement.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SUBDIRS: &[&str] = &[
    "wiki/concepts",
    "wiki/entities",
    "wiki/modules",
    "wiki/processes",
    "wiki/synthesis",
    "raw/code",
    "log",
];

/// Resolved absolute paths for the seven required vault subdirectories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPaths {
    pub root: PathBuf,
    pub wiki: PathBuf,
    pub wiki_concepts: PathBuf,
    pub wiki_entities: PathBuf,
    pub wiki_modules: PathBuf,
    pub wiki_processes: PathBuf,
    pub wiki_synthesis: PathBuf,
    pub raw_code: PathBuf,
    pub log: PathBuf,
    pub schema_md: PathBuf,
    pub manifest_yaml: PathBuf,
}

pub fn vault_paths(repo_root: &Path) -> VaultPaths {
    let root = repo_root.join(".codebus");
    let wiki = root.join("wiki");
    VaultPaths {
        wiki_concepts: wiki.join("concepts"),
        wiki_entities: wiki.join("entities"),
        wiki_modules: wiki.join("modules"),
        wiki_processes: wiki.join("processes"),
        wiki_synthesis: wiki.join("synthesis"),
        raw_code: root.join("raw").join("code"),
        log: root.join("log"),
        schema_md: root.join("CLAUDE.md"),
        manifest_yaml: root.join("manifest.yaml"),
        wiki,
        root,
    }
}

/// Create the 7-folder vault layout under `<repo_root>/.codebus/`.
/// Idempotent: re-running against an existing layout is a no-op.
pub fn create_vault_layout(repo_root: &Path) -> io::Result<VaultPaths> {
    let p = vault_paths(repo_root);
    for sub in SUBDIRS {
        fs::create_dir_all(p.root.join(sub))?;
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_vault_layout_creates_all_seven_subdirs() {
        let tmp = TempDir::new().unwrap();
        let p = create_vault_layout(tmp.path()).unwrap();
        for sub in SUBDIRS {
            assert!(p.root.join(sub).is_dir(), "missing dir: {sub}");
        }
    }

    #[test]
    fn create_vault_layout_does_not_create_v2_legacy_paths() {
        let tmp = TempDir::new().unwrap();
        let p = create_vault_layout(tmp.path()).unwrap();
        // v2 carved-out paths that v3 still rejects. Note: `.git/` is
        // intentionally NOT in this list — v3-vault-history (#4) flipped
        // the policy: nested git is created by init.rs, not by
        // create_vault_layout. layout itself only owns the 7 subdirs.
        assert!(!p.root.join("output").exists(), "v2 output/ must not exist");
        assert!(
            !p.root.join("goals.jsonl").exists(),
            "v2 goals.jsonl must not exist"
        );
    }

    #[test]
    fn create_vault_layout_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        create_vault_layout(tmp.path()).unwrap();
        // Drop a sentinel to detect over-eager re-creation
        let sentinel = tmp.path().join(".codebus/wiki/concepts/sentinel.txt");
        fs::write(&sentinel, "preserve me").unwrap();
        create_vault_layout(tmp.path()).unwrap();
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve me");
    }

    #[test]
    fn vault_paths_resolves_expected_locations() {
        let p = vault_paths(Path::new("/repo"));
        assert!(p.root.ends_with(".codebus"));
        assert!(p.schema_md.ends_with("CLAUDE.md"));
        assert!(p.manifest_yaml.ends_with("manifest.yaml"));
        assert!(p.wiki_concepts.ends_with("concepts"));
        assert!(p.raw_code.to_string_lossy().contains("raw"));
    }
}
