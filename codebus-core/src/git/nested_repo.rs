//! Nested git repository operations on the `.codebus/` vault.
//!
//! Two public operations: [`init_nested_repo`] (idempotent `git init -b main`
//! plus local user config decoupled from the user's global gitconfig) and
//! [`auto_commit`] (`git add -A` + `git commit -m`; no-op when working tree
//! clean). Both shell out to the `git` binary on PATH; failures propagate as
//! `io::Error` so callers (init.rs and later goal/fix verbs) can surface them
//! and exit non-zero rather than continue against an inconsistent vault state.

use std::io;
use std::path::Path;
use std::process::Command;

/// Initialize a nested git repo at `vault_root` if absent. Configures the
/// nested repo's local `user.email=codebus@local` / `user.name=codebus` so
/// codebus's auto-commits don't depend on the user's global git config and
/// the commit author trail clearly identifies machine-generated commits.
///
/// Idempotent: when `vault_root/.git` already exists this function is a
/// no-op and SHALL NOT modify the existing local config (preserves any
/// user-applied overrides like `git config user.email alice@example.com`).
pub fn init_nested_repo(vault_root: impl AsRef<Path>) -> io::Result<()> {
    let vault_root = vault_root.as_ref();
    if vault_root.join(".git").exists() {
        return Ok(());
    }
    run_git(vault_root, &["init", "-b", "main", "-q"])?;
    run_git(vault_root, &["config", "user.email", "codebus@local"])?;
    run_git(vault_root, &["config", "user.name", "codebus"])?;
    Ok(())
}

/// Stage everything (`git add -A`), then commit with `message` if the
/// working tree has any change. Returns the resulting `HEAD` sha (or the
/// existing HEAD when there was nothing to commit, which keeps callers
/// agnostic to the clean/dirty branch).
pub fn auto_commit(vault_root: impl AsRef<Path>, message: &str) -> io::Result<String> {
    let vault_root = vault_root.as_ref();
    run_git(vault_root, &["add", "-A"])?;

    let status = capture_git(vault_root, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        let head = capture_git(vault_root, &["rev-parse", "HEAD"])?;
        return Ok(head.trim().to_string());
    }

    run_git(vault_root, &["commit", "-m", message, "-q"])?;
    let head = capture_git(vault_root, &["rev-parse", "HEAD"])?;
    Ok(head.trim().to_string())
}

