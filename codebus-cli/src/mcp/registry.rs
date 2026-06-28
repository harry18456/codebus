//! Vault resolution for the multi-vault MCP server.
//!
//! The server runs in one of two [`ServeMode`]s:
//! - **Pinned** (`codebus mcp --vault <path>`): one vault fixed at startup.
//! - **Registry** (`codebus mcp`): vaults are read — READ-ONLY — from the app
//!   state registry (`~/.codebus/app-state.json`) on every call, so a vault
//!   added in the GUI becomes visible without restarting the server.
//!
//! A tool's `vault` argument is resolved here against the registry whitelist:
//! a supplied path is accepted only when it canonicalizes to a registered,
//! present (on-disk) vault — anything else (e.g. `~/.ssh`) is rejected. The
//! two resolvers differ by tool nature: `wiki_list` / `wiki_search` are
//! exploratory and aggregate across all present vaults when `vault` is
//! omitted ([`resolve_for_query`]); `wiki_read` locates one page and requires
//! an unambiguous vault ([`resolve_for_read`]).
//!
//! This module composes `codebus_core::app_state` and never writes the
//! registry; the `tools.rs` query logic is untouched.

use std::path::{Path, PathBuf};

use codebus_core::app_state::{app_state_path, read_app_state};

/// How the running server selects which vault(s) to serve.
#[derive(Clone, Debug)]
pub enum ServeMode {
    /// `--vault <path>`: one vault pinned at startup; `wiki_root` was validated
    /// to exist before serving.
    Pinned {
        vault: PathBuf,
        name: String,
        wiki_root: PathBuf,
    },
    /// No `--vault`: resolve from the app-state registry per call.
    Registry,
}

/// A vault resolved to its on-disk wiki root, carrying its registry identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedVault {
    /// Stable identifier = the normalized absolute path (what `vault_list`
    /// returns and what a caller passes back as `vault`).
    pub vault: String,
    /// Display name (registry `display_name`, or the pinned vault's dir name).
    pub name: String,
    /// `<vault>/.codebus/wiki`.
    pub wiki_root: PathBuf,
}

/// Why a `vault` argument could not be resolved. Each maps to an MCP
/// `invalid_params` error via [`ResolveError::message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveError {
    /// Pinned mode, a supplied `vault` differs from the pinned vault (P1).
    PinnedMismatch,
    /// Registry mode, a supplied `vault` is not a present registry member.
    NotInRegistry,
    /// Registry mode, `vault` omitted with more than one present vault, for a
    /// tool (`wiki_read`) that needs exactly one.
    SpecifyVault,
    /// Registry mode, no present vault is registered.
    NoVaultRegistered,
}

impl ResolveError {
    pub fn message(self) -> String {
        match self {
            ResolveError::PinnedMismatch => {
                "this server is pinned to one vault via --vault; the supplied `vault` does not \
                 match it. Omit `vault` to use the pinned vault."
                    .to_string()
            }
            ResolveError::NotInRegistry => {
                "`vault` is not a registered codebus vault. Call vault_list to see the available \
                 vaults and pass one of their `vault` paths."
                    .to_string()
            }
            ResolveError::SpecifyVault => {
                "more than one vault is registered; specify which one via the `vault` argument \
                 (call vault_list first)."
                    .to_string()
            }
            ResolveError::NoVaultRegistered => {
                "no codebus vault is registered. Open a vault in the codebus app, or start the \
                 server with --vault <path>."
                    .to_string()
            }
        }
    }
}

/// `vault_list` entries: `(vault-id, display-name)` for every served vault.
/// Pinned mode returns the one pinned vault; registry mode returns every
/// present registered vault.
pub fn list_entries(mode: &ServeMode) -> Vec<(String, String)> {
    match mode {
        ServeMode::Pinned { vault, name, .. } => {
            vec![(path_id(vault), name.clone())]
        }
        ServeMode::Registry => present_registry_vaults()
            .into_iter()
            .map(|rv| (rv.vault, rv.name))
            .collect(),
    }
}

/// Resolve the vault(s) for an exploratory tool (`wiki_list` / `wiki_search`).
/// On omission in registry mode with multiple present vaults, this returns
/// ALL present vaults (the caller aggregates and tags each result).
pub fn resolve_for_query(
    mode: &ServeMode,
    arg: Option<&str>,
) -> Result<Vec<ResolvedVault>, ResolveError> {
    match mode {
        ServeMode::Pinned { .. } => Ok(vec![resolve_pinned(mode, arg)?]),
        ServeMode::Registry => resolve_query_from(present_registry_vaults(), arg),
    }
}

