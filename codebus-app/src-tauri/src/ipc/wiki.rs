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

use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use codebus_core::vault::obsidian_register::{
    lookup_vault_id_at, obsidian_json_path, register_at,
};
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

// ---- Open in Obsidian -----------------------------------------------------
//
// Spec touchpoints (wiki-open-in-obsidian):
// - `app-workspace § Open Wiki Page In Obsidian` (`get_obsidian_vault_id`
//   probe + `open_wiki_in_obsidian` action + the URL-builder contract).
//
// The vault id is the 16-char SHA-256 prefix Obsidian uses as its vault key,
// resolved via `codebus_core::vault::obsidian_register` — the same source the
// CLI lint OSC 8 hyperlinks use, so the app and the terminal target identical
// `obsidian://open?vault=<id>&file=<rel>` URLs.

/// Resolve the Obsidian vault id registered for `<vault>/.codebus/wiki`.
///
/// Probe half of the design's two-command split — the frontend caches the
/// result and only renders `[Open in Obsidian]` when it is `Some`. `Err`
/// (obsidian.json present but unreadable / unparseable) is fail-soft: the
/// frontend treats it identically to `None`.
#[tauri::command]
pub async fn get_obsidian_vault_id(vault_path: String) -> IpcResult<Option<String>> {
    let wiki_root = Path::new(&vault_path).join(".codebus").join("wiki");
    register_and_resolve_vault_id(&wiki_root, obsidian_json_path().as_deref())
}

/// Open the wiki page identified by `slug` in Obsidian.
///
/// Action half of the split. Re-resolves the vault id on every call rather
/// than trusting a frontend-supplied cached id, so a vault that becomes
/// unregistered while the app is open is caught here (design Decision
/// "action command 重新解析 id").
#[tauri::command]
pub async fn open_wiki_in_obsidian(vault_path: String, slug: String) -> IpcResult<()> {
    let wiki_root = Path::new(&vault_path).join(".codebus").join("wiki");
    let url = resolve_obsidian_url(&wiki_root, obsidian_json_path().as_deref(), &slug)?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(AppError::internal)
}

/// Probe body of [`get_obsidian_vault_id`]: ensure the vault is registered in
/// Obsidian, then resolve its id. Because codebus-app creates/binds vaults
/// without init-time Obsidian registration, this is the universal touchpoint
/// that registers any vault the user views (new OR pre-existing) so the
/// `[Open in Obsidian]` button works.
///
/// Registration is idempotent (re-registering only refreshes the timestamp)
/// and fail-soft: a `RegisterOutcome::ObsidianNotInstalled` (config dir
/// absent → `json_path` is `None`) writes nothing, and an `IoError` is
/// swallowed — either way the subsequent [`resolve_vault_id`] reports the
/// resulting state (`None` → button hidden). This wrapper deliberately does
/// NOT live in [`resolve_vault_id`]: the `open_wiki_in_obsidian` action reuses
/// the pure-lookup `resolve_vault_id` so it still rejects an unregistered
/// vault rather than silently registering at click time.
fn register_and_resolve_vault_id(
    wiki_root: &Path,
    json_path: Option<&Path>,
) -> Result<Option<String>, AppError> {
    if let Some(p) = json_path {
        // Idempotent + fail-soft: ignore the outcome; resolve_vault_id below
        // reports the resulting registration state.
        let _ = register_at(wiki_root, p);
    }
    resolve_vault_id(wiki_root, json_path)
}

/// Pure-lookup resolver. `json_path` is `None` when Obsidian's config dir is
/// absent, mirroring `lookup_vault_id`'s own "no config dir → Ok(None)"
/// semantics. An `Err` from the core helper (parse failure) maps to
/// `AppError`. Used by both the probe (via
/// [`register_and_resolve_vault_id`]) and the `open_wiki_in_obsidian` action
/// (which relies on the pure-lookup behavior to reject unregistered vaults).
fn resolve_vault_id(
    wiki_root: &Path,
    json_path: Option<&Path>,
) -> Result<Option<String>, AppError> {
    match json_path {
        Some(p) => lookup_vault_id_at(wiki_root, p).map_err(AppError::from),
        None => Ok(None),
    }
}

