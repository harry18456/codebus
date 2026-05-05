use std::path::{Path, PathBuf};

const VAULT_MARKERS: &[&str] = &["CLAUDE.md", "wiki", "raw", "goals.jsonl"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultSanityResult {
    pub ok: bool,
    pub reason: Option<String>,
    pub hint: Option<String>,
}

impl VaultSanityResult {
    pub fn ok() -> Self {
        Self {
            ok: true,
            reason: None,
            hint: None,
        }
    }

    pub fn deny(reason: String, hint: String) -> Self {
        Self {
            ok: false,
            reason: Some(reason),
            hint: Some(hint),
        }
    }
}

fn looks_like_vault(path: &Path) -> bool {
    VAULT_MARKERS.iter().all(|m| path.join(m).exists())
}

fn resolve_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .or_else(home_via_dirs)
}

#[cfg(not(test))]
fn home_via_dirs() -> Option<PathBuf> {
    // Fallback: avoid hard-depending on dirs crate; users normally have
    // HOME / USERPROFILE set on every supported platform.
    None
}

#[cfg(test)]
fn home_via_dirs() -> Option<PathBuf> {
    None
}

/// Catches user mistakes that would otherwise produce nested-vault chaos:
///   - `--repo` points at `.codebus/` (basename or by marker structure)
///   - `--repo` points INSIDE a vault somewhere up the tree
///   - `--repo` points at the user-global `~/.codebus/` config dir
///
/// Returns `ok = true` when the path appears to be a real source repo.
pub fn check_repo_is_not_vault(repo_root: impl AsRef<Path>) -> VaultSanityResult {
    let resolved = match repo_root.as_ref().canonicalize() {
        Ok(p) => p,
        // If canonicalize fails (path doesn't exist), fall back to the
        // raw path — sanity rules can still match by name even when the
        // dir doesn't exist (TS used resolve(), which doesn't require
        // existence).
        Err(_) => repo_root.as_ref().to_path_buf(),
    };

    // 1. user-global ~/.codebus/ config dir
    if let Some(home) = resolve_home() {
        let global = home.join(".codebus");
        if resolved == global || resolved == global.canonicalize().unwrap_or(global.clone()) {
            return VaultSanityResult::deny(
                format!("--repo points at the user-global codebus config dir ({}).", resolved.display()),
                "~/.codebus/ holds your config.yaml, not source code. Pass --repo /path/to/your/source/repo.".into(),
            );
        }
    }

    // 2. repoRoot is itself a vault (named .codebus OR has marker structure)
    let basename = resolved.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if basename == ".codebus" || looks_like_vault(&resolved) {
        let parent = resolved.parent().map(Path::to_path_buf).unwrap_or_default();
        return VaultSanityResult::deny(
            format!(
                "--repo points at a codebus vault ({}), not a source repo.",
                resolved.display()
            ),
            format!(
                "Vaults live AT the source repo's .codebus/ subdir. Pass --repo {} (the parent).",
                parent.display()
            ),
        );
    }

    // 3. repoRoot is INSIDE a vault somewhere up the tree
    let mut cur = resolved.parent().map(Path::to_path_buf);
    while let Some(c) = cur {
        if c.parent() == Some(&c) {
            break;
        }
        let name = c.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == ".codebus" && looks_like_vault(&c) {
            let parent = c.parent().map(Path::to_path_buf).unwrap_or_default();
            return VaultSanityResult::deny(
                format!(
                    "--repo ({}) is inside a codebus vault at {}.",
                    resolved.display(),
                    c.display()
                ),
                format!(
                    "Pass --repo {} (the source repo containing the vault).",
                    parent.display()
                ),
            );
        }
        cur = c.parent().map(Path::to_path_buf);
    }

    VaultSanityResult::ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("codebus-sanity-{name}-{}", std::process::id()));
        if p.exists() {
            let _ = fs::remove_dir_all(&p);
        }
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_vault(at: &Path) {
        fs::create_dir_all(at.join("wiki")).unwrap();
        fs::create_dir_all(at.join("raw")).unwrap();
        fs::write(at.join("CLAUDE.md"), "schema").unwrap();
        fs::write(at.join("goals.jsonl"), "").unwrap();
    }

    #[test]
    fn plain_source_repo_is_ok() {
        let r = tmp("plain");
        let res = check_repo_is_not_vault(&r);
        assert!(res.ok, "expected ok for plain repo, got {res:?}");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn dot_codebus_named_dir_is_rejected() {
        let r = tmp("dotcodebus");
        let nested = r.join(".codebus");
        fs::create_dir_all(&nested).unwrap();
        let res = check_repo_is_not_vault(&nested);
        assert!(!res.ok);
        assert!(res.reason.unwrap().contains(".codebus"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn marker_structure_directory_is_rejected_even_without_dot_codebus_name() {
        let r = tmp("markerlike");
        make_vault(&r);
        let res = check_repo_is_not_vault(&r);
        assert!(!res.ok, "expected vault detection by markers, got {res:?}");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn pointing_inside_a_vault_subdir_is_rejected() {
        let r = tmp("inside");
        let vault = r.join(".codebus");
        make_vault(&vault);
        let inside = vault.join("wiki").join("concepts");
        fs::create_dir_all(&inside).unwrap();
        let res = check_repo_is_not_vault(&inside);
        assert!(!res.ok);
        assert!(res.reason.unwrap().contains("inside a codebus vault"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn nonexistent_path_does_not_panic() {
        // resolve() in TS does not require existence; Rust canonicalize
        // does, so we fall back to raw path. The only check we still hit
        // for non-existent paths is the basename rule.
        let res = check_repo_is_not_vault("/definitely/not/a/real/path/.codebus");
        assert!(!res.ok);
    }
}