/// Resolve exactly one vault for `wiki_read`. On omission in registry mode
/// with multiple present vaults this is an error (read needs an unambiguous
/// vault); the caller normally passes the `vault` carried on a prior
/// `wiki_list` / `wiki_search` result.
pub fn resolve_for_read(mode: &ServeMode, arg: Option<&str>) -> Result<ResolvedVault, ResolveError> {
    match mode {
        ServeMode::Pinned { .. } => resolve_pinned(mode, arg),
        ServeMode::Registry => resolve_read_from(present_registry_vaults(), arg),
    }
}

/// Whether the server tags each result with its source `vault` / `name`.
/// Registry mode does (so the caller can address `wiki_read`); pinned mode
/// keeps the v1 single-vault result shape.
pub fn tags_source(mode: &ServeMode) -> bool {
    matches!(mode, ServeMode::Registry)
}

fn resolve_pinned(mode: &ServeMode, arg: Option<&str>) -> Result<ResolvedVault, ResolveError> {
    let ServeMode::Pinned {
        vault,
        name,
        wiki_root,
    } = mode
    else {
        unreachable!("resolve_pinned called on registry mode");
    };
    if let Some(a) = arg
        && !same_path(Path::new(a), vault)
    {
        return Err(ResolveError::PinnedMismatch);
    }
    Ok(ResolvedVault {
        vault: path_id(vault),
        name: name.clone(),
        wiki_root: wiki_root.clone(),
    })
}

/// Pure resolution for exploratory tools given a present-vault list.
fn resolve_query_from(
    present: Vec<ResolvedVault>,
    arg: Option<&str>,
) -> Result<Vec<ResolvedVault>, ResolveError> {
    if present.is_empty() {
        return Err(ResolveError::NoVaultRegistered);
    }
    match arg {
        Some(a) => match_one(present, a).map(|rv| vec![rv]),
        // Omitted: one present → that one; many → aggregate across all.
        None => Ok(present),
    }
}

/// Pure resolution for `wiki_read` given a present-vault list.
fn resolve_read_from(
    present: Vec<ResolvedVault>,
    arg: Option<&str>,
) -> Result<ResolvedVault, ResolveError> {
    if present.is_empty() {
        return Err(ResolveError::NoVaultRegistered);
    }
    match arg {
        Some(a) => match_one(present, a),
        None => {
            if present.len() == 1 {
                Ok(present.into_iter().next().expect("len checked == 1"))
            } else {
                Err(ResolveError::SpecifyVault)
            }
        }
    }
}

/// Find the present vault whose path matches `arg` (canonicalized both sides),
/// or [`ResolveError::NotInRegistry`].
fn match_one(present: Vec<ResolvedVault>, arg: &str) -> Result<ResolvedVault, ResolveError> {
    present
        .into_iter()
        .find(|rv| same_path(Path::new(arg), Path::new(&rv.vault)))
        .ok_or(ResolveError::NotInRegistry)
}

/// Present (= on-disk) vaults from the registry, read-only. A registered
/// vault whose directory no longer exists (`is_missing`) is skipped.
fn present_registry_vaults() -> Vec<ResolvedVault> {
    let Some(path) = app_state_path() else {
        return Vec::new();
    };
    present_vaults_at(&path)
}

fn present_vaults_at(state_path: &Path) -> Vec<ResolvedVault> {
    read_app_state(state_path)
        .vault_list
        .into_iter()
        .filter_map(|e| {
            let vault_path = Path::new(&e.path);
            if !vault_path.is_dir() {
                return None; // is_missing → skip
            }
            let wiki_root = vault_path.join(".codebus").join("wiki");
            Some(ResolvedVault {
                vault: e.path,
                name: e.display_name,
                wiki_root,
            })
        })
        .collect()
}