/// Steps 1–4 of `open_wiki_in_obsidian` (everything except the actual
/// `open_url` spawn) factored out so the URL can be asserted in tests
/// without launching Obsidian.
fn resolve_obsidian_url(
    wiki_root: &Path,
    json_path: Option<&Path>,
    slug: &str,
) -> Result<String, AppError> {
    let vault_id = resolve_vault_id(wiki_root, json_path)?.ok_or_else(|| AppError::Invalid {
        field: "obsidian".into(),
        message: "vault not registered in Obsidian".into(),
    })?;
    let abs_page = find_page_by_slug(wiki_root, slug).ok_or_else(|| AppError::Invalid {
        field: "slug".into(),
        message: "no such wiki page".into(),
    })?;
    build_obsidian_url(&vault_id, wiki_root, &abs_page).ok_or_else(|| AppError::Invalid {
        field: "slug".into(),
        message: "could not build obsidian url".into(),
    })
}

/// Build `obsidian://open?vault=<id>&file=<rel>` where `<rel>` is `abs_page`
/// relative to `wiki_root`, separators normalized to `/`, each segment
/// percent-encoded. Returns `None` when `abs_page` is not under `wiki_root`,
/// resolves to an empty relative path, or contains a non-`Normal` component
/// (`..` / a root) that has no place in a vault-relative wiki path.
fn build_obsidian_url(vault_id: &str, wiki_root: &Path, abs_page: &Path) -> Option<String> {
    let rel = abs_page.strip_prefix(wiki_root).ok()?;
    let mut segments = Vec::new();
    for comp in rel.components() {
        match comp {
            Component::Normal(os) => segments.push(percent_encode_segment(os.to_str()?)),
            _ => return None,
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(format!(
        "obsidian://open?vault={vault_id}&file={}",
        segments.join("/")
    ))
}

/// Percent-encode one path segment per RFC 3986 unreserved set
/// (`A-Za-z0-9-._~` kept; everything else `%XX` per UTF-8 byte). `/` never
/// appears inside a segment, so it is not special-cased here (segments are
/// joined with a literal `/` afterward). Matches the encoder behind the CLI
/// lint OSC 8 URLs so both targets agree byte-for-byte.
fn percent_encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
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

    // ---- Open in Obsidian (wiki-open-in-obsidian) ----

    /// Task 1.1 / spec `Example: relative path + encoding cases` — the four
    /// table rows mapping `(slug, abs wiki path) -> file=` value.
    #[test]
    fn build_obsidian_url_matches_spec_examples() {
        let wiki_root = Path::new("/v/.codebus/wiki");
        let vault_id = "abc123def456abcd";
        let cases = [
            ("modules/uv-lib.md", "modules/uv-lib.md"),
            ("concepts/project-purpose.md", "concepts/project-purpose.md"),
            ("index.md", "index.md"),
            (
                "processes/授權流程.md",
                "processes/%E6%8E%88%E6%AC%8A%E6%B5%81%E7%A8%8B.md",
            ),
        ];
        for (rel, expected_file) in cases {
            let abs = wiki_root.join(rel);
            let url = build_obsidian_url(vault_id, wiki_root, &abs)
                .unwrap_or_else(|| panic!("expected Some for {rel}"));
            assert_eq!(
                url,
                format!("obsidian://open?vault={vault_id}&file={expected_file}"),
                "row {rel}"
            );
        }
    }

    /// Task 1.1 — `abs_page` outside `wiki_root` has no vault-relative form.
    #[test]
    fn build_obsidian_url_returns_none_outside_wiki_root() {
        let wiki_root = Path::new("/v/.codebus/wiki");
        let outside = Path::new("/v/other/page.md");
        assert!(build_obsidian_url("id", wiki_root, outside).is_none());
    }

    /// Helper: register `wiki` into a temp `obsidian.json` and return the id.
    fn register_temp_vault(wiki: &Path, json_path: &Path) -> String {
        use codebus_core::vault::obsidian_register::{register_at, RegisterOutcome};
        match register_at(wiki, json_path) {
            RegisterOutcome::Registered { vault_id, .. } => vault_id,
            other => panic!("setup register failed: {other:?}"),
        }
    }

    /// Task 1.2 — registered vault → `Ok(Some(id))`.
    #[test]
    fn get_obsidian_vault_id_returns_some_for_registered_vault() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();
        let id = register_temp_vault(&wiki_root, &json);

        let got = resolve_vault_id(&wiki_root, Some(&json)).unwrap();
        assert_eq!(got, Some(id));
    }

    /// Task 1.2 — no obsidian.json (config dir present, file absent) → `None`.
    #[test]
    fn get_obsidian_vault_id_returns_none_when_unregistered() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        let got = resolve_vault_id(&wiki_root, Some(&json)).unwrap();
        assert_eq!(got, None);
    }

    /// Task 1.2 — Obsidian config dir absent (`None` json path) → `Ok(None)`.
    #[test]
    fn get_obsidian_vault_id_returns_none_when_no_config_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        let got = resolve_vault_id(&wiki_root, None).unwrap();
        assert_eq!(got, None);
    }

    /// Task 1.2 — malformed obsidian.json → `Err(AppError)` (fail-soft).
    #[test]
    fn get_obsidian_vault_id_maps_parse_failure_to_app_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        fs::create_dir_all(json.parent().unwrap()).unwrap();
        fs::write(&json, b"not json {[").unwrap();
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        let err = resolve_vault_id(&wiki_root, Some(&json)).expect_err("must error");
        assert!(matches!(err, AppError::Io { .. }), "got {err:?}");
    }

    /// Task 1.1 — the probe registers a not-yet-registered vault and then
    /// resolves to Some; the entry is persisted to obsidian.json.
    #[test]
    fn probe_registers_unregistered_vault() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        fs::create_dir_all(json.parent().unwrap()).unwrap();
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        // Pure lookup sees no entry yet (action path would reject).
        assert_eq!(resolve_vault_id(&wiki_root, Some(&json)).unwrap(), None);

        // Probe registers, then resolves to Some.
        let got = register_and_resolve_vault_id(&wiki_root, Some(&json)).unwrap();
        assert!(got.is_some(), "probe must register then return Some");
        // Entry persisted: a subsequent pure lookup now finds the same id.
        assert_eq!(resolve_vault_id(&wiki_root, Some(&json)).unwrap(), got);
    }

    /// Task 1.1 — probe registration is idempotent: two calls return the same
    /// id and leave exactly one entry for the wiki path.
    #[test]
    fn probe_register_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        fs::create_dir_all(json.parent().unwrap()).unwrap();
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        let first = register_and_resolve_vault_id(&wiki_root, Some(&json)).unwrap();
        let second = register_and_resolve_vault_id(&wiki_root, Some(&json)).unwrap();
        assert!(first.is_some());
        assert_eq!(first, second);

        let body = fs::read_to_string(&json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let vaults = v.get("vaults").and_then(|m| m.as_object()).unwrap();
        assert_eq!(vaults.len(), 1, "idempotent: exactly one vault entry");
    }

    /// Task 1.3 — probe with no Obsidian config dir (None json path) returns
    /// Ok(None) and writes nothing (no regression for users without Obsidian).
    #[test]
    fn probe_returns_none_and_writes_nothing_when_no_config_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        fs::create_dir_all(&wiki_root).unwrap();

        let got = register_and_resolve_vault_id(&wiki_root, None).unwrap();
        assert_eq!(got, None);
        assert!(
            !tmp.path().join("obsidian/obsidian.json").exists(),
            "no obsidian.json may be created when config dir is absent"
        );
    }

    /// Task 1.3 — registered vault + real sub-folder slug → spec URL.
    #[test]
    fn open_wiki_in_obsidian_builds_url_for_valid_slug() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        write_file(&wiki_root, "modules/uv-lib.md", "# uv-lib\n");
        let id = register_temp_vault(&wiki_root, &json);

        let url = resolve_obsidian_url(&wiki_root, Some(&json), "uv-lib").unwrap();
        assert_eq!(
            url,
            format!("obsidian://open?vault={id}&file=modules/uv-lib.md")
        );
    }

    /// Task 1.3 — unregistered vault → `AppError::Invalid { field: "obsidian" }`.
    #[test]
    fn open_wiki_in_obsidian_rejects_unregistered_vault() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        write_file(&wiki_root, "modules/uv-lib.md", "# uv-lib\n");

        let err = resolve_obsidian_url(&wiki_root, Some(&json), "uv-lib").expect_err("must fail");
        match err {
            AppError::Invalid { field, .. } => assert_eq!(field, "obsidian"),
            other => panic!("expected Invalid(obsidian), got {other:?}"),
        }
    }

    /// Task 1.3 — registered vault but unknown slug → `Invalid { field: "slug" }`.
    #[test]
    fn open_wiki_in_obsidian_rejects_unknown_slug() {
        let tmp = tempfile::TempDir::new().unwrap();
        let json = tmp.path().join("obsidian/obsidian.json");
        let wiki_root = tmp.path().join("repo/.codebus/wiki");
        write_file(&wiki_root, "modules/uv-lib.md", "# uv-lib\n");
        register_temp_vault(&wiki_root, &json);

        let err =
            resolve_obsidian_url(&wiki_root, Some(&json), "no-such-page").expect_err("must fail");
        match err {
            AppError::Invalid { field, .. } => assert_eq!(field, "slug"),
            other => panic!("expected Invalid(slug), got {other:?}"),
        }
    }
}
