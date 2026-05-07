use codebus_core::git::nested_repo::{auto_commit, init_nested_repo};
use codebus_core::schema::CODEBUS_SCHEMA;
use codebus_core::vault::layout::{VaultPaths, vault_paths};
use std::fs;
use std::io;
use std::path::Path;

const REQUIRED_INTERNAL_GITIGNORE_LINES: &[&str] =
    &[".lock", "raw/code/", "**/.obsidian/", "logs/"];

pub fn run_init(repo_root: impl AsRef<Path>) -> io::Result<()> {
    let repo_root = repo_root.as_ref();
    let p = vault_paths(repo_root);

    fs::create_dir_all(&p.root)?;
    fs::create_dir_all(&p.raw)?;
    fs::create_dir_all(&p.raw_code)?;
    fs::create_dir_all(&p.wiki)?;
    for folder in &p.wiki_page_folders {
        fs::create_dir_all(folder)?;
    }
    fs::create_dir_all(&p.output)?;

    if !p.schema_md.exists() {
        fs::write(&p.schema_md, CODEBUS_SCHEMA)?;
    }
    if !p.goals_jsonl.exists() {
        fs::write(&p.goals_jsonl, "")?;
    }

    merge_gitignore_lines(&p.gitignore, REQUIRED_INTERNAL_GITIGNORE_LINES)?;

    init_nested_repo(&p.root)?;

    if repo_root.join(".git").exists() {
        ensure_codebus_in_source_gitignore(repo_root)?;
    }

    auto_commit(&p.root, "init: codebus vault").map_err(|e| io::Error::other(e.to_string()))?;
    let _ = p; // p borrowed only above
    Ok(())
}

fn merge_gitignore_lines(path: &Path, required: &[&str]) -> io::Result<()> {
    let existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let present: std::collections::HashSet<&str> = existing.lines().map(|l| l.trim()).collect();
    let missing: Vec<&&str> = required
        .iter()
        .filter(|l| !present.contains(*l as &str))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let needs_nl = !existing.is_empty() && !existing.ends_with('\n');
    let mut out = existing;
    if needs_nl {
        out.push('\n');
    }
    for l in missing {
        out.push_str(l);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn ensure_codebus_in_source_gitignore(repo_root: &Path) -> io::Result<()> {
    let gi_path = repo_root.join(".gitignore");
    let existing = if gi_path.exists() {
        fs::read_to_string(&gi_path)?
    } else {
        String::new()
    };
    let present: std::collections::HashSet<&str> = existing.lines().map(|l| l.trim()).collect();
    if present.contains(".codebus") {
        return Ok(());
    }
    let needs_nl = !existing.is_empty() && !existing.ends_with('\n');
    let mut out = existing;
    if needs_nl {
        out.push('\n');
    }
    out.push_str(".codebus\n");
    fs::write(&gi_path, out)?;
    Ok(())
}

#[allow(dead_code)]
pub fn paths(repo_root: impl AsRef<Path>) -> VaultPaths {
    vault_paths(repo_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-init-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn nanos() -> u32 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    #[test]
    fn init_creates_full_vault_skeleton() {
        let repo = tmp("skeleton");
        run_init(&repo).unwrap();
        let p = vault_paths(&repo);
        assert!(p.root.is_dir());
        assert!(p.raw.is_dir());
        assert!(p.raw_code.is_dir());
        assert!(p.wiki.is_dir());
        assert!(p.output.is_dir());
        for folder in &p.wiki_page_folders {
            assert!(folder.is_dir(), "missing {}", folder.display());
        }
        assert!(p.schema_md.exists());
        assert!(p.goals_jsonl.exists());
        assert!(p.git.exists());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn schema_content_matches_constant() {
        let repo = tmp("schema");
        run_init(&repo).unwrap();
        let p = vault_paths(&repo);
        let written = fs::read_to_string(&p.schema_md).unwrap();
        assert_eq!(written, CODEBUS_SCHEMA);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn schema_is_not_overwritten_on_reinit() {
        let repo = tmp("preserve");
        run_init(&repo).unwrap();
        let p = vault_paths(&repo);
        fs::write(&p.schema_md, "USER CUSTOMIZATION").unwrap();
        run_init(&repo).unwrap();
        assert_eq!(
            fs::read_to_string(&p.schema_md).unwrap(),
            "USER CUSTOMIZATION"
        );
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn internal_gitignore_lines_are_merged() {
        let repo = tmp("gimerge");
        run_init(&repo).unwrap();
        let p = vault_paths(&repo);
        let content = fs::read_to_string(&p.gitignore).unwrap();
        for line in REQUIRED_INTERNAL_GITIGNORE_LINES {
            assert!(
                content.lines().any(|l| l.trim() == *line),
                "missing {line} in {content:?}"
            );
        }
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn dotcodebus_added_to_source_gitignore_when_source_is_git() {
        let repo = tmp("srcgi");
        // make source a git repo
        std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(&repo)
            .status()
            .unwrap();
        run_init(&repo).unwrap();
        let gi = fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert!(gi.lines().any(|l| l.trim() == ".codebus"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn source_gitignore_untouched_when_source_is_not_git() {
        let repo = tmp("nongit");
        run_init(&repo).unwrap();
        assert!(!repo.join(".gitignore").exists());
        let _ = fs::remove_dir_all(&repo);
    }
}