/// Compare two paths for identity by canonicalizing both. Canonicalization
/// requires the path to exist, which is true for present registry vaults and
/// for any real vault a caller copied from `vault_list`; a non-existent or
/// out-of-registry path fails to match (rejected as not-in-registry).
fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// The stable string id for a vault path (its display form).
fn path_id(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::app_state::{AppState, CURRENT_SCHEMA_VERSION, StoredVaultEntry, save_app_state};
    use tempfile::TempDir;

    fn vault_dir(parent: &TempDir, name: &str) -> PathBuf {
        let p = parent.path().join(name);
        std::fs::create_dir_all(p.join(".codebus").join("wiki")).unwrap();
        p
    }

    fn resolved(p: &Path, name: &str) -> ResolvedVault {
        ResolvedVault {
            vault: p.display().to_string(),
            name: name.into(),
            wiki_root: p.join(".codebus").join("wiki"),
        }
    }

    #[test]
    fn query_omit_single_returns_that_one() {
        let tmp = TempDir::new().unwrap();
        let a = vault_dir(&tmp, "a");
        let present = vec![resolved(&a, "a")];
        let got = resolve_query_from(present.clone(), None).unwrap();
        assert_eq!(got, present);
    }

    #[test]
    fn query_omit_many_aggregates_all() {
        let tmp = TempDir::new().unwrap();
        let a = vault_dir(&tmp, "a");
        let b = vault_dir(&tmp, "b");
        let present = vec![resolved(&a, "a"), resolved(&b, "b")];
        let got = resolve_query_from(present.clone(), None).unwrap();
        assert_eq!(got.len(), 2, "omit + multi must aggregate across all present");
        assert_eq!(got, present);
    }

    #[test]
    fn query_supplied_in_registry_limits_to_one() {
        let tmp = TempDir::new().unwrap();
        let a = vault_dir(&tmp, "a");
        let b = vault_dir(&tmp, "b");
        let present = vec![resolved(&a, "a"), resolved(&b, "b")];
        let got = resolve_query_from(present, Some(&b.display().to_string())).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "b");
    }

    #[test]
    fn supplied_outside_registry_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let a = vault_dir(&tmp, "a");
        let outside = TempDir::new().unwrap(); // a real dir, but not registered
        let present = vec![resolved(&a, "a")];
        let err = resolve_query_from(present.clone(), Some(&outside.path().display().to_string()))
            .unwrap_err();
        assert_eq!(err, ResolveError::NotInRegistry);
        // read path rejects it too.
        let err = resolve_read_from(present, Some(&outside.path().display().to_string()))
            .unwrap_err();
        assert_eq!(err, ResolveError::NotInRegistry);
    }

    #[test]
    fn read_omit_multi_requires_explicit_vault() {
        let tmp = TempDir::new().unwrap();
        let present = vec![resolved(&vault_dir(&tmp, "a"), "a"), resolved(&vault_dir(&tmp, "b"), "b")];
        let err = resolve_read_from(present, None).unwrap_err();
        assert_eq!(err, ResolveError::SpecifyVault);
    }

    #[test]
    fn read_omit_single_defaults() {
        let tmp = TempDir::new().unwrap();
        let a = vault_dir(&tmp, "a");
        let got = resolve_read_from(vec![resolved(&a, "a")], None).unwrap();
        assert_eq!(got.name, "a");
    }

    #[test]
    fn empty_registry_errors_for_both_resolvers() {
        assert_eq!(resolve_query_from(vec![], None).unwrap_err(), ResolveError::NoVaultRegistered);
        assert_eq!(resolve_read_from(vec![], Some("/whatever")).unwrap_err(), ResolveError::NoVaultRegistered);
    }

    #[test]
    fn pinned_rejects_mismatched_vault_but_accepts_omission() {
        let tmp = TempDir::new().unwrap();
        let v = vault_dir(&tmp, "pinned");
        let mode = ServeMode::Pinned {
            vault: v.clone(),
            name: "pinned".into(),
            wiki_root: v.join(".codebus").join("wiki"),
        };
        // Omitted → the pinned vault.
        assert_eq!(resolve_for_read(&mode, None).unwrap().name, "pinned");
        // Matching path → ok.
        assert!(resolve_for_read(&mode, Some(&v.display().to_string())).is_ok());
        // Different path → fail-loud (P1).
        let other = TempDir::new().unwrap();
        assert_eq!(
            resolve_for_read(&mode, Some(&other.path().display().to_string())).unwrap_err(),
            ResolveError::PinnedMismatch
        );
        assert!(!tags_source(&mode), "pinned mode keeps the v1 untagged result shape");
    }

    #[test]
    fn present_vaults_skips_missing_paths() {
        let home = TempDir::new().unwrap();
        let vaults = TempDir::new().unwrap();
        let present = vault_dir(&vaults, "real");
        let state_path = home.path().join("app-state.json");
        let state = AppState {
            schema_version: CURRENT_SCHEMA_VERSION,
            vault_list: vec![
                StoredVaultEntry {
                    path: present.display().to_string(),
                    display_name: "real".into(),
                    last_opened: "2026-06-27T00:00:00Z".into(),
                },
                StoredVaultEntry {
                    path: vaults.path().join("does-not-exist").display().to_string(),
                    display_name: "ghost".into(),
                    last_opened: "2026-06-27T00:00:00Z".into(),
                },
            ],
        };
        save_app_state(&state_path, &state).unwrap();

        let got = present_vaults_at(&state_path);
        assert_eq!(got.len(), 1, "missing vault dir must be skipped");
        assert_eq!(got[0].name, "real");
        assert!(tags_source(&ServeMode::Registry));
    }
}
