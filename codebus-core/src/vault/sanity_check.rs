//! Pre-flight sanity check for `codebus init`: refuse to initialize when
//! the target path is already a vault root or sits inside one. Avoids
//! creating `<repo>/.codebus/.codebus/` or other nested-vault accidents.

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("path appears to be (or sit inside) a codebus vault: {path:?} ({reason})")]
pub struct VaultRefusal {
    pub path: PathBuf,
    pub reason: String,
}

/// Refuse if `path` looks like an existing codebus vault root, identified
/// by the presence of a sibling `wiki/` directory and a `manifest.yaml`
/// file. Also refuse when `path` is literally named `.codebus` (covers the
/// "user typed `codebus init` from inside `.codebus/`" case before the
/// vault has accumulated its sentinels).
pub fn check_repo_is_not_vault(path: &Path) -> Result<(), VaultRefusal> {
    if path.file_name().and_then(|n| n.to_str()) == Some(".codebus") {
        return Err(VaultRefusal {
            path: path.to_path_buf(),
            reason: "directory is named `.codebus` — refusing to nest a vault inside another vault"
                .into(),
        });
    }
    let wiki = path.join("wiki");
    let manifest = path.join("manifest.yaml");
    if wiki.is_dir() && manifest.is_file() {
        return Err(VaultRefusal {
            path: path.to_path_buf(),
            reason:
                "directory contains `wiki/` and `manifest.yaml` — looks like a codebus vault root"
                    .into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn accepts_normal_repo_root() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("README.md"), "hi").unwrap();
        assert!(check_repo_is_not_vault(tmp.path()).is_ok());
    }

    #[test]
    fn refuses_directory_named_dot_codebus() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join(".codebus");
        fs::create_dir_all(&nested).unwrap();
        let err = check_repo_is_not_vault(&nested).unwrap_err();
        assert_eq!(err.path, nested);
        assert!(err.reason.contains(".codebus"));
    }

    #[test]
    fn refuses_directory_with_wiki_and_manifest_siblings() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        fs::write(tmp.path().join("manifest.yaml"), "codebus_version: x").unwrap();
        let err = check_repo_is_not_vault(tmp.path()).unwrap_err();
        assert!(err.reason.contains("vault"));
    }

    #[test]
    fn accepts_directory_with_only_wiki_subfolder() {
        // Some monorepos have a `wiki/` dir for documentation. Without
        // manifest.yaml present, that's not a codebus vault.
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        assert!(check_repo_is_not_vault(tmp.path()).is_ok());
    }
}
