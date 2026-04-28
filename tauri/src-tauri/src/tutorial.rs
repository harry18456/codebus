//! Tutorial file IPC for the R-01 station-board frontend.
//!
//! Three Tauri commands expose `<workspace>/codebus-tutorials/` to the
//! Nuxt frontend through a single Rust trust boundary:
//!
//! - `read_tutorial_file` — read MOC / station markdown / route.json
//! - `write_progress_file` — persist `progress.json` (single-writer)
//! - `list_tutorial_tasks` — enumerate task directories for the
//!   implicit-latest fallback (D-T11)
//!
//! All three share `validate_path` for path safety: absolute workspace
//! root, `codebus-tutorials/` prefix, canonical containment, extension
//! allowlist, segment safety (Windows reserved names / ADS colon /
//! trailing dot/space / dotdot / dot), and symlink-outside rejection.
//!
//! IPC errors collapse to a fixed vocabulary
//! (`E_INVALID_PATH` / `E_WORKSPACE_INVALID` / `E_TASK_ID_INVALID` /
//! `E_NOT_FOUND` / `E_DENIED` / `E_NOT_REGULAR_FILE` / `E_IO`) so an
//! XSS-tainted frontend cannot enumerate the host filesystem layout
//! through error strings. Internal validation messages still surface
//! through `log::warn!` for developer diagnosis (server-side only).

use std::collections::BTreeMap;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;

const TUTORIALS_SUBDIR: &str = "codebus-tutorials";

const ALLOWED_EXTENSIONS: &[&str] = &["md", "json"];

/// Windows treats these stems as device handles regardless of
/// extension (`con.md` blocks on console input). Match case-folded
/// stem only — the `.md` suffix is irrelevant.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
    "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8",
    "lpt9",
];

const READ_MAX_BYTES: u64 = 5 * 1024 * 1024;

