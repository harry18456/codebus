//! Append `.codebus/` to source repo's root `.gitignore` when init runs
//! against a git repo. Idempotent; non-git repos are skipped.

use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitignoreOutcome {
    NotAGitRepo,
    Created,
    Appended,
    AlreadyPresent,
}

const CODEBUS_IGNORE_LINE: &str = ".codebus/";

pub fn ensure_codebus_in_gitignore(repo_root: &Path) -> io::Result<GitignoreOutcome> {
    if !repo_root.join(".git").exists() {
        return Ok(GitignoreOutcome::NotAGitRepo);
    }

    let gi_path = repo_root.join(".gitignore");
    let existing = match fs::read_to_string(&gi_path) {
        Ok(s) => Some(s),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };

    match existing {
        None => {
            fs::write(&gi_path, format!("{CODEBUS_IGNORE_LINE}\n"))?;
            Ok(GitignoreOutcome::Created)
        }
        Some(body) => {
            if body.lines().any(|line| line == CODEBUS_IGNORE_LINE) {
                Ok(GitignoreOutcome::AlreadyPresent)
            } else {
                let mut next = body;
                if !next.ends_with('\n') {
                    next.push('\n');
                }
                next.push_str(CODEBUS_IGNORE_LINE);
                next.push('\n');
                fs::write(&gi_path, next)?;
                Ok(GitignoreOutcome::Appended)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_git_repo(root: &Path) {
        fs::create_dir_all(root.join(".git")).unwrap();
    }

    #[test]
    fn creates_gitignore_when_missing() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Created);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(body, ".codebus/\n");
    }

    #[test]
    fn appends_when_entry_missing() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(body, "node_modules\n.codebus/\n");
    }

    #[test]
    fn idempotent_when_entry_present() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let initial = "node_modules\n.codebus/\ntarget/\n";
        fs::write(tmp.path().join(".gitignore"), initial).unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::AlreadyPresent);
        assert_eq!(
            fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
            initial
        );
    }

    #[test]
    fn skips_when_not_a_git_repo() {
        let tmp = TempDir::new().unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::NotAGitRepo);
        assert!(!tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn appends_correctly_when_existing_lacks_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(tmp.path().join(".gitignore"), "node_modules").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_eq!(body, "node_modules\n.codebus/\n");
    }
}
