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
    let diff = capture_git(vault_root, &["diff", "--name-only", base, "--", subdir])?;
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
}
