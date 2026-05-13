//! Append `.codebus/` and the repo-root skill bundle directories to source
//! repo's root `.gitignore` when init runs against a git repo. Idempotent;
//! non-git repos are skipped.
//!
//! v3-lint adds `.claude/skills/codebus-{goal,query,fix}/` so the dual-write
//! skill bundles at the source repo root don't pollute source git history.
//! v3-chat-verb adds `.claude/skills/codebus-chat/` for the fourth bundle.

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

/// Lines codebus init ensures are present in the source repo's `.gitignore`.
/// Order is the canonical declaration order — used both for first-time
/// creation and for appending missing lines while preserving existing
/// non-codebus entries.
pub const REQUIRED_IGNORE_LINES: &[&str] = &[
    ".codebus/",
    ".claude/skills/codebus-goal/",
    ".claude/skills/codebus-query/",
    ".claude/skills/codebus-fix/",
    ".claude/skills/codebus-chat/",
];

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
            let mut body = String::new();
            for line in REQUIRED_IGNORE_LINES {
                body.push_str(line);
                body.push('\n');
            }
            fs::write(&gi_path, body)?;
            Ok(GitignoreOutcome::Created)
        }
        Some(body) => {
            let present: std::collections::HashSet<&str> = body.lines().collect();
            let missing: Vec<&&str> = REQUIRED_IGNORE_LINES
                .iter()
                .filter(|l| !present.contains(*l as &str))
                .collect();
            if missing.is_empty() {
                Ok(GitignoreOutcome::AlreadyPresent)
            } else {
                let mut next = body;
                if !next.ends_with('\n') {
                    next.push('\n');
                }
                for line in missing {
                    next.push_str(line);
                    next.push('\n');
                }
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

    fn assert_all_required_present(body: &str) {
        for line in REQUIRED_IGNORE_LINES {
            assert!(
                body.lines().any(|l| l == *line),
                ".gitignore missing required line `{line}`:\n{body}"
            );
        }
    }

    #[test]
    fn creates_gitignore_with_all_required_lines_when_missing() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Created);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_all_required_present(&body);
    }

    #[test]
    fn appends_missing_required_lines_preserving_existing_entries() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(body.starts_with("node_modules\n"));
        assert_all_required_present(&body);
    }

    #[test]
    fn idempotent_when_all_required_present() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let mut initial = String::from("node_modules\n");
        for line in REQUIRED_IGNORE_LINES {
            initial.push_str(line);
            initial.push('\n');
        }
        initial.push_str("target/\n");
        fs::write(tmp.path().join(".gitignore"), &initial).unwrap();
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
        assert!(body.starts_with("node_modules\n"));
        assert_all_required_present(&body);
    }

    #[test]
    fn partial_appends_only_missing_required_lines() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        // Existing has .codebus/ but lacks the .claude/skills/codebus-* trio
        fs::write(tmp.path().join(".gitignore"), ".codebus/\n").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path()).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_all_required_present(&body);
        // .codebus/ should not appear twice
        let count = body.lines().filter(|l| *l == ".codebus/").count();
        assert_eq!(count, 1);
    }
}
