//! Idempotent registration of `.codebus/wiki/` into Obsidian's user-level
//! vault registry (`obsidian.json`).
//!
//! # What this module does
//!
//! When codebus runs `init`/`goal`/`query`/`fix`, it wants Obsidian to know
//! about `<repo>/.codebus/wiki/` so that `obsidian://open?vault=<id>&file=...`
//! URIs (emitted by the terminal renderer as OSC 8 hyperlinks) can be
//! Ctrl+Clicked open. That mapping lives in a single user-level JSON file
//! whose path comes from [`crate::obsidian::config_path::obsidian_json_path`].
//!
//! [`register_vault`] performs the registration; [`lookup_vault_id`] performs
//! a read-only lookup of an already-registered vault id (used by the run
//! flow after `init` has already done the actual write).
//!
//! # Safety / fail-soft contract
//!
//! Both entry points are **fail-soft**: codebus must produce a working wiki
//! whether or not Obsidian is installed, running, or reachable. Anything
//! short of clean success is reported as a [`RegisterOutcome`] variant or a
//! `Ok(None)` lookup — never a panic, never an aborting error bubbled up.
//!
//! # Audit notes
//!
//! - **Scoundrel** — a malicious `obsidian.json` could contain a vault entry
//!   whose `path` is a hostile traversal like `\\..\\..\\system32`. We do
//!   **not** canonicalize, validate, or execute anything from the file;
//!   we only round-trip unknown entries verbatim and write our own. The
//!   threat surface is what the user's existing Obsidian install already
//!   trusts — codebus does not widen it.
//! - **Lazy Developer** — id hash uses `String::to_lowercase()` (Unicode-
//!   aware), not `to_ascii_lowercase()`. For typical filesystem paths the
//!   two are equivalent (paths are mostly ASCII), but Unicode-aware lower-
//!   case is the safer default and matches the spike implementation.
//! - **Confused Developer** — `RegisterOutcome::IoError { reason }` formats
//!   as `"<kind>: <message>"` so logs distinguish `PermissionDenied` from
//!   `NotFound` etc.
//!
//! # Testability
//!
//! The OS-level lookups (`config_path::obsidian_json_path`,
//! `process_detect::is_obsidian_running`) are not easily overridable from a
//! unit test. We split the I/O into `_at` inner functions that take the
//! `obsidian.json` path and an explicit `obsidian_running` flag, so tests
//! drive controlled tempdir paths without touching the user's real
//! `obsidian.json`. The thin public wrappers do the OS plumbing.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::obsidian::{config_path, process_detect};

/// Outcome of [`register_vault`]. Always returned by value — never panics,
/// never propagates I/O errors as `Err`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// Vault entry written or refreshed. `vault_id` is the effective key in
    /// `obsidian.json` — either the codebus-computed SHA-256 id (fresh
    /// insert) or the existing user-created id (same-path reuse).
    Registered { vault_id: String },
    /// `dirs::config_dir()/obsidian/` does not exist — Obsidian is not
    /// installed on this machine. Caller should silently skip.
    ObsidianNotInstalled,
    /// An Obsidian process is currently running — skip writing to avoid
    /// race-overwrite by Obsidian's own vault-list flush. Caller should
    /// emit a hint and skip.
    ObsidianRunning,
    /// Read / parse / write of `obsidian.json` failed. `reason` is a human-
    /// readable `"<kind>: <message>"` for logging. Init must not abort on
    /// this — caller logs a warning and continues.
    IoError { reason: String },
}

/// Internal serde model for one vault entry.
///
/// `path` is the absolute path with OS-native separators, exactly as Obsidian
/// itself writes it. `ts` is Unix milliseconds. `open` is `false` for codebus-
/// written entries — we register the vault but do not auto-open it on
/// Obsidian launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct VaultEntry {
    path: String,
    ts: u64,
    open: bool,
}

