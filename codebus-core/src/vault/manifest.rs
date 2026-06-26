//! `.codebus/manifest.yaml` — vault metadata + sync state.
//!
//! Five top-level fields:
//!   - codebus_version (write-once at first init)
//!   - created_at      (write-once at first init, UTC ISO 8601)
//!   - repo_root       (write-once at first init, absolute path)
//!   - last_sync_at    (updated every init, UTC ISO 8601)
//!   - source_signal   (updated every init; nested mapping)
//!       - git_head: Option<String>  // verbatim .git/HEAD content
//!       - file_count: usize         // from raw_sync SyncSummary
//!       - total_bytes: u64          // from raw_sync SyncSummary

use std::fs;
use std::io;
use std::path::Path;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::vault::raw_sync::SyncSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestOutcome {
    Written,
    Updated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSignal {
    pub git_head: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub codebus_version: String,
    pub created_at: String,
    pub repo_root: String,
    pub last_sync_at: String,
    pub source_signal: SourceSignal,
}

/// Compute the source-state signal for the current init invocation.
/// `git_head` is the verbatim contents of `<repo_root>/.git/HEAD` (preserves
/// `ref: refs/heads/main\n` or detached SHA forms); `None` if no git repo.
pub fn compute_source_signal(repo_root: &Path, sync_summary: &SyncSummary) -> SourceSignal {
    let git_head = read_git_head(repo_root);
    SourceSignal {
        git_head,
        file_count: sync_summary.files,
        total_bytes: sync_summary.bytes,
    }
}

fn read_git_head(repo_root: &Path) -> Option<String> {
    let head_path = repo_root.join(".git").join("HEAD");
    if !head_path.is_file() {
        return None;
    }
    fs::read_to_string(&head_path).ok()
}

fn now_utc_iso8601() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Write or update the manifest at `<vault_root>/manifest.yaml`.
/// First init writes all five top-level fields plus `source_signal`.
/// Subsequent inits preserve `codebus_version`, `created_at`, `repo_root`
/// from the existing file, and update `last_sync_at` + `source_signal`.
pub fn write_or_update_manifest(
    repo_root: &Path,
    vault_root: &Path,
    codebus_version: &str,
    signal: SourceSignal,
) -> io::Result<ManifestOutcome> {
    let path = vault_root.join("manifest.yaml");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let now = now_utc_iso8601();

    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        let mut existing: Manifest = serde_yaml::from_str(&raw)
            .map_err(|e| io::Error::other(format!("parse manifest: {e}")))?;
        existing.last_sync_at = now;
        existing.source_signal = signal;
        let yaml = serde_yaml::to_string(&existing)
            .map_err(|e| io::Error::other(format!("serialize manifest: {e}")))?;
        fs::write(&path, yaml)?;
        return Ok(ManifestOutcome::Updated);
    }

    let abs_repo = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let manifest = Manifest {
        codebus_version: codebus_version.to_string(),
        created_at: now.clone(),
        repo_root: abs_repo.to_string_lossy().into_owned(),
        last_sync_at: now,
        source_signal: signal,
    };
    let yaml = serde_yaml::to_string(&manifest)
        .map_err(|e| io::Error::other(format!("serialize manifest: {e}")))?;
    fs::write(&path, yaml)?;
    Ok(ManifestOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn dummy_summary(files: usize, bytes: u64) -> SyncSummary {
        SyncSummary {
            files,
            bytes,
            pii_matches: 0,
            pii_skipped_files: 0,
            pii_masked_matches: 0,
            oversized_skipped_files: 0,
            unscanned_files: 0,
        }
    }

    #[test]
    fn writes_manifest_with_all_fields_on_first_init() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().join(".codebus");
        fs::create_dir_all(&vault).unwrap();

        let signal = SourceSignal {
            git_head: None,
            file_count: 12,
            total_bytes: 4096,
        };
        let outcome = write_or_update_manifest(tmp.path(), &vault, "0.3.0-test", signal).unwrap();
        assert_eq!(outcome, ManifestOutcome::Written);

        let body = fs::read_to_string(vault.join("manifest.yaml")).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
        let map = parsed.as_mapping().unwrap();
        assert_eq!(map.len(), 5);
        for key in [
            "codebus_version",
            "created_at",
            "repo_root",
            "last_sync_at",
            "source_signal",
        ] {
            assert!(
                map.contains_key(serde_yaml::Value::String(key.into())),
                "missing top-level key `{key}`"
            );
        }
        let sig = map
            .get(serde_yaml::Value::String("source_signal".into()))
            .and_then(|v| v.as_mapping())
            .unwrap();
        assert_eq!(sig.len(), 3);
        for key in ["git_head", "file_count", "total_bytes"] {
            assert!(
                sig.contains_key(serde_yaml::Value::String(key.into())),
                "missing source_signal key `{key}`"
            );
        }
    }

    #[test]
    fn re_init_preserves_write_once_fields_and_updates_sync_state() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().join(".codebus");
        fs::create_dir_all(&vault).unwrap();

        let signal_first = SourceSignal {
            git_head: Some("first\n".into()),
            file_count: 10,
            total_bytes: 1000,
        };
        write_or_update_manifest(tmp.path(), &vault, "0.3.0-first", signal_first).unwrap();
        let body_first = fs::read_to_string(vault.join("manifest.yaml")).unwrap();
        let parsed_first: Manifest = serde_yaml::from_str(&body_first).unwrap();

        let signal_second = SourceSignal {
            git_head: Some("second\n".into()),
            file_count: 15,
            total_bytes: 2000,
        };
        let outcome =
            write_or_update_manifest(tmp.path(), &vault, "0.4.0-second", signal_second.clone())
                .unwrap();
        assert_eq!(outcome, ManifestOutcome::Updated);

        let body_second = fs::read_to_string(vault.join("manifest.yaml")).unwrap();
        let parsed_second: Manifest = serde_yaml::from_str(&body_second).unwrap();

        // Write-once fields preserved
        assert_eq!(parsed_second.codebus_version, "0.3.0-first");
        assert_eq!(parsed_second.created_at, parsed_first.created_at);
        assert_eq!(parsed_second.repo_root, parsed_first.repo_root);

        // Sync state updated
        assert_eq!(parsed_second.source_signal, signal_second);
        // last_sync_at format check (precise equality might fail due to second-resolution timing,
        // but at minimum the field shape is correct)
        assert!(parsed_second.last_sync_at.ends_with('Z'));
        assert!(parsed_second.last_sync_at.contains('T'));
    }

    #[test]
    fn compute_source_signal_records_git_head_when_git_repo() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let head_content = "ref: refs/heads/main\n";
        fs::write(tmp.path().join(".git/HEAD"), head_content).unwrap();

        let signal = compute_source_signal(tmp.path(), &dummy_summary(42, 12345));
        assert_eq!(signal.git_head, Some(head_content.to_string()));
        assert_eq!(signal.file_count, 42);
        assert_eq!(signal.total_bytes, 12345);
    }

    #[test]
    fn compute_source_signal_records_null_git_head_when_non_git() {
        let tmp = TempDir::new().unwrap();
        let signal = compute_source_signal(tmp.path(), &dummy_summary(7, 100));
        assert_eq!(signal.git_head, None);
        assert_eq!(signal.file_count, 7);
        assert_eq!(signal.total_bytes, 100);
    }

    #[test]
    fn null_git_head_serializes_to_yaml_null() {
        let tmp = TempDir::new().unwrap();
        let vault = tmp.path().join(".codebus");
        fs::create_dir_all(&vault).unwrap();

        let signal = SourceSignal {
            git_head: None,
            file_count: 1,
            total_bytes: 1,
        };
        write_or_update_manifest(tmp.path(), &vault, "0.3", signal).unwrap();
        let body = fs::read_to_string(vault.join("manifest.yaml")).unwrap();
        let v: serde_yaml::Value = serde_yaml::from_str(&body).unwrap();
        let sig = v.get("source_signal").unwrap();
        let head = sig.get("git_head").unwrap();
        assert!(head.is_null(), "expected null git_head, got {head:?}");
    }
}
