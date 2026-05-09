//! Vault root auto-detection for the lint subsystem.
//!
//! Per v3-lint spec `Vault Root Auto-Detection` requirement, lint resolves
//! the vault root using this precedence on each invocation:
//!
//! 1. Explicit `--repo <PATH>` → use `<PATH>/.codebus/` as vault root.
//! 2. `<cwd>/wiki/` exists → cwd IS vault root (agent-from-vault scenario).
//! 3. `<cwd>/.codebus/wiki/` exists → cwd is source repo root.
//! 4. None of the above → return error so CLI can exit 2 with hint.
//!
//! `init`/`goal`/`query` do NOT use this — they always treat input as
//! source repo root and append `.codebus`. Detection is lint-specific
//! because lint needs to work from both CLI cwd and agent cwd (which is
//! already inside `.codebus/`).

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LocateError {
    /// Neither `<cwd>/wiki/` nor `<cwd>/.codebus/wiki/` exists, and no `--repo`
    /// flag was provided. CLI surfaces this as exit 2 with a hint to run
    /// `codebus init` first.
    NoVaultFound,
}

impl std::fmt::Display for LocateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocateError::NoVaultFound => write!(
                f,
                "no codebus vault found at cwd or under .codebus/ — run `codebus init` first"
            ),
        }
    }
}

impl std::error::Error for LocateError {}

/// Resolve the vault root path (the `.codebus/` directory).
///
/// - If `repo_override` is `Some(path)`, returns `<path>/.codebus/` regardless
///   of cwd contents (the explicit `--repo` flag wins).
/// - Otherwise, inspects `cwd`:
///   - If `<cwd>/wiki/` exists → returns `cwd` itself (agent-from-vault case).
///   - Else if `<cwd>/.codebus/wiki/` exists → returns `<cwd>/.codebus`.
///   - Else returns `Err(LocateError::NoVaultFound)`.
pub fn locate_vault_root(
    cwd: impl AsRef<Path>,
    repo_override: Option<&Path>,
) -> Result<PathBuf, LocateError> {
    if let Some(repo) = repo_override {
        return Ok(repo.join(".codebus"));
    }

    let cwd = cwd.as_ref();

    if cwd.join("wiki").is_dir() {
        return Ok(cwd.to_path_buf());
    }

    if cwd.join(".codebus").join("wiki").is_dir() {
        return Ok(cwd.join(".codebus"));
    }

    Err(LocateError::NoVaultFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_vault_when_cwd_has_wiki_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        let resolved = locate_vault_root(tmp.path(), None).unwrap();
        assert_eq!(resolved, tmp.path());
    }

    #[test]
    fn detects_vault_when_cwd_has_dot_codebus_wiki() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".codebus").join("wiki")).unwrap();
        let resolved = locate_vault_root(tmp.path(), None).unwrap();
        assert_eq!(resolved, tmp.path().join(".codebus"));
    }

    #[test]
    fn errors_when_no_vault_locatable() {
        let tmp = tempfile::tempdir().unwrap();
        let result = locate_vault_root(tmp.path(), None);
        assert!(matches!(result, Err(LocateError::NoVaultFound)));
    }

    #[test]
    fn explicit_repo_override_wins_over_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        let other = tempfile::tempdir().unwrap();
        let resolved = locate_vault_root(tmp.path(), Some(other.path())).unwrap();
        assert_eq!(resolved, other.path().join(".codebus"));
    }

    #[test]
    fn explicit_repo_override_does_not_check_existence() {
        let nonexistent = Path::new("/this/path/definitely/does/not/exist");
        let resolved = locate_vault_root(".", Some(nonexistent)).unwrap();
        assert_eq!(resolved, nonexistent.join(".codebus"));
    }

    #[test]
    fn cwd_with_only_dot_codebus_no_wiki_subdir_does_not_match() {
        // `.codebus/` exists but no `wiki/` inside — this is an in-progress
        // init or corrupted vault; locate should error out, not return a
        // half-vault path.
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".codebus")).unwrap();
        // Note: no wiki/ subdir
        let result = locate_vault_root(tmp.path(), None);
        assert!(matches!(result, Err(LocateError::NoVaultFound)));
    }
}
