//! Resolve the cross-OS path to Obsidian's vault registry file `obsidian.json`.
//!
//! # "None means not installed"
//!
//! Both public functions return `Option<PathBuf>` and use `None` as the signal
//! that **Obsidian is not installed on this machine**. Callers (e.g. the
//! registry module) should silently skip vault registration in that case
//! rather than treating it as an error — codebus can produce a valid wiki
//! whether or not the user has Obsidian.
//!
//! Concretely, `None` happens when either:
//! - `dirs::config_dir()` itself returns `None` (unsupported platform / no
//!   `HOME` env var / etc.), or
//! - the resolved `<config>/obsidian/` directory does not exist on disk.
//!
//! The `obsidian.json` file *inside* that directory is **not** existence-
//! checked here: the registry creates a fresh `{"vaults":{}}` if it's
//! missing. Only the parent directory matters for "is Obsidian installed".
//!
//! # Path layout per OS (via `dirs::config_dir()`)
//!
//! | OS      | base                                  | resolved                                |
//! |---------|---------------------------------------|-----------------------------------------|
//! | Windows | `%APPDATA%` (Roaming)                 | `%APPDATA%\obsidian\`                   |
//! | macOS   | `~/Library/Application Support`       | `~/Library/Application Support/obsidian/` |
//! | Linux   | `$XDG_CONFIG_HOME` or `~/.config`     | `~/.config/obsidian/`                   |
//!
//! # Trust assumption
//!
//! We trust whatever `dirs::config_dir()` returns. An attacker who controls
//! `$XDG_CONFIG_HOME` / `%APPDATA%` could redirect this lookup, but that is
//! the same trust level any user-config-aware tool (git, gh, etc.) operates
//! at, and codebus only writes a JSON file containing a vault path entry —
//! never executable content. Hardening this further would require a
//! codebus-controlled config root, which defeats the purpose of integrating
//! with an existing Obsidian installation.

use std::path::PathBuf;

/// Cross-OS resolution of `<config>/obsidian/`.
///
/// Returns `Some(path)` only when both:
/// - `dirs::config_dir()` returned a base directory, AND
/// - `<base>/obsidian/` exists *as a directory* (not a regular file).
///
/// Returns `None` otherwise — interpreted by callers as "Obsidian is not
/// installed". The strict `is_dir()` check (rather than `exists()`) guards
/// against a corrupted or sabotaged setup where a regular file sits at that
/// path: subsequent `read_dir`-style calls would fail anyway, so we surface
/// the failure as "not installed" at the boundary.
pub fn resolve_obsidian_config_dir() -> Option<PathBuf> {
    resolve_from_base(dirs::config_dir())
}

/// Cross-OS resolution of `<config>/obsidian/obsidian.json`.
///
/// Returns `Some(path)` whenever [`resolve_obsidian_config_dir`] does. The
/// `obsidian.json` file itself is **not** existence-checked here — the
/// registry module is responsible for writing a fresh `{"vaults":{}}` when
/// missing. Only the parent directory's existence determines "is Obsidian
/// installed".
///
/// Returns `None` if [`resolve_obsidian_config_dir`] returns `None`.
pub fn obsidian_json_path() -> Option<PathBuf> {
    resolve_obsidian_config_dir().map(|dir| dir.join("obsidian.json"))
}

/// Pure helper for testability — separates the OS-environment lookup
/// (`dirs::config_dir()`) from the path-join + existence-check logic.
///
/// The thin public wrapper above is just `dirs::config_dir()` plumbing;
/// integration / smoke tests in later tasks cover the wiring. Unit tests
/// here drive `resolve_from_base` directly with controlled tempdir inputs.
fn resolve_from_base(base: Option<PathBuf>) -> Option<PathBuf> {
    base.map(|p| p.join("obsidian"))
        .filter(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Allocate a uniquely-named tempdir under `std::env::temp_dir()`.
    /// Aligns with the project convention (no `tempfile` crate dep).
    fn unique_tempdir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "codebus-obsidian-config-path-{tag}-{nanos}-{pid}",
            pid = std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn resolve_returns_some_when_config_dir_exists() {
        let base = unique_tempdir("ok");
        // Simulate Obsidian being installed: create `<base>/obsidian/`.
        let obsidian_dir = base.join("obsidian");
        fs::create_dir_all(&obsidian_dir).expect("create obsidian dir");

        let resolved = resolve_from_base(Some(base.clone()));
        assert_eq!(resolved.as_deref(), Some(obsidian_dir.as_path()));

        // Cleanup best-effort.
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_returns_none_when_dir_missing() {
        let base = unique_tempdir("missing");
        // Intentionally do NOT create `<base>/obsidian/`.
        assert!(!base.join("obsidian").exists(), "precondition: dir absent");

        let resolved = resolve_from_base(Some(base.clone()));
        assert!(resolved.is_none(), "missing dir must yield None");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_returns_none_when_path_is_a_file_not_dir() {
        // Lazy-Developer audit lens: a regular file at the path should NOT
        // be treated as "installed". `is_dir()` (not `exists()`) guards this.
        let base = unique_tempdir("file");
        let path = base.join("obsidian");
        fs::write(&path, b"not a dir").expect("write decoy file");

        let resolved = resolve_from_base(Some(base.clone()));
        assert!(resolved.is_none(), "regular file must not be treated as installed");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn resolve_returns_none_when_base_is_none() {
        // Models `dirs::config_dir()` returning None on an unsupported platform.
        assert!(resolve_from_base(None).is_none());
    }

    #[test]
    fn obsidian_json_path_appends_filename() {
        // Drive the public wrapper indirectly: build the joined path the same
        // way `obsidian_json_path` does, then assert structure.
        let base = unique_tempdir("json");
        let obsidian_dir = base.join("obsidian");
        fs::create_dir_all(&obsidian_dir).expect("create obsidian dir");

        let resolved_dir = resolve_from_base(Some(base.clone())).expect("resolved");
        let json = resolved_dir.join("obsidian.json");

        // Cross-platform component check: last two components are
        // ["obsidian", "obsidian.json"].
        let comps: Vec<_> = json
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert!(comps.len() >= 2, "path must have >= 2 components");
        assert_eq!(comps[comps.len() - 1], "obsidian.json");
        assert_eq!(comps[comps.len() - 2], "obsidian");

        // And: we deliberately do NOT existence-check obsidian.json.
        assert!(!Path::new(&json).exists(), "json should not be auto-created");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn obsidian_json_path_propagates_none() {
        // When the underlying resolve yields None, the json variant must too.
        // We can't override `dirs::config_dir()`, so model the contract
        // through the pure helper composition the wrapper uses.
        let base_none: Option<PathBuf> = None;
        let resolved = resolve_from_base(base_none);
        let json = resolved.map(|d| d.join("obsidian.json"));
        assert!(json.is_none());
    }
}