fn is_valid_task_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 80 {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
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
        return Err(format!("segment uses Windows reserved name: {name}"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialTaskMeta {
    pub id: String,
    pub frontmatter_raw: Option<String>,
    pub dir_mtime_unix: i64,
}

pub fn validate_path(
    workspace_root: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let ws = Path::new(workspace_root);
    if !ws.is_absolute() {
        return Err(format!("workspace_root must be absolute: {workspace_root}"));
    }
    if !ws.exists() {
        return Err(format!("workspace_root does not exist: {workspace_root}"));
    }
    if !ws.is_dir() {
        return Err(format!(
            "workspace_root is not a directory: {workspace_root}"
        ));
    }

    let rel_normalised = relative_path.replace('\\', "/");
    let prefix_with_slash = format!("{TUTORIALS_SUBDIR}/");
    if !rel_normalised.starts_with(&prefix_with_slash) {
        return Err(format!(
            "relative_path must start with '{prefix_with_slash}': {relative_path}"
        ));
    }
    for seg in rel_normalised.split('/') {
        if seg == ".." {
            return Err(format!("relative_path contains '..': {relative_path}"));
        }
        if seg == "." {
            return Err(format!("relative_path contains '.': {relative_path}"));
        }
        is_safe_segment(seg)?;
    }

    let extension = Path::new(&rel_normalised)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match extension.as_deref() {
        Some(ext) if ALLOWED_EXTENSIONS.contains(&ext) => {}
        _ => {
            return Err(format!(
                "extension not allowed (only {ALLOWED_EXTENSIONS:?}): {relative_path}"
            ));
        }
    }

    let ws_canonical = dunce::canonicalize(ws)
        .map_err(|e| format!("canonicalize workspace_root failed: {e}"))?;
    let joined = ws_canonical.join(&rel_normalised);
    let joined_canonical = match dunce::canonicalize(&joined) {
        Ok(p) => p,
        Err(_) => {
            // File may not exist yet (write path) — fall back to lexical
            // resolution. Re-walk every existing ancestor with
            // symlink_metadata to make sure no segment slipped in as a
            // symlink to outside the workspace.
            let mut cursor = ws_canonical.clone();
            for segment in Path::new(&rel_normalised).iter() {
                cursor = cursor.join(segment);
                if let Ok(meta) = std::fs::symlink_metadata(&cursor) {
                    if meta.file_type().is_symlink() {
                        let resolved = dunce::canonicalize(&cursor).map_err(|e| {
                            format!("canonicalize symlink ancestor failed: {e}")
                        })?;
                        if !resolved.starts_with(&ws_canonical) {
                            return Err(format!(
                                "symlink resolves outside workspace_root: {}",
                                cursor.display()
                            ));
                        }
                    }
                }
            }
            cursor
        }
    };

    if !joined_canonical.starts_with(&ws_canonical) {
        return Err(format!(
            "resolved path escapes workspace_root: {}",
            joined_canonical.display()
        ));
    }

    Ok(joined_canonical)
}

fn validate_workspace_for_write(workspace_root: &str) -> Result<PathBuf, String> {
    let ws = Path::new(workspace_root);
    if !ws.is_absolute() {
        return Err(format!("workspace_root must be absolute: {workspace_root}"));
    }
    if !ws.exists() {
        return Err(format!("workspace_root does not exist: {workspace_root}"));
    }
    if !ws.is_dir() {
        return Err(format!(
            "workspace_root is not a directory: {workspace_root}"
        ));
    }
    dunce::canonicalize(ws).map_err(|e| format!("canonicalize workspace_root failed: {e}"))
}

pub fn progress_path_for(
    workspace_root: &str,
    task_id: &str,
) -> Result<PathBuf, String> {
    if !is_valid_task_id(task_id) {
        return Err(format!(
            "task_id must match ^[a-z0-9_-]{{1,80}}$: {task_id}"
        ));
    }
    let ws_canonical = validate_workspace_for_write(workspace_root)?;
    Ok(ws_canonical
        .join(TUTORIALS_SUBDIR)
        .join(task_id)
        .join("progress.json"))
}

/// Workspace-canonical helper used by `write_progress_file` to re-check
/// containment after `create_dir_all` (the gap between
/// `progress_path_for` validation and the actual write is a TOCTOU
/// window we want to slam shut).
pub fn workspace_canonical(workspace_root: &str) -> Result<PathBuf, String> {
    validate_workspace_for_write(workspace_root)
}

pub fn list_tutorial_tasks_in(
    workspace_root: &str,
) -> Result<Vec<TutorialTaskMeta>, String> {
    let ws_canonical = validate_workspace_for_write(workspace_root)?;
    let tutorials_dir = ws_canonical.join(TUTORIALS_SUBDIR);
    if !tutorials_dir.exists() {
        return Ok(Vec::new());
    }
    if !tutorials_dir.is_dir() {
        return Err(format!(
            "{TUTORIALS_SUBDIR} exists but is not a directory: {}",
            tutorials_dir.display()
        ));
    }

    let mut entries: BTreeMap<String, TutorialTaskMeta> = BTreeMap::new();
    let read_dir = std::fs::read_dir(&tutorials_dir)
        .map_err(|e| format!("read_dir {TUTORIALS_SUBDIR} failed: {e}"))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("read_dir entry failed: {e}"))?;
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_dir() {
            continue;
        }
        let id = match path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let frontmatter_raw = read_frontmatter_block(&path.join("tutorial.md"));
        let dir_mtime_unix = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        entries.insert(
            id.clone(),
            TutorialTaskMeta {
                id,
                frontmatter_raw,
                dir_mtime_unix,
            },
        );
    }
    Ok(entries.into_values().collect())
}

fn read_frontmatter_block(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    let mut buf = String::new();
    let mut total = 0usize;
    for line in lines {
        if line.trim() == "---" {
            return Some(buf);
        }
        total += line.len() + 1;
        if total > 8 * 1024 {
            // Cap raw frontmatter at 8KB to bound the WebView heap and
            // gray-matter parse time. Anything larger than that is a
            // pathological / hostile tutorial.md.
            return None;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    None
}

pub static PROGRESS_WRITE_LOCK: Mutex<()> = Mutex::const_new(());

// ----- IPC error vocabulary ------------------------------------------------

/// Map an opaque internal validation error string to a fixed-vocabulary
/// IPC code. Internal detail is `log::warn!`-ed server-side only so an
/// XSS-tainted frontend cannot enumerate filesystem layout via error
/// strings (the security review's "info leak via error strings" finding).
fn wire_validation_error(detail: &str) -> String {
    if detail.contains("workspace_root") {
        "E_WORKSPACE_INVALID".to_string()
    } else if detail.contains("task_id") {
        "E_TASK_ID_INVALID".to_string()
    } else {
        "E_INVALID_PATH".to_string()
    }
}

fn wire_io_error(e: &IoError) -> String {
    match e.kind() {
        ErrorKind::NotFound => "E_NOT_FOUND",
        ErrorKind::PermissionDenied => "E_DENIED",
        ErrorKind::InvalidInput => "E_NOT_REGULAR_FILE",
        _ => "E_IO",
    }
    .to_string()
}

// ----- Tauri commands ------------------------------------------------------

#[tauri::command]
pub async fn read_tutorial_file(
    workspace_root: String,
    relative_path: String,
) -> Result<String, String> {
    let validated = validate_path(&workspace_root, &relative_path).map_err(|e| {
        log::warn!("read_tutorial_file validate_path: {e}");
        wire_validation_error(&e)
    })?;
    open_and_read(&validated).await.map_err(|e| {
        log::warn!("read_tutorial_file io {}: {e}", validated.display());
        wire_io_error(&e)
    })
}

/// Race-resistant read: open via `tokio::fs::File` (binds to inode at
/// open time, decouples subsequent reads from path-string TOCTOU),
/// then verify the inode is a regular file and within the size cap
/// before draining bytes.
async fn open_and_read(path: &Path) -> Result<String, IoError> {
    let mut file = tokio::fs::File::open(path).await?;
    let meta = file.metadata().await?;
    if !meta.is_file() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if meta.len() > READ_MAX_BYTES {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            format!("file exceeds size cap of {READ_MAX_BYTES} bytes"),
        ));
    }
    let mut buf = String::with_capacity(meta.len() as usize);
    file.read_to_string(&mut buf).await?;
    Ok(buf)
}

#[tauri::command]
pub async fn write_progress_file(
    workspace_root: String,
    task_id: String,
    payload: String,
) -> Result<(), String> {
    let target = progress_path_for(&workspace_root, &task_id).map_err(|e| {
        log::warn!("write_progress_file progress_path_for: {e}");
        wire_validation_error(&e)
    })?;
    let ws_canonical = workspace_canonical(&workspace_root).map_err(|e| {
        log::warn!("write_progress_file workspace_canonical: {e}");
        wire_validation_error(&e)
    })?;
    let parent = target
        .parent()
        .ok_or_else(|| "E_INVALID_PATH".to_string())?
        .to_path_buf();

    let _guard = PROGRESS_WRITE_LOCK.lock().await;

    tokio::fs::create_dir_all(&parent).await.map_err(|e| {
        log::warn!("write_progress_file create_dir_all {}: {e}", parent.display());
        wire_io_error(&e)
    })?;

    // Re-canonicalise the parent after create_dir_all so a race that
    // swapped any ancestor for an out-of-tree symlink between
    // progress_path_for validation and the actual write is caught
    // before bytes hit the disk.
    let parent_canonical = dunce::canonicalize(&parent).map_err(|e| {
        log::warn!("write_progress_file recheck parent {}: {e}", parent.display());
        wire_io_error(&e)
    })?;
    if !parent_canonical.starts_with(&ws_canonical) {
        log::warn!(
            "write_progress_file parent escaped workspace post create_dir_all: {} not under {}",
            parent_canonical.display(),
            ws_canonical.display()
        );
        return Err("E_INVALID_PATH".to_string());
    }

    let final_target = parent_canonical.join("progress.json");
    tokio::fs::write(&final_target, payload.as_bytes())
        .await
        .map_err(|e| {
            log::warn!(
                "write_progress_file write {}: {e}",
                final_target.display()
            );
            wire_io_error(&e)
        })
}

#[tauri::command]
pub async fn list_tutorial_tasks(
    workspace_root: String,
) -> Result<Vec<TutorialTaskMeta>, String> {
    let ws = workspace_root.clone();
    tokio::task::spawn_blocking(move || list_tutorial_tasks_in(&ws))
        .await
        .map_err(|e| {
            log::warn!("list_tutorial_tasks join error: {e}");
            "E_IO".to_string()
        })?
        .map_err(|e| {
            log::warn!("list_tutorial_tasks: {e}");
            wire_validation_error(&e)
        })
}
