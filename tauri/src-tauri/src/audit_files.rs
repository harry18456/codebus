//! Workspace-level audit JSONL IPC for the frontend audit views.
//!
//! Single Tauri command `read_audit_jsonl(workspace_root, audit_kind)`
//! exposes the seven workspace-level audit files under
//! `<workspace_root>/.codebus/` as parsed JSON entries. The audit_kind
//! enum is fixed at seven values mirroring CLAUDE.md `七層 Audit JSONL`;
//! callers cannot read arbitrary files. Path validation reuses the
//! same red-team coverage as `tutorial::validate_path` but with a
//! distinct prefix (`.codebus/`) and stricter extension allowlist
//! (`.jsonl` only).
//!
//! A defensive parity test (`tests/audit_kind_filename_parity.rs`)
//! grep-checks that the Rust mapping below matches the Python
//! `_<NAME>_FILENAME` constants in
//! `sidecar/src/codebus_agent/_audit_paths.py` so the two language
//! sides cannot drift independently.

use std::path::{Path, PathBuf};

use tokio::io::AsyncReadExt;

const AUDIT_SUBDIR: &str = ".codebus";

const ALLOWED_EXTENSION: &str = "jsonl";

const READ_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Canonical mapping: audit_kind enum → filename under `<ws>/.codebus/`.
///
/// Order mirrors the seven-tab UI declaration in `frontend-shell`
/// Requirement `AuditPanel surfaces seven workspace-level audit JSONL
/// tabs`. The `tests/audit_kind_filename_parity.rs` defensive test
/// asserts pair equality with the Python `_<NAME>_FILENAME` constants
/// in `sidecar/src/codebus_agent/_audit_paths.py`.
pub const AUDIT_KIND_TO_FILENAME: &[(&str, &str)] = &[
    ("sanitize", "sanitize_audit.jsonl"),
    ("tool", "tool_audit.jsonl"),
    ("reasoning", "reasoning_log.jsonl"),
    ("token", "token_usage.jsonl"),
    ("llm", "llm_calls.jsonl"),
    ("kb_growth", "kb_growth.jsonl"),
    ("generator", "generator_log.jsonl"),
];

/// Windows treats these stems as device handles regardless of
/// extension. Match case-folded stem only.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
    "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8",
    "lpt9",
];

fn audit_kind_to_filename(audit_kind: &str) -> Option<&'static str> {
    AUDIT_KIND_TO_FILENAME
        .iter()
        .find_map(|(k, v)| if *k == audit_kind { Some(*v) } else { None })
}

fn is_safe_segment(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("empty path segment".to_string());
    }
    if name.contains(':') {
        return Err(format!("segment contains ':' (Windows ADS / drive): {name}"));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(format!("segment ends with '.' or ' ': {name}"));
    }
    let stem_lower = name
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if WINDOWS_RESERVED_NAMES.iter().any(|r| *r == stem_lower) {
        return Err(format!("segment matches Windows reserved name: {name}"));
    }
    Ok(())
}

/// Resolve `<workspace_root>/.codebus/<filename>.jsonl` for an
/// audit_kind, enforcing the same path-safety contract as
/// `tutorial::validate_path` but with `.codebus/` prefix and `.jsonl`
/// extension allowlist.
///
/// Returns `Err(code)` on:
///   - `E_AUDIT_KIND_INVALID` — kind not in `AUDIT_KIND_TO_FILENAME`
///   - `E_WORKSPACE_INVALID` — ws_root not absolute / not exists / not a dir
///   - `E_INVALID_PATH` — segment safety violation (rare; defensive)
///   - `E_DENIED` — canonical path escapes workspace via symlink
pub fn validate_audit_path(
    workspace_root: &str,
    audit_kind: &str,
) -> Result<PathBuf, String> {
    let filename = audit_kind_to_filename(audit_kind)
        .ok_or_else(|| "E_AUDIT_KIND_INVALID".to_string())?;

    let ws = Path::new(workspace_root);
    if !ws.is_absolute() {
        log::warn!("validate_audit_path: workspace_root not absolute: {workspace_root}");
        return Err("E_WORKSPACE_INVALID".to_string());
    }
    if !ws.exists() {
        log::warn!("validate_audit_path: workspace_root does not exist: {workspace_root}");
        return Err("E_WORKSPACE_INVALID".to_string());
    }
    if !ws.is_dir() {
        log::warn!("validate_audit_path: workspace_root not a directory: {workspace_root}");
        return Err("E_WORKSPACE_INVALID".to_string());
    }

    // Defensive segment checks even though the segments are
    // hard-coded constants — guards future mutations of
    // AUDIT_KIND_TO_FILENAME.
    is_safe_segment(AUDIT_SUBDIR).map_err(|e| {
        log::warn!("validate_audit_path: bad subdir segment: {e}");
        "E_INVALID_PATH".to_string()
    })?;
    is_safe_segment(filename).map_err(|e| {
        log::warn!("validate_audit_path: bad filename segment: {e}");
        "E_INVALID_PATH".to_string()
    })?;

    let extension = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    if extension.as_deref() != Some(ALLOWED_EXTENSION) {
        log::warn!(
            "validate_audit_path: extension not allowed (only .{ALLOWED_EXTENSION}): {filename}"
        );
        return Err("E_INVALID_PATH".to_string());
    }

    let ws_canonical = dunce::canonicalize(ws).map_err(|e| {
        log::warn!("validate_audit_path: canonicalize ws_root failed: {e}");
        "E_WORKSPACE_INVALID".to_string()
    })?;
    let joined = ws_canonical.join(AUDIT_SUBDIR).join(filename);

    // File may not exist yet; fall back to lexical resolution while
    // walking each existing ancestor to catch symlink escapes.
    let joined_canonical = match dunce::canonicalize(&joined) {
        Ok(p) => p,
        Err(_) => {
            let mut cursor = ws_canonical.clone();
            for segment in &[AUDIT_SUBDIR, filename] {
                cursor = cursor.join(segment);
                if let Ok(meta) = std::fs::symlink_metadata(&cursor) {
                    if meta.file_type().is_symlink() {
                        let resolved = dunce::canonicalize(&cursor).map_err(|e| {
                            log::warn!(
                                "validate_audit_path: canonicalize symlink ancestor {} failed: {e}",
                                cursor.display()
                            );
                            "E_DENIED".to_string()
                        })?;
                        if !resolved.starts_with(&ws_canonical) {
                            log::warn!(
                                "validate_audit_path: symlink resolves outside workspace: {}",
                                cursor.display()
                            );
                            return Err("E_DENIED".to_string());
                        }
                    }
                }
            }
            cursor
        }
    };

    if !joined_canonical.starts_with(&ws_canonical) {
        log::warn!(
            "validate_audit_path: resolved path escapes workspace_root: {}",
            joined_canonical.display()
        );
        return Err("E_DENIED".to_string());
    }

    Ok(joined_canonical)
}

