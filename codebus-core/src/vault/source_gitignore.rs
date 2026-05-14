//! Append `.codebus/` (and optionally the repo-root skill bundle
//! directories) to source repo's root `.gitignore` when init runs
//! against a git repo. Idempotent; non-git repos are skipped.
//!
//! v3-skill-bundles-vault-only: the `.claude/skills/codebus-*/` patterns
//! are now conditional on the `include_skill_bundles` flag passed from
//! `vault::init::run_init`, mirroring whether init will materialize
//! repo-root skill bundles. The `.codebus/` line is unconditional.

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

/// The single unconditional line every codebus-init must add to the
/// source repo's `.gitignore`. Vault bytes live under `.codebus/` and
/// must never be tracked by the source repo, regardless of whether
/// repo-root skill bundles are written.
pub const CORE_IGNORE_LINE: &str = ".codebus/";

/// Repo-root skill bundle directories. Only appended when init was asked
/// to write the repo-root copies (`with_repo_root_skills: true`); when
/// init runs in the default vault-only mode, these lines are NOT added
/// to the source `.gitignore`.
pub const SKILL_BUNDLE_IGNORE_LINES: &[&str] = &[
    ".claude/skills/codebus-goal/",
    ".claude/skills/codebus-query/",
    ".claude/skills/codebus-fix/",
    ".claude/skills/codebus-chat/",
];

pub fn ensure_codebus_in_gitignore(
    repo_root: &Path,
    include_skill_bundles: bool,
) -> io::Result<GitignoreOutcome> {
    if !repo_root.join(".git").exists() {
        return Ok(GitignoreOutcome::NotAGitRepo);
    }

    let required: Vec<&'static str> = if include_skill_bundles {
        std::iter::once(CORE_IGNORE_LINE)
            .chain(SKILL_BUNDLE_IGNORE_LINES.iter().copied())
            .collect()
    } else {
        vec![CORE_IGNORE_LINE]
    };

    let gi_path = repo_root.join(".gitignore");
    let existing = match fs::read_to_string(&gi_path) {
        Ok(s) => Some(s),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(err),
    };

    match existing {
        None => {
            let mut body = String::new();
            for line in &required {
                body.push_str(line);
                body.push('\n');
            }
            fs::write(&gi_path, body)?;
            Ok(GitignoreOutcome::Created)
        }
        Some(body) => {
            let present: std::collections::HashSet<&str> = body.lines().collect();
            let missing: Vec<&str> = required
                .iter()
                .copied()
                .filter(|l| !present.contains(l))
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

    fn assert_core_line_present(body: &str) {
        assert!(
            body.lines().any(|l| l == CORE_IGNORE_LINE),
            ".gitignore missing core line `{CORE_IGNORE_LINE}`:\n{body}"
        );
    }

    fn assert_skill_bundle_lines_present(body: &str) {
        for line in SKILL_BUNDLE_IGNORE_LINES {
            assert!(
                body.lines().any(|l| l == *line),
                ".gitignore missing skill-bundle line `{line}`:\n{body}"
            );
        }
    }

    fn assert_skill_bundle_lines_absent(body: &str) {
        for line in SKILL_BUNDLE_IGNORE_LINES {
            assert!(
                !body.lines().any(|l| l == *line),
                ".gitignore must NOT contain skill-bundle line `{line}` in default mode:\n{body}"
            );
        }
    }

    #[test]
    fn creates_gitignore_with_core_line_only_in_default_mode() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let outcome = ensure_codebus_in_gitignore(tmp.path(), false).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Created);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_core_line_present(&body);
        assert_skill_bundle_lines_absent(&body);
    }

    #[test]
    fn creates_gitignore_with_all_lines_when_include_skill_bundles() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Created);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_core_line_present(&body);
        assert_skill_bundle_lines_present(&body);
    }

    #[test]
    fn appends_missing_required_lines_preserving_existing_entries() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(tmp.path().join(".gitignore"), "node_modules\n").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(body.starts_with("node_modules\n"));
        assert_core_line_present(&body);
        assert_skill_bundle_lines_present(&body);
    }

    #[test]
    fn idempotent_when_all_required_present() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        let mut initial = String::from("node_modules\n");
        initial.push_str(CORE_IGNORE_LINE);
        initial.push('\n');
        for line in SKILL_BUNDLE_IGNORE_LINES {
            initial.push_str(line);
            initial.push('\n');
        }
        initial.push_str("target/\n");
        fs::write(tmp.path().join(".gitignore"), &initial).unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::AlreadyPresent);
        assert_eq!(
            fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
            initial
        );
    }

    #[test]
    fn skips_when_not_a_git_repo() {
        let tmp = TempDir::new().unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::NotAGitRepo);
        assert!(!tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn appends_correctly_when_existing_lacks_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(tmp.path().join(".gitignore"), "node_modules").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(body.starts_with("node_modules\n"));
        assert_core_line_present(&body);
        assert_skill_bundle_lines_present(&body);
    }

    #[test]
    fn partial_appends_only_missing_required_lines() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        // Existing has .codebus/ but lacks the .claude/skills/codebus-* trio
        fs::write(tmp.path().join(".gitignore"), ".codebus/\n").unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), true).unwrap();
        assert_eq!(outcome, GitignoreOutcome::Appended);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_core_line_present(&body);
        assert_skill_bundle_lines_present(&body);
        // .codebus/ should not appear twice
        let count = body.lines().filter(|l| *l == ".codebus/").count();
        assert_eq!(count, 1);
    }

    /// Spec scenario: "Init does not add repo-root skill bundle gitignore
    /// patterns by default" — when an existing source repo `.gitignore`
    /// already has the core line but no skill-bundle lines, the default
    /// (no opt-in) re-init MUST NOT add them.
    #[test]
    fn default_mode_skips_skill_bundle_lines_even_when_other_lines_missing() {
        let tmp = TempDir::new().unwrap();
        make_git_repo(tmp.path());
        fs::write(
            tmp.path().join(".gitignore"),
            "node_modules\n.codebus/\n",
        )
        .unwrap();
        let outcome = ensure_codebus_in_gitignore(tmp.path(), false).unwrap();
        // Already-present case from the default-mode perspective: only the
        // core line is required, and it is present.
        assert_eq!(outcome, GitignoreOutcome::AlreadyPresent);
        let body = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert_skill_bundle_lines_absent(&body);
    }
}
