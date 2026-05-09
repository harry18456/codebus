//! Detect whether the source repository has drifted relative to what the
//! vault manifest captured the last time `init` (or a previous re-sync)
//! ran. The verb commands (currently `goal`) consult this before deciding
//! whether to re-run the raw mirror.
//!
//! Fail-safe by design: when detection itself cannot complete (manifest
//! missing / malformed YAML / I/O error), `detect_drift` returns `true`
//! so the caller proceeds with a re-sync rather than skipping it. The
//! cost of an unnecessary re-sync (~100ms walk) is much smaller than the
//! cost of letting the agent see stale raw mirror content.

use std::fs;
use std::path::Path;

use crate::vault::manifest::{Manifest, SourceSignal};

/// Return `true` if the source has drifted (re-sync needed) OR if detection
/// itself failed (fail-safe). Return `false` only when the manifest reads
/// cleanly AND all three signal fields (`git_head`, `file_count`,
/// `total_bytes`) match the supplied `current`.
pub fn detect_drift(manifest_yaml: &Path, current: &SourceSignal) -> bool {
    match read_stored_signal(manifest_yaml) {
        Ok(stored) => {
            stored.git_head != current.git_head
                || stored.file_count != current.file_count
                || stored.total_bytes != current.total_bytes
        }
        Err(_) => true, // fail-safe → drift, force re-sync
    }
}

fn read_stored_signal(manifest_yaml: &Path) -> std::io::Result<SourceSignal> {
    let raw = fs::read_to_string(manifest_yaml)?;
    let manifest: Manifest = serde_yaml::from_str(&raw)
        .map_err(|e| std::io::Error::other(format!("parse manifest: {e}")))?;
    Ok(manifest.source_signal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_manifest(dir: &Path, signal: SourceSignal) -> std::path::PathBuf {
        let path = dir.join("manifest.yaml");
        let manifest = Manifest {
            codebus_version: "0.3.0-test".to_string(),
            created_at: "2026-05-09T00:00:00Z".to_string(),
            repo_root: "/repo".to_string(),
            last_sync_at: "2026-05-09T00:00:00Z".to_string(),
            source_signal: signal,
        };
        let yaml = serde_yaml::to_string(&manifest).unwrap();
        fs::write(&path, yaml).unwrap();
        path
    }

    fn signal(git_head: Option<&str>, file_count: usize, total_bytes: u64) -> SourceSignal {
        SourceSignal {
            git_head: git_head.map(|s| s.to_string()),
            file_count,
            total_bytes,
        }
    }

    #[test]
    fn unchanged_when_all_three_fields_match() {
        let tmp = TempDir::new().unwrap();
        let stored = signal(Some("ref: refs/heads/main\n"), 142, 89234);
        let manifest_path = write_manifest(tmp.path(), stored.clone());
        assert!(!detect_drift(&manifest_path, &stored));
    }

    #[test]
    fn drift_when_git_head_differs() {
        let tmp = TempDir::new().unwrap();
        let stored = signal(Some("ref: refs/heads/main\n"), 142, 89234);
        let manifest_path = write_manifest(tmp.path(), stored);
        let current = signal(Some("ref: refs/heads/feature\n"), 142, 89234);
        assert!(detect_drift(&manifest_path, &current));
    }

    #[test]
    fn drift_when_file_count_differs() {
        let tmp = TempDir::new().unwrap();
        let stored = signal(Some("ref: refs/heads/main\n"), 142, 89234);
        let manifest_path = write_manifest(tmp.path(), stored);
        let current = signal(Some("ref: refs/heads/main\n"), 143, 89234);
        assert!(detect_drift(&manifest_path, &current));
    }

    #[test]
    fn drift_when_total_bytes_differs() {
        let tmp = TempDir::new().unwrap();
        let stored = signal(Some("ref: refs/heads/main\n"), 142, 89234);
        let manifest_path = write_manifest(tmp.path(), stored);
        let current = signal(Some("ref: refs/heads/main\n"), 142, 89890);
        assert!(detect_drift(&manifest_path, &current));
    }

    #[test]
    fn fail_safe_drift_when_manifest_missing() {
        let tmp = TempDir::new().unwrap();
        let manifest_path = tmp.path().join("manifest.yaml");
        // file does not exist
        assert!(!manifest_path.exists());
        let current = signal(Some("ref: refs/heads/main\n"), 0, 0);
        assert!(detect_drift(&manifest_path, &current));
    }

    #[test]
    fn fail_safe_drift_when_manifest_malformed() {
        let tmp = TempDir::new().unwrap();
        let manifest_path = tmp.path().join("manifest.yaml");
        // not valid YAML for the Manifest schema (missing required fields)
        fs::write(&manifest_path, "this is not :: valid yaml: [{").unwrap();
        let current = signal(Some("ref: refs/heads/main\n"), 0, 0);
        assert!(detect_drift(&manifest_path, &current));
    }

    #[test]
    fn unchanged_when_both_git_head_are_none() {
        // Non-git source repos: stored.git_head == None == current.git_head;
        // file_count + total_bytes equal → no drift.
        let tmp = TempDir::new().unwrap();
        let stored = signal(None, 7, 100);
        let manifest_path = write_manifest(tmp.path(), stored.clone());
        assert!(!detect_drift(&manifest_path, &stored));
    }
}