/// Internal serde model for `obsidian.json`.
///
/// `vaults` is `BTreeMap` (not `HashMap`) so the JSON output has stable key
/// ordering — testability + cleaner diffs if the user reads the file. The
/// `#[serde(flatten)] other` field captures any unknown top-level keys (e.g.
/// `frameless`, `width`, etc.) so we round-trip them verbatim and never
/// clobber unrelated Obsidian settings the user has tuned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ObsidianConfig {
    #[serde(default)]
    vaults: BTreeMap<String, VaultEntry>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

/// Register `<vault_path>` (typically `<repo>/.codebus/wiki/`) into the
/// user-level `obsidian.json`. Idempotent: an entry whose `path` already
/// matches reuses the existing key and only refreshes `ts`.
///
/// Returns a [`RegisterOutcome`] — never panics, never propagates I/O.
pub fn register_vault(vault_path: &Path) -> RegisterOutcome {
    let Some(json_path) = config_path::obsidian_json_path() else {
        return RegisterOutcome::ObsidianNotInstalled;
    };
    register_vault_at(
        vault_path,
        &json_path,
        process_detect::is_obsidian_running(),
    )
}

/// Read-only lookup of an already-registered vault id.
///
/// Used by the run flow (`goal`/`query`/`fix`) AFTER init has done the
/// actual registration: avoids a second write and any race window with
/// Obsidian. Returns:
/// - `Ok(Some(id))` if a same-path entry exists
/// - `Ok(None)` if `obsidian.json` is missing, contains no matching entry,
///   or Obsidian is not installed at all
/// - `Err(io::Error)` only on file-read I/O errors with `kind != NotFound`
pub fn lookup_vault_id(vault_path: &Path) -> io::Result<Option<String>> {
    let Some(json_path) = config_path::obsidian_json_path() else {
        return Ok(None);
    };
    lookup_vault_id_at(vault_path, &json_path)
}

/// Inner, testable form of [`register_vault`]. Tests drive this directly
/// with a controlled `json_path` and explicit `obsidian_running` flag.
pub(crate) fn register_vault_at(
    vault_path: &Path,
    json_path: &Path,
    obsidian_running: bool,
) -> RegisterOutcome {
    if obsidian_running {
        return RegisterOutcome::ObsidianRunning;
    }

    // Step 1: read existing config (or start blank).
    let mut cfg = match std::fs::read(json_path) {
        Ok(bytes) => match serde_json::from_slice::<ObsidianConfig>(&bytes) {
            Ok(parsed) => parsed,
            Err(err) => {
                return RegisterOutcome::IoError {
                    reason: format!("parse: {err}"),
                };
            }
        },
        Err(err) if err.kind() == io::ErrorKind::NotFound => ObsidianConfig::default(),
        Err(err) => {
            return RegisterOutcome::IoError {
                reason: format!("{kind}: {err}", kind = err.kind()),
            };
        }
    };

    // Step 2: compute target id (SHA-256[:16] of lowercased absolute path).
    let abs_path_string = vault_path.to_string_lossy().into_owned();
    let target_id = compute_vault_id(&abs_path_string);

    // Step 3: find any existing entry with the same path (case rules per OS).
    let existing_key = cfg
        .vaults
        .iter()
        .find_map(|(k, v)| same_path(&v.path, &abs_path_string).then(|| k.clone()));

    let now_ms = unix_now_ms();
    let effective_id = match existing_key {
        Some(key) => {
            // Reuse the existing id — only refresh `ts`. Preserve `open` and
            // the on-disk `path` casing exactly as Obsidian wrote it.
            if let Some(entry) = cfg.vaults.get_mut(&key) {
                entry.ts = now_ms;
            }
            key
        }
        None => {
            cfg.vaults.insert(
                target_id.clone(),
                VaultEntry {
                    path: abs_path_string,
                    ts: now_ms,
                    open: false,
                },
            );
            target_id
        }
    };

    // Step 4: write back. Compact JSON (Obsidian itself writes compact);
    // UTF-8 without BOM (std::fs::write default).
    let json_bytes = match serde_json::to_vec(&cfg) {
        Ok(b) => b,
        Err(err) => {
            return RegisterOutcome::IoError {
                reason: format!("serialize: {err}"),
            };
        }
    };
    if let Err(err) = std::fs::write(json_path, &json_bytes) {
        return RegisterOutcome::IoError {
            reason: format!("{kind}: {err}", kind = err.kind()),
        };
    }

    RegisterOutcome::Registered {
        vault_id: effective_id,
    }
}

