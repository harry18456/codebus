use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVersion {
    pub commit: Option<String>,
    pub uncommitted: bool,
}

/// Read a source repo's git state. `commit = None` when the directory is
/// not a git repo (no `.git/` at root). `uncommitted = true` when
/// `git status --porcelain` reports any line.
pub fn get_source_version(repo_root: impl AsRef<Path>) -> SourceVersion {
    let repo_root = repo_root.as_ref();
    if !repo_root.join(".git").exists() {
        return SourceVersion {
            commit: None,
            uncommitted: false,
        };
    }

    let commit = run_git(repo_root, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string());
    let status = run_git(repo_root, &["status", "--porcelain"]).unwrap_or_default();
    let uncommitted = !status.trim().is_empty();

    SourceVersion {
        commit,
        uncommitted,
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-srcver-{name}-{}-{}",
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

    fn run(cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("git command");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_repo(p: &Path) {
        run(p, &["init", "-b", "main", "-q"]);
        run(p, &["config", "user.email", "t@t"]);
        run(p, &["config", "user.name", "tester"]);
    }

    #[test]
    fn non_git_dir_returns_none_commit() {
        let p = tmp("nogit");
        let v = get_source_version(&p);
        assert_eq!(v.commit, None);
        assert_eq!(v.uncommitted, false);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn fresh_commit_returns_clean_state() {
        let p = tmp("fresh");
        init_repo(&p);
        fs::write(p.join("a.txt"), "x").unwrap();
        run(&p, &["add", "."]);
        run(&p, &["commit", "-m", "init", "-q"]);
        let v = get_source_version(&p);
        assert!(
            v.commit.as_deref().map(|s| s.len() == 40).unwrap_or(false),
            "expected 40-char sha, got {:?}",
            v.commit
        );
        assert_eq!(v.uncommitted, false);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn untracked_file_marks_uncommitted() {
        let p = tmp("untracked");
        init_repo(&p);
        fs::write(p.join("a.txt"), "x").unwrap();
        run(&p, &["add", "."]);
        run(&p, &["commit", "-m", "init", "-q"]);
        fs::write(p.join("b.txt"), "y").unwrap();
        let v = get_source_version(&p);
        assert!(v.commit.is_some());
        assert_eq!(v.uncommitted, true);
        let _ = fs::remove_dir_all(&p);
    }

    #[test]
    fn modified_tracked_file_marks_uncommitted() {
        let p = tmp("modified");
        init_repo(&p);
        fs::write(p.join("a.txt"), "x").unwrap();
        run(&p, &["add", "."]);
        run(&p, &["commit", "-m", "init", "-q"]);
        fs::write(p.join("a.txt"), "y").unwrap();
        let v = get_source_version(&p);
        assert_eq!(v.uncommitted, true);
        let _ = fs::remove_dir_all(&p);
    }
}