/// Current `HEAD` sha of the nested vault repo, or `None` when it
/// cannot be resolved (no commits yet / not a repo). Used by the
/// goal content-verify stage to pin a pre-run revision before the goal
/// agent spawn (goal-content-verify design D3).
pub fn rev_parse_head(vault_root: impl AsRef<Path>) -> Option<String> {
    capture_git(vault_root.as_ref(), &["rev-parse", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Repo-relative paths under `subdir` that this run created or modified
/// since `base_rev` (goal-content-verify design D3). Combines tracked
/// modifications (`git diff --name-only <base_rev> -- <subdir>`) with
/// brand-new untracked files (`git ls-files --others --exclude-standard
/// -- <subdir>`) so a freshly-written, not-yet-committed wiki page is
/// detected as "changed" (the design's intent is *added or modified*
/// pages; `git diff` alone misses untracked additions). When `base_rev`
/// is `None` the diff is taken against `HEAD`. Returns a de-duplicated,
/// sorted list; an empty list means nothing under `subdir` changed.
/// Any git failure surfaces as `io::Error` (caller treats the whole
/// content-verify stage as best-effort / non-fatal).
pub fn changed_paths_under(
    vault_root: impl AsRef<Path>,
    base_rev: Option<&str>,
    subdir: &str,
) -> io::Result<Vec<String>> {
    let vault_root = vault_root.as_ref();
    let base = base_rev.unwrap_or("HEAD");
    // core-quality-residuals (F3): `--diff-filter=ACMR` whitelists Added /
    // Copied / Modified / Renamed. Without this filter `git diff` defaults
    // include Deleted (D) entries, which then leak to `goal` content-verify
    // and make the verify spawn try to `Read` files that no longer exist.
    // Whitelist over `d` blacklist because future git filter types (T/U/X/B)
    // are equally unwanted for content-verify — explicit ACMR matches the
    // verb-library §Goal Content Verify spec wording "created or modified".
    let diff = capture_git(
        vault_root,
        &["diff", "--name-only", "--diff-filter=ACMR", base, "--", subdir],
    )?;
    let others = capture_git(
        vault_root,
        &["ls-files", "--others", "--exclude-standard", "--", subdir],
    )?;
    let mut paths: Vec<String> = diff
        .lines()
        .chain(others.lines())
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn run_git(cwd: &Path, args: &[&str]) -> io::Result<()> {
    let out = Command::new("git").current_dir(cwd).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}

fn capture_git(cwd: &Path, args: &[&str]) -> io::Result<String> {
    let out = Command::new("git").current_dir(cwd).args(args).output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn init_creates_dot_git_with_codebus_identity() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        assert!(v.path().join(".git").exists());
        let email = capture_git(v.path(), &["config", "--get", "user.email"]).unwrap();
        assert_eq!(email.trim(), "codebus@local");
        let name = capture_git(v.path(), &["config", "--get", "user.name"]).unwrap();
        assert_eq!(name.trim(), "codebus");
    }

    #[test]
    fn init_is_idempotent() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        init_nested_repo(v.path()).expect("second init should be a no-op");
    }

    #[test]
    fn init_does_not_overwrite_existing_user_config() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        // Simulate user override of nested repo identity.
        run_git(v.path(), &["config", "user.email", "alice@example.com"]).unwrap();
        // Re-init should be a no-op and preserve override.
        init_nested_repo(v.path()).unwrap();
        let email = capture_git(v.path(), &["config", "--get", "user.email"]).unwrap();
        assert_eq!(email.trim(), "alice@example.com");
    }

    #[test]
    fn auto_commit_writes_changes() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        fs::write(v.path().join("a.md"), "x").unwrap();
        let sha = auto_commit(v.path(), "first").unwrap();
        assert_eq!(sha.len(), 40, "expected 40-char sha, got `{sha}`");
        let st = capture_git(v.path(), &["status", "--porcelain"]).unwrap();
        assert!(st.trim().is_empty(), "working tree dirty after commit");
    }

    #[test]
    fn auto_commit_returns_existing_head_when_clean() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        fs::write(v.path().join("a.md"), "x").unwrap();
        let sha1 = auto_commit(v.path(), "first").unwrap();
        let sha2 = auto_commit(v.path(), "second-no-op").unwrap();
        assert_eq!(sha1, sha2, "clean working tree must return existing HEAD");
    }

    #[test]
    fn auto_commit_message_appears_in_log() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        fs::write(v.path().join("a.md"), "x").unwrap();
        auto_commit(v.path(), "wiki: explore X").unwrap();
        let log = capture_git(v.path(), &["log", "--pretty=format:%s", "-1"]).unwrap();
        assert_eq!(log.trim(), "wiki: explore X");
    }

    // --- core-quality-residuals (F3): `changed_paths_under` test module.
    // The function had ZERO test coverage before this change. We backfill
    // the expected behavior under the verb-library §Goal Content Verify
    // spec wording: "diffing the vault git repository ... created or
    // modified pages". Specifically:
    //   - Added / Modified / Renamed paths SHALL be returned.
    //   - **Deleted** paths SHALL be excluded (the F3 bug — `git diff`
    //     defaults include them; without `--diff-filter=ACMR` they leak
    //     to the verify spawn which then fails to Read them).
    //   - Untracked new files SHALL be included (the function already
    //     `ls-files --others` for this — regression guard).
    //   - Empty diff SHALL return an empty list.
    //   - Subdir filter SHALL restrict scope (no leakage from outside).

    /// Helper: init a nested repo, write a `wiki/<name>.md` file, commit
    /// it, return the resulting HEAD sha so callers can pass it as the
    /// `base_rev` for subsequent `changed_paths_under` calls.
    fn seed_wiki_commit(vault_root: &Path, name: &str, body: &str) -> String {
        fs::create_dir_all(vault_root.join("wiki")).unwrap();
        fs::write(vault_root.join("wiki").join(name), body).unwrap();
        auto_commit(vault_root, &format!("seed: {name}")).unwrap()
    }

    #[test]
    fn changed_paths_under_includes_added_files() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        // Add a new committed file after `base`.
        fs::write(v.path().join("wiki/beta.md"), "beta").unwrap();
        auto_commit(v.path(), "add beta").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            changed.contains(&"wiki/beta.md".to_string()),
            "added file SHALL appear in changed list; got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_includes_modified_files() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "original");
        fs::write(v.path().join("wiki/alpha.md"), "modified").unwrap();
        auto_commit(v.path(), "modify alpha").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            changed.contains(&"wiki/alpha.md".to_string()),
            "modified file SHALL appear in changed list; got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_includes_renamed_files() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        // Use `git mv` so git records the change as a rename (with default
        // similarity detection the file is small enough to detect).
        run_git(
            v.path(),
            &["mv", "wiki/alpha.md", "wiki/alpha-renamed.md"],
        )
        .unwrap();
        auto_commit(v.path(), "rename alpha").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        // The new path SHALL appear; under the ACMR filter the rename's
        // new path is what `git diff --name-only` reports for R.
        assert!(
            changed.contains(&"wiki/alpha-renamed.md".to_string()),
            "renamed file (new path) SHALL appear in changed list; got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_excludes_deleted_files() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        // Delete the file AND commit the deletion so it appears in `git
        // diff <base> HEAD` as a D entry.
        fs::remove_file(v.path().join("wiki/alpha.md")).unwrap();
        auto_commit(v.path(), "delete alpha").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            !changed.contains(&"wiki/alpha.md".to_string()),
            "deleted file SHALL NOT appear in changed list (F3 fix); got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_includes_untracked_new_files() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        // Write a new file but do NOT commit — should appear via the
        // `ls-files --others` channel (untracked).
        fs::write(v.path().join("wiki/draft.md"), "draft").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            changed.contains(&"wiki/draft.md".to_string()),
            "untracked file SHALL appear in changed list; got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_returns_empty_for_no_changes() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            changed.is_empty(),
            "no changes since `base` SHALL return empty list; got: {changed:?}"
        );
    }

    #[test]
    fn changed_paths_under_restricts_to_subdir() {
        let v = TempDir::new().unwrap();
        init_nested_repo(v.path()).unwrap();
        let base = seed_wiki_commit(v.path(), "alpha.md", "alpha");
        // Change a file OUTSIDE the subdir; the call SHALL NOT see it.
        fs::write(v.path().join("other.md"), "x").unwrap();
        auto_commit(v.path(), "outside wiki").unwrap();
        let changed = changed_paths_under(v.path(), Some(&base), "wiki/").unwrap();
        assert!(
            !changed.iter().any(|p| p == "other.md"),
            "subdir filter SHALL exclude paths outside wiki/; got: {changed:?}"
        );
    }
}
