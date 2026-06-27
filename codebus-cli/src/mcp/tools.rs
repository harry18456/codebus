//! Pure query logic behind the MCP wiki tools — pagination, keyword search,
//! and slug→path resolution. Kept free of any rmcp types so the behavior is
//! unit-testable without a live server; `server.rs` wraps these in tool
//! handlers and serializes the result structs to JSON text content.

use std::fs;
use std::path::{Path, PathBuf};

use codebus_core::wiki::read::{find_page_by_slug, list_pages, strip_frontmatter};
use serde::Serialize;

/// Default character budget for a single `wiki_read` page slice.
pub const DEFAULT_READ_LIMIT: usize = 12_000;
/// Hard ceiling for a `wiki_read` slice. CJK worst case is ~1 token/char, so
/// 20000 chars stays under a ~25k-token single-tool-output budget with margin.
pub const MAX_READ_LIMIT: usize = 20_000;
/// Max pages returned by one `wiki_search` call before flagging `truncated`.
pub const SEARCH_RESULT_CAP: usize = 20;
/// Characters of context kept on each side of a search match in a snippet.
const SNIPPET_RADIUS: usize = 100;

/// One character-paginated slice of a page body (frontmatter already stripped).
#[derive(Debug, Serialize, PartialEq)]
pub struct ReadSlice {
    pub content: String,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub has_more: bool,
    pub total_chars: usize,
}

/// One `wiki_search` hit.
#[derive(Debug, Serialize, PartialEq)]
pub struct SearchHit {
    pub slug: String,
    pub title: String,
    pub snippet: String,
}

/// Result of a `wiki_search`: the capped hit list plus whether more pages
/// matched than were returned.
#[derive(Debug, Serialize, PartialEq)]
pub struct SearchOutcome {
    pub results: Vec<SearchHit>,
    pub truncated: bool,
}

/// Slice `body` by Unicode character (never by byte, so multi-byte UTF-8 / CJK
/// characters are never split). `limit` is clamped to `[1, MAX_READ_LIMIT]`;
/// `offset` past the end yields empty content with `has_more == false`.
pub fn paginate(body: &str, offset: usize, limit: usize) -> ReadSlice {
    let chars: Vec<char> = body.chars().collect();
    let total = chars.len();
    let limit = limit.clamp(1, MAX_READ_LIMIT);
    let start = offset.min(total);
    let end = (start + limit).min(total);
    let content: String = chars[start..end].iter().collect();
    let has_more = end < total;
    ReadSlice {
        content,
        offset: start,
        next_offset: if has_more { Some(end) } else { None },
        has_more,
        total_chars: total,
    }
}

/// Resolve `slug` to a page path under `wiki_root`, returning `None` for an
/// unknown slug. Resolution is by filename stem (see
/// [`find_page_by_slug`]); as defense-in-depth the canonical resolved path is
/// verified to stay within the canonical wiki root, so a slug bearing `..`
/// or separators can never escape the subtree to reach `raw/code/`.
pub fn resolve_page_path(wiki_root: &Path, slug: &str) -> Option<PathBuf> {
    let path = find_page_by_slug(wiki_root, slug)?;
    let canon_root = wiki_root.canonicalize().ok()?;
    let canon_path = path.canonicalize().ok()?;
    canon_path.starts_with(&canon_root).then_some(path)
}