/// Inner, testable form of [`lookup_vault_id`].
pub(crate) fn lookup_vault_id_at(
    vault_path: &Path,
    json_path: &Path,
) -> io::Result<Option<String>> {
    let bytes = match std::fs::read(json_path) {
        Ok(b) => b,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let cfg: ObsidianConfig = match serde_json::from_slice(&bytes) {
        Ok(c) => c,
        Err(err) => {
            // Surface parse failures as I/O errors (`InvalidData`) so callers
            // distinguish "file unreadable" from "no matching entry".
            return Err(io::Error::new(io::ErrorKind::InvalidData, err));
        }
    };
    let abs_path_string = vault_path.to_string_lossy().into_owned();
    let key = cfg
        .vaults
        .iter()
        .find_map(|(k, v)| same_path(&v.path, &abs_path_string).then(|| k.clone()));
    Ok(key)
}

/// Lowercase + SHA-256, hex-encode, take first 16 hex chars.
fn compute_vault_id(abs_path: &str) -> String {
    let lower = abs_path.to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(lower.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest.iter() {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex.truncate(16);
    hex
}

/// Path equality — case-insensitive AND separator-insensitive on Windows,
/// byte-equal elsewhere.
///
/// Spec scenario "Existing same-path entry reuses its id" requires Windows
/// case-insensitivity (NTFS is case-insensitive by default). Windows also
/// accepts both `/` and `\` as path separators interchangeably, and codebus
/// itself routes user input through `Path` which preserves whatever
/// separator the user provided — so `D:/foo` and `D:\foo` are the same
/// vault on disk but differ as strings. We normalize both separators to
/// `/` before comparison to handle the common case of CLI giving forward
/// slashes while Obsidian's GUI stores native backslashes.
///
/// On Linux/macOS we keep byte-equal: ext4 is case-sensitive; APFS is
/// case-insensitive but codebus follows the byte-equal convention used
/// elsewhere in the codebase to avoid surprising the user.
fn same_path(a: &str, b: &str) -> bool {
    if cfg!(target_os = "windows") {
        let normalize = |s: &str| s.replace('\\', "/");
        normalize(a).eq_ignore_ascii_case(&normalize(b))
    } else {
        a == b
    }
}

/// Current Unix milliseconds. Saturates at `0` on a clock skew that puts
/// the system before the Unix epoch (extremely unlikely; defensive).
fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Allocate a uniquely-named tempdir under `std::env::temp_dir()`.
    fn unique_tempdir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "codebus-obsidian-registry-{tag}-{nanos}-{pid}",
            pid = std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    /// Helper to read & parse the on-disk config back out for assertions.
    fn read_cfg(json_path: &Path) -> ObsidianConfig {
        let bytes = fs::read(json_path).expect("read obsidian.json");
        serde_json::from_slice(&bytes).expect("parse obsidian.json")
    }

    #[test]
    fn fresh_init_writes_new_vault_entry() {
        // Spec scenario "Fresh init writes new vault entry".
        let dir = unique_tempdir("fresh");
        let json_path = dir.join("obsidian.json");
        let vault_path = dir.join("repo").join(".codebus").join("wiki");

        let before = unix_now_ms();
        let outcome = register_vault_at(&vault_path, &json_path, false);
        let after = unix_now_ms();

        // Outcome shape.
        let id = match outcome {
            RegisterOutcome::Registered { vault_id } => vault_id,
            other => panic!("expected Registered, got {other:?}"),
        };

        // Id is the lowercase 16-hex prefix of SHA-256 of the lowercase path.
        let expected_id = compute_vault_id(&vault_path.to_string_lossy());
        assert_eq!(id, expected_id, "vault_id matches SHA-256[:16] of path");
        assert_eq!(id.len(), 16, "id is 16 hex chars");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "id is lowercase hex"
        );

        // Persisted entry has the right shape.
        let cfg = read_cfg(&json_path);
        let entry = cfg.vaults.get(&id).expect("entry under target id");
        assert_eq!(entry.path, vault_path.to_string_lossy());
        assert!(!entry.open, "open is false");
        assert!(
            entry.ts >= before && entry.ts <= after.saturating_add(10),
            "ts is recent ({} not in [{}, {}+10])",
            entry.ts,
            before,
            after
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn obsidian_running_returns_running_outcome() {
        // Spec scenario "Obsidian running emits hint and skips" — we DO NOT
        // touch obsidian.json when Obsidian is running.
        let dir = unique_tempdir("running");
        let json_path = dir.join("obsidian.json");
        let pre_existing = br#"{"vaults":{"abc":{"path":"/tmp/x","ts":42,"open":true}}}"#;
        fs::write(&json_path, pre_existing).unwrap();
        let mtime_before = fs::metadata(&json_path).unwrap().modified().unwrap();

        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let outcome = register_vault_at(&vault_path, &json_path, true);

        assert_eq!(outcome, RegisterOutcome::ObsidianRunning);

        // File contents preserved byte-for-byte.
        let after_bytes = fs::read(&json_path).unwrap();
        assert_eq!(
            after_bytes,
            pre_existing.to_vec(),
            "obsidian.json must not be modified when Obsidian is running"
        );
        let mtime_after = fs::metadata(&json_path).unwrap().modified().unwrap();
        assert_eq!(mtime_before, mtime_after, "mtime unchanged");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn existing_same_path_entry_reuses_id() {
        // Spec scenario "Existing same-path entry reuses its id". The user
        // previously added this vault manually in Obsidian's UI, producing
        // a random id. Codebus must reuse it, not insert a parallel entry.
        let dir = unique_tempdir("reuse");
        let json_path = dir.join("obsidian.json");
        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let path_string = vault_path.to_string_lossy().into_owned();

        // Pre-populate with a "user-created" random id.
        let prepopulated = serde_json::json!({
            "vaults": {
                "random_user_id": {
                    "path": path_string,
                    "ts": 1u64,
                    "open": true,
                }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let outcome = register_vault_at(&vault_path, &json_path, false);

        let id = match outcome {
            RegisterOutcome::Registered { vault_id } => vault_id,
            other => panic!("expected Registered, got {other:?}"),
        };
        assert_eq!(id, "random_user_id", "must reuse existing key, not SHA-256 id");

        let cfg = read_cfg(&json_path);
        assert_eq!(cfg.vaults.len(), 1, "no duplicate entry inserted");
        let entry = cfg.vaults.get("random_user_id").expect("entry preserved");
        assert!(entry.ts > 1, "ts refreshed (was 1)");
        assert!(entry.open, "open field preserved (was true)");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn case_insensitive_match_on_windows() {
        // Windows: NTFS is case-insensitive; pre-existing entry stored in
        // upper case must still match a lookup in lower case.
        let dir = unique_tempdir("winci");
        let json_path = dir.join("obsidian.json");
        let upper_path = "C:\\Users\\TEST\\REPO\\.CODEBUS\\WIKI";
        let lower_path = "c:\\users\\test\\repo\\.codebus\\wiki";

        let prepopulated = serde_json::json!({
            "vaults": {
                "preexisting_id": { "path": upper_path, "ts": 1u64, "open": false }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let outcome = register_vault_at(Path::new(lower_path), &json_path, false);

        match outcome {
            RegisterOutcome::Registered { vault_id } => assert_eq!(vault_id, "preexisting_id"),
            other => panic!("expected Registered with reused id, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn separator_insensitive_match_on_windows() {
        // Windows: CLI typically gives forward slashes (`D:/foo/bar`) while
        // Obsidian's GUI stores native backslashes (`D:\\foo\\bar`). They
        // refer to the same on-disk path; lookup must match across both.
        let dir = unique_tempdir("winsep");
        let json_path = dir.join("obsidian.json");
        let backslash_path = "D:\\side_project\\buddy-gacha\\.codebus\\wiki";
        let forward_path = "D:/side_project/buddy-gacha/.codebus/wiki";

        let prepopulated = serde_json::json!({
            "vaults": {
                "preexisting_id": { "path": backslash_path, "ts": 1u64, "open": false }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let id = lookup_vault_id_at(Path::new(forward_path), &json_path)
            .expect("lookup ok")
            .expect("entry must match across separator forms");
        assert_eq!(id, "preexisting_id");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn case_sensitive_no_match_on_unix() {
        // Non-Windows: byte-equal — different case is a different path.
        let dir = unique_tempdir("unixcs");
        let json_path = dir.join("obsidian.json");
        let upper_path = "/HOME/TEST/REPO/.CODEBUS/WIKI";
        let lower_path = "/home/test/repo/.codebus/wiki";

        let prepopulated = serde_json::json!({
            "vaults": {
                "preexisting_id": { "path": upper_path, "ts": 1u64, "open": false }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let outcome = register_vault_at(Path::new(lower_path), &json_path, false);

        match outcome {
            RegisterOutcome::Registered { vault_id } => {
                assert_ne!(
                    vault_id, "preexisting_id",
                    "case-sensitive OS must NOT reuse different-case entry"
                );
            }
            other => panic!("expected Registered (with new id), got {other:?}"),
        }
        let cfg = read_cfg(&json_path);
        assert_eq!(cfg.vaults.len(), 2, "both entries should now exist");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_error_returns_io_error() {
        // Garbage bytes → IoError, original file untouched.
        let dir = unique_tempdir("parse");
        let json_path = dir.join("obsidian.json");
        let garbage = b"this is not json {{{";
        fs::write(&json_path, garbage).unwrap();

        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let outcome = register_vault_at(&vault_path, &json_path, false);

        match outcome {
            RegisterOutcome::IoError { reason } => {
                assert!(!reason.is_empty(), "reason populated");
            }
            other => panic!("expected IoError, got {other:?}"),
        }

        // Original content preserved (no overwrite on parse failure).
        let still = fs::read(&json_path).unwrap();
        assert_eq!(still, garbage.to_vec(), "garbage preserved on parse error");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_failure_returns_io_error() {
        // Make json_path point at a non-existent parent directory so
        // std::fs::write fails. We DO NOT pre-create the parent.
        let dir = unique_tempdir("writefail");
        let bogus_parent = dir.join("does-not-exist");
        let json_path = bogus_parent.join("obsidian.json");
        // Sanity: parent missing.
        assert!(!bogus_parent.exists());

        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let outcome = register_vault_at(&vault_path, &json_path, false);

        match outcome {
            RegisterOutcome::IoError { reason } => {
                assert!(!reason.is_empty(), "reason populated");
            }
            other => panic!("expected IoError, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_top_level_keys_round_trip() {
        // Validates `#[serde(flatten)] other`: Obsidian writes more top-level
        // keys than `vaults` (e.g. `frameless`). We must round-trip them.
        let dir = unique_tempdir("rt");
        let json_path = dir.join("obsidian.json");
        let prepopulated = serde_json::json!({
            "vaults": {},
            "frameless": true,
            "foo": "bar",
            "nested": { "k": 1 },
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let outcome = register_vault_at(&vault_path, &json_path, false);
        assert!(
            matches!(outcome, RegisterOutcome::Registered { .. }),
            "registration should succeed, got {outcome:?}"
        );

        // Re-parse as a generic Value to assert the flat keys survived.
        let bytes = fs::read(&json_path).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v.get("frameless"), Some(&serde_json::Value::Bool(true)));
        assert_eq!(
            v.get("foo"),
            Some(&serde_json::Value::String("bar".into()))
        );
        assert_eq!(v.get("nested").and_then(|n| n.get("k")), Some(&serde_json::json!(1)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lookup_returns_id_when_entry_present() {
        let dir = unique_tempdir("lookuphit");
        let json_path = dir.join("obsidian.json");
        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let path_string = vault_path.to_string_lossy().into_owned();

        let prepopulated = serde_json::json!({
            "vaults": {
                "deadbeef0badc0de": {
                    "path": path_string,
                    "ts": 1u64,
                    "open": false,
                }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let id = lookup_vault_id_at(&vault_path, &json_path)
            .expect("lookup ok")
            .expect("entry present");
        assert_eq!(id, "deadbeef0badc0de");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lookup_returns_none_when_no_match() {
        let dir = unique_tempdir("lookupmiss");
        let json_path = dir.join("obsidian.json");
        let other_path = "/some/other/vault";

        let prepopulated = serde_json::json!({
            "vaults": {
                "abc": { "path": other_path, "ts": 1u64, "open": false }
            }
        });
        fs::write(&json_path, serde_json::to_vec(&prepopulated).unwrap()).unwrap();

        let target = dir.join("repo").join(".codebus").join("wiki");
        let result = lookup_vault_id_at(&target, &json_path).expect("lookup ok");
        assert!(result.is_none(), "no match → None");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lookup_returns_none_when_file_missing() {
        let dir = unique_tempdir("lookupabs");
        let json_path = dir.join("obsidian.json");
        // Do NOT create the file.
        assert!(!json_path.exists());

        let vault_path = dir.join("repo").join(".codebus").join("wiki");
        let result = lookup_vault_id_at(&vault_path, &json_path).expect("lookup ok on missing");
        assert!(result.is_none(), "missing file → Ok(None)");

        let _ = fs::remove_dir_all(&dir);
    }

    /// Public-API smoke test for `register_vault`: we cannot reliably
    /// override `dirs::config_dir()` from a unit test without an env-var
    /// dance (and `dirs` caches results across calls). On a CI runner where
    /// Obsidian is not installed, this returns `ObsidianNotInstalled`; on
    /// a developer machine where it IS installed, the behaviour depends on
    /// whether Obsidian is currently running. Either way the call must not
    /// panic. Ignored to keep CI deterministic — run manually with
    /// `cargo test -p codebus-core public_register_vault_smoke -- --ignored`.
    #[test]
    #[ignore]
    fn public_register_vault_smoke() {
        let tmp = unique_tempdir("publicsmoke");
        let vault_path = tmp.join("wiki");
        let _ = register_vault(&vault_path);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compute_vault_id_is_lowercase_16_hex_of_sha256_of_lowercased_path() {
        // Lock-in test for the exact algorithm — guards against accidental
        // refactor (e.g. truncating digest bytes BEFORE hex-encoding).
        let id_a = compute_vault_id("/tmp/X/.CODEBUS/WIKI");
        let id_b = compute_vault_id("/tmp/x/.codebus/wiki");
        assert_eq!(id_a, id_b, "id is lowercase-invariant");
        assert_eq!(id_a.len(), 16);
        assert!(id_a.chars().all(|c| c.is_ascii_hexdigit()));

        // Cross-check the math by hand: sha256 of the lowercase form, hex,
        // first 16 chars.
        let mut h = Sha256::new();
        h.update(b"/tmp/x/.codebus/wiki");
        let d = h.finalize();
        let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(id_a, hex[..16]);
    }
}
