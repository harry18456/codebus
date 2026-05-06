//! Cross-iteration "fake memory" for the lint-feedback-loop fix module.
//!
//! Each iteration of `lint_and_fix` ends with the agent having edited the
//! vault. The next iteration's prompt needs to remind the agent of what it
//! already did so it doesn't retry the same failed approach. We use the
//! vault's nested git repo as the ground-truth record: capture HEAD's sha
//! before the loop starts, then on each subsequent iteration shell out to
//! `git diff <snapshot_sha> -- wiki/` to get a structured summary.
//!
//! Why git diff over stream events: the agent's stream may report "I split
//! X into Y, Z" without actually doing it. Disk diff is what was really
//! written.

use std::path::Path;
use std::process::Command;

/// Hard cap on the diff summary length. Massive diffs (e.g. agent moved 50
/// pages) would otherwise blow out the prompt's token budget. The diff is
/// truncated mid-line; downstream consumers should treat it as advisory,
/// not authoritative.
pub const DIFF_SUMMARY_CAP_BYTES: usize = 30 * 1024;

/// Capture `git diff <base_sha> -- wiki/` against the vault's nested repo.
///
/// - `vault_root` points at `<repo>/.codebus/` (the nested git repo).
/// - `base_sha` is the HEAD captured before the fix loop's first iteration.
///
/// Returns `Ok(String)`:
/// - empty string when there are no changes against `base_sha`
/// - full diff output (UTF-8 lossy, capped at [`DIFF_SUMMARY_CAP_BYTES`])
///   otherwise
/// - empty string when git itself fails (e.g. `base_sha` doesn't exist
///   yet because the vault has never been committed). We fall through to
///   empty rather than propagating the error so the fix loop can still
///   make progress on a fresh vault.
///
/// The function pre-stages with `git add -A` so newly-created files (the
/// agent's Write tool produces untracked files) participate in the diff.
/// `git diff <commit>` against the working tree only reports tracked-file
/// deltas; without staging, brand-new pages would silently disappear from
/// the "previous attempt" memory. Staging is benign in the codebus
/// nested vault repo because the surrounding workflow ends in
/// `auto_commit` which would `git add -A` anyway.
pub fn git_diff_summary(vault_root: impl AsRef<Path>, base_sha: &str) -> std::io::Result<String> {
    let vault_root = vault_root.as_ref();
    // Best-effort stage: ignore errors so a partially-broken repo (e.g.
    // missing index) still falls through to the diff attempt.
    let _ = Command::new("git")
        .current_dir(vault_root)
        .args(["add", "-A", "--", "wiki/"])
        .output();
    let out = Command::new("git")
        .current_dir(vault_root)
        .args(["diff", base_sha, "--", "wiki/"])
        .output()?;
    if !out.status.success() {
        return Ok(String::new());
    }
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    if s.len() > DIFF_SUMMARY_CAP_BYTES {
        s.truncate(DIFF_SUMMARY_CAP_BYTES);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::nested_repo::{auto_commit, init_nested_repo};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn nanos() -> u32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    }

    fn tmp_vault(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codebus-fixmem-{name}-{}-{}",
            std::process::id(),
            nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("wiki/concepts")).unwrap();
        dir
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p);
    }

    fn head_sha(vault: &Path) -> String {
        let out = Command::new("git")
            .current_dir(vault)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn clean_vault_returns_empty_diff_against_head() {
        let v = tmp_vault("clean");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("wiki/concepts/foo.md"), "# foo\n").unwrap();
        auto_commit(&v, "seed").unwrap();
        let sha = head_sha(&v);
        let diff = git_diff_summary(&v, &sha).unwrap();
        assert!(diff.is_empty(), "expected empty, got: {diff:?}");
        cleanup(&v);
    }

    #[test]
    fn modified_vault_returns_non_empty_diff_against_base() {
        let v = tmp_vault("modified");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("wiki/concepts/foo.md"), "before\n").unwrap();
        auto_commit(&v, "seed").unwrap();
        let base = head_sha(&v);
        // mutate post-commit; do NOT commit again — diff is against working tree
        fs::write(v.join("wiki/concepts/foo.md"), "after\n").unwrap();
        let diff = git_diff_summary(&v, &base).unwrap();
        assert!(!diff.is_empty(), "expected non-empty diff");
        // diff format includes the file path and the +/- markers
        assert!(diff.contains("concepts/foo.md"));
        assert!(diff.contains("-before"));
        assert!(diff.contains("+after"));
        cleanup(&v);
    }

    #[test]
    fn nonexistent_base_sha_falls_through_to_empty_without_panic() {
        let v = tmp_vault("badbase");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("wiki/concepts/foo.md"), "x").unwrap();
        auto_commit(&v, "seed").unwrap();
        let bogus = "0000000000000000000000000000000000000000";
        let diff = git_diff_summary(&v, bogus).unwrap();
        assert!(diff.is_empty(), "bogus sha must not panic, got: {diff:?}");
        cleanup(&v);
    }

    #[test]
    fn massive_diff_is_truncated_to_cap() {
        let v = tmp_vault("bigdiff");
        init_nested_repo(&v).unwrap();
        fs::write(v.join("wiki/concepts/big.md"), "seed\n").unwrap();
        auto_commit(&v, "seed").unwrap();
        let base = head_sha(&v);
        // Write a pre-cap-sized addition. With surrounding diff metadata,
        // the resulting `git diff` output exceeds the cap.
        let big = "x".repeat(DIFF_SUMMARY_CAP_BYTES + 1024);
        fs::write(v.join("wiki/concepts/big.md"), &big).unwrap();
        let diff = git_diff_summary(&v, &base).unwrap();
        assert!(
            diff.len() <= DIFF_SUMMARY_CAP_BYTES,
            "diff length {} exceeds cap {}",
            diff.len(),
            DIFF_SUMMARY_CAP_BYTES
        );
        cleanup(&v);
    }
}
