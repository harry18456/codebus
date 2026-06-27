//! Tolerant wiki page reading — listing, slug resolution, frontmatter
//! stripping, and title projection.
//!
//! Extracted from codebus-app's `ipc/wiki.rs` so the MCP server
//! (codebus-cli) and the Tauri wiki commands share one implementation.
//!
//! This is the **tolerant** reader: a page with missing or malformed
//! frontmatter still lists (slug doubles as title). It is deliberately
//! separate from the **strict** [`crate::wiki::frontmatter::parse_page`]
//! validator used by lint, which rejects any page missing a required field.
//! Reusing the strict parser here would drop frontmatter-less pages from the
//! index — the opposite of what listing/search want.
//!
//! Error model: [`find_page_by_slug`] returns `Option` (an unknown slug is
//! `None`, NOT an error), so callers can map a missing page to their own
//! domain error (`AppError::Invalid`, MCP `ErrorData::invalid_params`) while
//! a genuine I/O failure on a resolved file surfaces as `io::Error`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One row in the wiki page index. `slug` is the filename without the `.md`
/// extension; `path` is absolute; `title` is the frontmatter title with the
/// slug as a fallback. `goals` and `updated` are projected from the leading
/// YAML frontmatter; both default to empty when frontmatter is missing or
/// malformed so a page without frontmatter still appears in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageMeta {
    pub slug: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub updated: String,
}

/// List every `*.md` page under `wiki_root`, projecting each to a
/// [`WikiPageMeta`]. A missing `wiki_root` yields an empty list (not an
/// error). Individual files that cannot be read are skipped; a failure to
/// read `wiki_root` itself surfaces as `io::Error`.
pub fn list_pages(wiki_root: &Path) -> io::Result<Vec<WikiPageMeta>> {
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
        let yaml = parse_frontmatter_yaml(&body);
        let title = yaml
            .as_ref()
            .and_then(|y| y.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| slug.clone());
        let goals = yaml
            .as_ref()
            .and_then(|y| y.get("goals"))
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|entry| entry.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let updated = yaml
            .as_ref()
            .and_then(|y| y.get("updated"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        out.push(WikiPageMeta {
            slug,
            path: path.display().to_string(),
            title,
            goals,
            updated,
        });
    })?;
    Ok(out)
}

/// Resolve a page by `slug` (filename stem). Returns the first `*.md` file
/// under `wiki_root` whose stem equals `slug`, or `None` if no page matches.
///
/// Resolution is by stem comparison, NOT path joining: a `slug` containing
/// `..` or path separators can only ever match a page already inside
/// `wiki_root`, so it cannot escape the subtree. An unknown slug is `None`,
/// not an error — callers map that to their own "no such page" failure.
pub fn find_page_by_slug(wiki_root: &Path, slug: &str) -> Option<PathBuf> {
    let mut found: Option<PathBuf> = None;
    let _ = walk_md_files(wiki_root, &mut |path| {
        if found.is_some() {
            return;
        }
        if path.file_stem().and_then(|s| s.to_str()) == Some(slug) {
            found = Some(path.to_path_buf());
        }
    });
    found
}

/// Project the frontmatter `title` from page `content`, falling back to
/// `slug` when frontmatter is absent, unparseable, or has no `title` key.
pub fn frontmatter_title(content: &str, slug: &str) -> String {
    parse_frontmatter_yaml(content)
        .as_ref()
        .and_then(|y| y.get("title"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| slug.to_string())
}

/// Strip a leading `---\n...\n---\n` frontmatter block. When there is no
/// opening delimiter or no closing delimiter, the original content is
/// returned unchanged so the body still has something to render.
pub fn strip_frontmatter(content: &str) -> &str {
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

/// Recurse `dir`, invoking `visit(file)` for each `*.md` file found. Errors
/// from individual `fs::read_dir` calls on subfolders are tolerated — a
/// top-level error surfaces only when `dir` itself cannot be read.
fn walk_md_files(dir: &Path, visit: &mut dyn FnMut(&Path)) -> io::Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Tolerate subdirectory failures so a single unreadable taxonomy
            // folder doesn't sink the whole listing.
            let _ = walk_md_files(&path, visit);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            visit(&path);
        }
    }
    Ok(())
}

