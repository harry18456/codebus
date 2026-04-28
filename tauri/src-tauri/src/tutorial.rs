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
//! allowlist, and symlink-outside rejection. Logic is split between a
//! sync `validate_path` helper (testable without a Tauri runtime) and
//! thin async command wrappers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Subtree of every workspace where Module 5 Generator writes tutorials.
const TUTORIALS_SUBDIR: &str = "codebus-tutorials";

/// Allowlist of extensions readable by `read_tutorial_file`. Anything
/// outside this set is rejected even if the path otherwise validates,
/// so a misrouted `.ssh/id_rsa`-style read can never succeed.
const ALLOWED_EXTENSIONS: &[&str] = &["md", "json"];

/// Permissive task_id regex covering Module 5 Generator output
/// (`generate_<8 hex>`) plus future runners. Disallows path-injection
/// characters (`.`, `/`, whitespace).
fn is_valid_task_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 80 {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Metadata returned by `list_tutorial_tasks`. The frontend uses
/// `frontmatter_raw` (parsed by `gray-matter`) to read `generated_at`;
/// `dir_mtime_unix` is the mtime fallback when frontmatter is missing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialTaskMeta {
    pub id: String,
    pub frontmatter_raw: Option<String>,
    pub dir_mtime_unix: i64,
}

/// Validate `workspace_root` + `relative_path` for read access. Returns
/// the canonical absolute path inside the workspace on success.
///
/// Defends against:
/// - non-absolute / non-existent / non-directory workspace
/// - `relative_path` not under `codebus-tutorials/`
/// - `..` traversal after canonicalisation
/// - symlinks resolving outside the workspace
/// - extensions outside the read allowlist
pub fn validate_path(
    workspace_root: &str,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let ws = Path::new(workspace_root);
    if !ws.is_absolute() {
        return Err(format!(
            "workspace_root must be absolute: {workspace_root}"
        ));
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
    if rel_normalised.split('/').any(|seg| seg == "..") {
        return Err(format!("relative_path contains '..': {relative_path}"));
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
            // resolution, but still verify each existing ancestor doesn't
            // symlink outside the workspace.
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

/// Validate `workspace_root` for write access (used by
/// `write_progress_file`). Same rules as `validate_path` but skips the
/// relative_path / extension layer (we build the path internally).
fn validate_workspace_for_write(workspace_root: &str) -> Result<PathBuf, String> {
    let ws = Path::new(workspace_root);
    if !ws.is_absolute() {
        return Err(format!(
            "workspace_root must be absolute: {workspace_root}"
        ));
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

/// Compute the absolute progress.json path for a given workspace + task.
/// Validates `task_id` for path-injection safety before joining.
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

/// Sync helper used by both the `list_tutorial_tasks` command and the
/// integration tests. Returns an empty Vec when `codebus-tutorials/` is
/// absent or empty (the empty-CTA branch in D-T13).
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

    // Sort by id so output order is deterministic across platforms.
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

/// Read the YAML frontmatter block from a markdown file (between two
/// `---` lines at the top). Returns the raw block contents (without the
/// fences) so the frontend can parse with `gray-matter`. None when no
/// frontmatter is present or the file is unreadable.
fn read_frontmatter_block(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let first = lines.next()?;
    if first.trim() != "---" {
        return None;
    }
    let mut buf = String::new();
    for line in lines {
        if line.trim() == "---" {
            return Some(buf);
        }
        buf.push_str(line);
        buf.push('\n');
    }
    None
}

/// Process-wide lock that serialises `write_progress_file` writes so a
/// torn write across multiple windows is impossible.
pub static PROGRESS_WRITE_LOCK: Mutex<()> = Mutex::const_new(());

#[tauri::command]
pub async fn read_tutorial_file(
    workspace_root: String,
    relative_path: String,
) -> Result<String, String> {
    let validated = validate_path(&workspace_root, &relative_path)?;
    tokio::fs::read_to_string(&validated)
        .await
        .map_err(|e| format!("read_tutorial_file failed: {e}"))
}

#[tauri::command]
pub async fn write_progress_file(
    workspace_root: String,
    task_id: String,
    payload: String,
) -> Result<(), String> {
    let target = progress_path_for(&workspace_root, &task_id)?;
    let parent = target
        .parent()
        .ok_or_else(|| format!("progress path has no parent: {}", target.display()))?
        .to_path_buf();

    let _guard = PROGRESS_WRITE_LOCK.lock().await;
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(|e| format!("create_dir_all parent failed: {e}"))?;
    tokio::fs::write(&target, payload.as_bytes())
        .await
        .map_err(|e| format!("write_progress_file failed: {e}"))
}

#[tauri::command]
pub async fn list_tutorial_tasks(
    workspace_root: String,
) -> Result<Vec<TutorialTaskMeta>, String> {
    let ws = workspace_root.clone();
    tokio::task::spawn_blocking(move || list_tutorial_tasks_in(&ws))
        .await
        .map_err(|e| format!("list_tutorial_tasks join error: {e}"))?
}