/// Case-insensitive substring search of `query` over each page's title and
/// stripped body, treating `query` as a single needle (no tokenization).
/// Returns at most [`SEARCH_RESULT_CAP`] hits; `truncated` is set when more
/// pages matched. An empty/whitespace `query` is rejected by the caller, not
/// here. A no-match is an empty (non-truncated) outcome, not an error.
pub fn search_pages(wiki_root: &Path, query: &str) -> std::io::Result<SearchOutcome> {
    let needle = query.to_lowercase();
    let pages = list_pages(wiki_root)?;
    let mut results = Vec::new();
    let mut truncated = false;
    for page in pages {
        let body = match fs::read_to_string(&page.path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let stripped = strip_frontmatter(&body);
        let title_matches = page.title.to_lowercase().contains(&needle);
        let body_matches = stripped.to_lowercase().contains(&needle);
        if !(title_matches || body_matches) {
            continue;
        }
        if results.len() >= SEARCH_RESULT_CAP {
            truncated = true;
            break;
        }
        // Prefer a body snippet around the match; fall back to the body head
        // when the hit was title-only.
        let snippet = make_snippet(stripped, &needle);
        results.push(SearchHit {
            slug: page.slug,
            title: page.title,
            snippet,
        });
    }
    Ok(SearchOutcome { results, truncated })
}

/// Build a context snippet around the first occurrence of `needle_lower`
/// (already lowercased) in `body`. Slicing is by character; an ellipsis marks
/// each truncated edge. A title-only match (needle absent from the body)
/// yields the body head.
fn make_snippet(body: &str, needle_lower: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let body_lower = body.to_lowercase();
    let needle_chars = needle_lower.chars().count();
    let (start, end) = match body_lower.find(needle_lower) {
        Some(byte_idx) => {
            let pos = body_lower[..byte_idx].chars().count();
            let start = pos.saturating_sub(SNIPPET_RADIUS);
            let end = (pos + needle_chars + SNIPPET_RADIUS).min(chars.len());
            (start, end)
        }
        None => (0, (SNIPPET_RADIUS * 2).min(chars.len())),
    };
    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.extend(&chars[start..end]);
    if end < chars.len() {
        snippet.push('…');
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_page(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
    }

    /// Spec `mcp-server` § wiki_read "Example: character pagination boundaries".
    #[test]
    fn paginate_matches_spec_boundary_table() {
        // total_chars, offset, limit -> (content_len, has_more, next_offset)
        let cases = [
            (30000usize, 0usize, 12000usize, 12000usize, true, Some(12000usize)),
            (30000, 12000, 12000, 12000, true, Some(24000)),
            (30000, 24000, 12000, 6000, false, None),
            (30000, 0, 99999, 20000, true, Some(20000)), // limit clamped to 20000
            (500, 0, 12000, 500, false, None),
        ];
        for (total, offset, limit, exp_len, exp_more, exp_next) in cases {
            let body: String = std::iter::repeat('a').take(total).collect();
            let slice = paginate(&body, offset, limit);
            assert_eq!(slice.content.chars().count(), exp_len, "len for {total}/{offset}/{limit}");
            assert_eq!(slice.has_more, exp_more, "has_more for {total}/{offset}/{limit}");
            assert_eq!(slice.next_offset, exp_next, "next for {total}/{offset}/{limit}");
            assert_eq!(slice.total_chars, total);
        }
    }

    #[test]
    fn paginate_never_splits_cjk_characters() {
        // 10 CJK chars (each 3 bytes in UTF-8). A byte-based slice at offset 5
        // would land mid-character; the char-based slice must stay clean.
        let body = "授權流程模組設定頁面"; // 10 chars: 授權流程模組設定頁面
        let slice = paginate(body, 2, 4);
        assert_eq!(slice.content, "流程模組"); // chars[2..6]
        assert_eq!(slice.offset, 2);
        assert_eq!(slice.total_chars, 10);
        assert!(slice.has_more);
        assert_eq!(slice.next_offset, Some(6));
    }

    #[test]
    fn paginate_offset_past_end_is_empty_and_done() {
        let slice = paginate("short", 100, 50);
        assert_eq!(slice.content, "");
        assert!(!slice.has_more);
        assert_eq!(slice.next_offset, None);
        assert_eq!(slice.total_chars, 5);
    }

    #[test]
    fn search_matches_case_insensitively_with_snippet() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        write_page(root, "auth.md", "---\ntitle: Auth\n---\nThe AUTHentication flow validates tokens.\n");
        write_page(root, "other.md", "---\ntitle: Other\n---\nUnrelated content here.\n");

        let out = search_pages(root, "authentication").unwrap();
        assert_eq!(out.results.len(), 1, "exactly one body match: {:?}", out.results);
        assert!(!out.truncated);
        assert_eq!(out.results[0].slug, "auth");
        assert!(
            out.results[0].snippet.to_lowercase().contains("authentication"),
            "snippet should contain the match: {}",
            out.results[0].snippet
        );
    }

    #[test]
    fn search_no_match_is_empty_not_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_page(tmp.path(), "a.md", "---\ntitle: A\n---\nhello world\n");
        let out = search_pages(tmp.path(), "zzznomatch").unwrap();
        assert!(out.results.is_empty());
        assert!(!out.truncated);
    }

    #[test]
    fn search_caps_results_and_flags_truncated() {
        let tmp = tempfile::TempDir::new().unwrap();
        // 25 pages all containing the needle → capped at SEARCH_RESULT_CAP=20.
        for i in 0..25 {
            write_page(
                tmp.path(),
                &format!("p{i:02}.md"),
                "---\ntitle: P\n---\nthe needle is here\n",
            );
        }
        let out = search_pages(tmp.path(), "needle").unwrap();
        assert_eq!(out.results.len(), SEARCH_RESULT_CAP);
        assert!(out.truncated, "more than cap matched → truncated");
    }

    #[test]
    fn resolve_page_path_resolves_known_rejects_unknown() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_page(tmp.path(), "modules/uv-lib.md", "# uv\n");
        assert!(resolve_page_path(tmp.path(), "uv-lib").is_some());
        assert!(resolve_page_path(tmp.path(), "no-such").is_none());
    }

    #[test]
    fn resolve_page_path_traversal_slug_cannot_escape() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki = tmp.path().join("wiki");
        write_page(&wiki, "page.md", "# page\n");
        // A secret file outside the wiki subtree (mimics raw/code/).
        write_page(tmp.path(), "raw/code/secret.md", "SECRET\n");
        // Stem-based resolution can only ever match pages inside `wiki`.
        assert!(resolve_page_path(&wiki, "../raw/code/secret").is_none());
        assert!(resolve_page_path(&wiki, "secret").is_none());
        assert!(resolve_page_path(&wiki, "page").is_some());
    }
}