/// Parse the leading YAML frontmatter block into a `serde_yaml::Value`.
/// Returns `None` when the file does not start with `---`, the closing
/// delimiter is missing, or the YAML cannot be parsed. Callers project
/// individual keys (`title`, `goals`, `updated`, ...) off the returned value;
/// missing keys are tolerated so a page without frontmatter still renders.
fn parse_frontmatter_yaml(content: &str) -> Option<serde_yaml::Value> {
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
    serde_yaml::from_str(&yaml_text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn list_pages_projects_goals_and_updated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().to_path_buf();

        write_file(
            &wiki_root,
            "modules/auth-middleware.md",
            "---\ntitle: Auth Middleware\ntype: module\ngoals:\n  - g-first\n  - g-second\nupdated: '2026-05-27T11:00:00Z'\n---\n\nbody\n",
        );
        write_file(
            &wiki_root,
            "modules/no-goals.md",
            "---\ntitle: Standalone\ntype: module\nupdated: '2026-05-26T08:00:00Z'\n---\n\nbody\n",
        );
        write_file(&wiki_root, "modules/raw.md", "# no frontmatter\n");

        let pages = list_pages(&wiki_root).unwrap();
        let by_slug: std::collections::HashMap<&str, &WikiPageMeta> =
            pages.iter().map(|p| (p.slug.as_str(), p)).collect();

        let auth = by_slug["auth-middleware"];
        assert_eq!(
            auth.goals,
            vec!["g-first".to_string(), "g-second".to_string()]
        );
        assert_eq!(auth.updated, "2026-05-27T11:00:00Z");

        let solo = by_slug["no-goals"];
        assert!(solo.goals.is_empty(), "missing goals → empty Vec");
        assert_eq!(solo.updated, "2026-05-26T08:00:00Z");

        let raw = by_slug["raw"];
        assert!(raw.goals.is_empty(), "no frontmatter → empty Vec");
        assert_eq!(raw.updated, "", "no frontmatter → empty string");
    }

    #[test]
    fn list_pages_extracts_frontmatter_title_with_slug_fallback() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().to_path_buf();

        write_file(
            &wiki_root,
            "modules/uv-lib.md",
            "---\ntitle: UV library entry\ntype: module\n---\n\n# uv-lib\nbody\n",
        );
        write_file(&wiki_root, "modules/raw.md", "# no frontmatter here\n");

        let pages = list_pages(&wiki_root).unwrap();
        assert_eq!(pages.len(), 2, "expected 2 pages: {pages:?}");
        let by_slug: std::collections::HashMap<&str, &WikiPageMeta> =
            pages.iter().map(|p| (p.slug.as_str(), p)).collect();
        assert_eq!(by_slug["uv-lib"].title, "UV library entry");
        assert_eq!(
            by_slug["raw"].title, "raw",
            "no frontmatter → title falls back to slug"
        );
    }

    #[test]
    fn list_pages_returns_empty_for_missing_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pages = list_pages(&tmp.path().join("missing")).unwrap();
        assert!(pages.is_empty());
    }

    #[test]
    fn find_page_by_slug_resolves_known_and_rejects_unknown() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().to_path_buf();
        write_file(&wiki_root, "modules/uv-lib.md", "# uv-lib\n");

        let found = find_page_by_slug(&wiki_root, "uv-lib");
        assert!(found.is_some(), "known slug must resolve");
        assert!(found.unwrap().ends_with("uv-lib.md"));

        assert!(
            find_page_by_slug(&wiki_root, "no-such-page").is_none(),
            "unknown slug → None (not an error)"
        );
    }

    #[test]
    fn frontmatter_title_falls_back_to_slug() {
        assert_eq!(
            frontmatter_title("---\ntitle: Real Title\n---\nbody\n", "the-slug"),
            "Real Title"
        );
        assert_eq!(
            frontmatter_title("# no frontmatter\n", "the-slug"),
            "the-slug",
            "no frontmatter → slug fallback"
        );
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
}