/// Read `<workspace_root>/.codebus/<filename>.jsonl` (resolved via
/// `validate_audit_path`) and return parsed JSON entries.
///
/// Behaviour matrix:
///   - File missing on disk → `Ok(vec![])` (empty audit ≠ error).
///   - File exists but not a regular file → `Err("E_NOT_REGULAR_FILE")`.
///   - File size > 5 MiB → `Err("E_AUDIT_TOO_LARGE")` pre-parse.
///   - JSON parse error on any single line → SKIP that line with
///     `log::warn!`, continue. A single corrupt line MUST NOT poison
///     the whole list.
///   - Any audit_kind / path / IO failure → `Err("E_*")` per
///     `validate_audit_path`'s vocabulary plus `E_NOT_REGULAR_FILE`,
///     `E_AUDIT_TOO_LARGE`, and `E_IO`.
#[tauri::command]
pub async fn read_audit_jsonl(
    workspace_root: String,
    audit_kind: String,
) -> Result<Vec<serde_json::Value>, String> {
    read_audit_jsonl_inner(&workspace_root, &audit_kind).await
}

pub async fn read_audit_jsonl_inner(
    workspace_root: &str,
    audit_kind: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let resolved = validate_audit_path(workspace_root, audit_kind)?;

    let metadata = match tokio::fs::symlink_metadata(&resolved).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => {
            log::warn!(
                "read_audit_jsonl: symlink_metadata failed for {}: {e}",
                resolved.display()
            );
            return Err("E_IO".to_string());
        }
    };

    if !metadata.file_type().is_file() {
        log::warn!(
            "read_audit_jsonl: not a regular file: {}",
            resolved.display()
        );
        return Err("E_NOT_REGULAR_FILE".to_string());
    }

    if metadata.len() > READ_MAX_BYTES {
        log::warn!(
            "read_audit_jsonl: file too large ({} bytes > {READ_MAX_BYTES}): {}",
            metadata.len(),
            resolved.display()
        );
        return Err("E_AUDIT_TOO_LARGE".to_string());
    }

    let mut file = match tokio::fs::File::open(&resolved).await {
        Ok(f) => f,
        Err(e) => {
            log::warn!("read_audit_jsonl: open failed for {}: {e}", resolved.display());
            return Err("E_IO".to_string());
        }
    };

    let mut buf = Vec::with_capacity(metadata.len() as usize);
    if let Err(e) = file.read_to_end(&mut buf).await {
        log::warn!("read_audit_jsonl: read_to_end failed: {e}");
        return Err("E_IO".to_string());
    }

    let text = match String::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("read_audit_jsonl: invalid UTF-8: {e}");
            return Err("E_IO".to_string());
        }
    };

    let mut entries = Vec::new();
    for (lineno, raw) in text.split('\n').enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) => entries.push(v),
            Err(e) => {
                log::warn!(
                    "read_audit_jsonl: skip corrupt line {} in {}: {e}",
                    lineno + 1,
                    resolved.display()
                );
            }
        }
    }
    Ok(entries)
}
