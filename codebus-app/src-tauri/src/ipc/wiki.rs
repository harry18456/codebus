//! Wiki read IPC commands.
//!
//! Spec touchpoints (v3-app-workspace-goal):
//! - `app-workspace § Tauri IPC Commands for Goal Lifecycle and Wiki Read`
//!   (`list_wiki_pages` + `read_wiki_page` entries)
//! - `app-workspace § Wikilink Resolution and Click Behavior`
//!   (client-side resolution → `list_wiki_pages` loads the index at
//!   workspace mount, `read_wiki_page` only fires when navigating to a
//!   resolvable target).
//!
//! Surface:
//! - `list_wiki_pages(vault_path)` — glob `<vault>/.codebus/wiki/**/*.md`,
//!   project to `WikiPageMeta { slug, path, title }`.
//! - `read_wiki_page(vault_path, page_slug)` — read the file body and
//!   strip the leading frontmatter block.
//!
//! Frontmatter title parse strategy: tolerant. If the file has no
//! frontmatter or the YAML cannot be parsed, the slug doubles as the
//! title so the file tree still renders.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::IpcResult;
use crate::error::AppError;

/// One row in the wiki page index. `slug` is the filename without the
/// `.md` extension; `path` is absolute; `title` is the frontmatter
/// title with the slug as a fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageMeta {
    pub slug: String,
    pub path: String,
    pub title: String,
}

// ---- list_wiki_pages ------------------------------------------------------

#[tauri::command]
pub async fn list_wiki_pages(vault_path: String) -> IpcResult<Vec<WikiPageMeta>> {
    let wiki_root = Path::new(&vault_path).join(".codebus").join("wiki");
    list_wiki_pages_impl(&wiki_root)
}

pub(crate) fn list_wiki_pages_impl(wiki_root: &Path) -> Result<Vec<WikiPageMeta>, AppError> {
    if !wiki_root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    walk_md_files(wiki_root, &mut |path| {
        let slug = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => return,
        };
        let body = match fs::read_to_string(path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let title = parse_title(&body).unwrap_or_else(|| slug.clone());
        out.push(WikiPageMeta {
            slug,
            path: path.display().to_string(),
            title,
        });
    })?;
    Ok(out)
}

/// Recurse `dir`, invoking `visit(file)` for each `*.md` file found.
/// Errors from individual `fs::read_dir` calls (permission denied on a
/// subfolder, etc.) are tolerated — we surface a top-level error only
/// when the root itself cannot be read.
fn walk_md_files(dir: &Path, visit: &mut dyn FnMut(&Path)) -> Result<(), AppError> {
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Tolerate subdirectory failures so a single unreadable
            // taxonomy folder doesn't sink the whole listing.
            let _ = walk_md_files(&path, visit);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            visit(&path);
        }
    }
    Ok(())
}

/// Extract `title` from the leading YAML frontmatter block. Returns
/// `None` when:
/// - the file does not start with `---\n` / `---\r\n`
/// - the YAML cannot be parsed
/// - the `title` key is missing or non-string
fn parse_title(content: &str) -> Option<String> {
    let after_open = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))?;
    let mut yaml_text = String::new();
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            break;
        }
        yaml_text.push_str(line);
    }
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_text).ok()?;
    parsed
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---- read_wiki_page -------------------------------------------------------

#[tauri::command]
pub async fn read_wiki_page(vault_path: String, page_slug: String) -> IpcResult<String> {
    let wiki_root = Path::new(&vault_path).join(".codebus").join("wiki");
    read_wiki_page_impl(&wiki_root, &page_slug)
}

pub(crate) fn read_wiki_page_impl(
    wiki_root: &Path,
    page_slug: &str,
) -> Result<String, AppError> {
    let path = find_page_by_slug(wiki_root, page_slug).ok_or_else(|| AppError::Invalid {
        field: "page_slug".into(),
        message: "no such page".into(),
    })?;
    let body = fs::read_to_string(&path)?;
    Ok(strip_frontmatter(&body).to_string())
}

fn find_page_by_slug(wiki_root: &Path, page_slug: &str) -> Option<PathBuf> {
    let mut found: Option<PathBuf> = None;
    let _ = walk_md_files(wiki_root, &mut |path| {
        if found.is_some() {
            return;
        }
        if path.file_stem().and_then(|s| s.to_str()) == Some(page_slug) {
            found = Some(path.to_path_buf());
        }
    });
    found
}

/// Strip a leading `---\n...\n---\n` frontmatter block. When there is
/// no opening delimiter or no closing delimiter, the original content
/// is returned unchanged so the Milkdown preview still has something
/// to render.
fn strip_frontmatter(content: &str) -> &str {
    let rest = match content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    {
        Some(r) => r,
        None => return content,
    };
    let mut idx = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            let end = idx + line.len();
            return &rest[end..];
        }
        idx += line.len();
    }
    content
}

// ---- tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_file(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    /// Task 3.6 acceptance: list_wiki_pages extracts the frontmatter
    /// title for files that have one; falls back to slug otherwise.
    #[test]
    fn list_wiki_pages_extracts_frontmatter_title() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().to_path_buf();

        write_file(
            &wiki_root,
            "modules/uv-lib.md",
            "---\ntitle: UV library entry\ntype: module\n---\n\n# uv-lib\nbody\n",
        );
        write_file(
            &wiki_root,
            "modules/uv-child.md",
            "---\ntitle: 'UV child resolver'\n---\n\nbody\n",
        );
        write_file(&wiki_root, "modules/raw.md", "# no frontmatter here\n");

        let pages = list_wiki_pages_impl(&wiki_root).unwrap();
        assert_eq!(pages.len(), 3, "expected 3 pages: {pages:?}");
        let by_slug: std::collections::HashMap<&str, &WikiPageMeta> =
            pages.iter().map(|p| (p.slug.as_str(), p)).collect();
        assert_eq!(by_slug["uv-lib"].title, "UV library entry");
        assert_eq!(by_slug["uv-child"].title, "UV child resolver");
        assert_eq!(
            by_slug["raw"].title, "raw",
            "no frontmatter → title falls back to slug"
        );
    }

    /// Task 3.7 acceptance: read_wiki_page strips leading frontmatter.
    #[test]
    fn read_wiki_page_strips_frontmatter() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().to_path_buf();

        let body = "# uv-lib\n\nLibrary entry point for `uv`.\n";
        write_file(
            &wiki_root,
            "modules/uv-lib.md",
            &format!("---\ntitle: UV\ntype: module\n---\n{body}"),
        );

        let returned = read_wiki_page_impl(&wiki_root, "uv-lib").unwrap();
        assert!(
            !returned.starts_with("---"),
            "frontmatter must be stripped, got: {returned:?}"
        );
        assert!(returned.contains("Library entry point"));
    }

    /// Task 3.7 acceptance follow-up: unknown slug → AppError::Invalid.
    #[test]
    fn read_wiki_page_returns_invalid_for_unknown_slug() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = read_wiki_page_impl(tmp.path(), "no-such-page").expect_err("must fail");
        match err {
            AppError::Invalid { field, message } => {
                assert_eq!(field, "page_slug");
                assert!(message.contains("no such page"));
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn strip_frontmatter_handles_crlf() {
        let s = "---\r\ntitle: x\r\n---\r\nbody\r\n";
        assert_eq!(strip_frontmatter(s), "body\r\n");
    }

    #[test]
    fn strip_frontmatter_passes_through_when_no_delim() {
        let s = "# heading\nbody\n";
        assert_eq!(strip_frontmatter(s), s);
    }

    #[test]
    fn list_wiki_pages_returns_empty_for_missing_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pages = list_wiki_pages_impl(&tmp.path().join("missing")).unwrap();
        assert!(pages.is_empty());
    }
}
