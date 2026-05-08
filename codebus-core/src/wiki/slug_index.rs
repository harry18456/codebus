//! Slug → location index for the wiki vault.
//!
//! Built once per run by walking `.codebus/wiki/`: 5 typed folders
//! (concepts/entities/modules/processes/synthesis), 3 special pages at root
//! (overview/index/log), and `goals/<slug>.md` per-goal reading guides.
//!
//! The terminal renderer consults this index to resolve `[[slug]]` wikilinks
//! to actual on-disk relative paths (without `.md` extension), which it then
//! embeds into OSC 8 hyperlink URIs as `&file=<path>`.
//!
//! ## Slug derivation
//!
//! Slug is the file stem (filename without `.md`). Codebus enforces the
//! "filename = slug" invariant via `wiki/lint/duplicate_slug.rs`; we do NOT
//! parse frontmatter here — file stem is canonical and avoids extra I/O.
//!
//! ## Path traversal note (audit: scoundrel)
//!
//! A malicious vault could conceivably contain slugs with `..` segments.
//! This index does NOT canonicalize paths — it stores them verbatim relative
//! to the wiki root. Containment is enforced by the OSC 8 URI consumer
//! (Obsidian) which interprets `&file=<path>` within its registered vault
//! root. Skipping canonicalization here also means error messages keep
//! showing the user-visible repo-relative path.

use crate::vault::layout::VaultPaths;
use crate::wiki::types::PageType;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

/// Where in the vault a wikilink target lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlugLocation {
    /// One of the 5 typed folders (concepts/entities/modules/processes/synthesis).
    Type(PageType),
    /// Root special page: overview / index / log.
    Special,
    /// `goals/<slug>.md` per-goal reading guide.
    Goal,
}

/// Map from slug → (location, relative path without `.md`).
///
/// Path is forward-slashed and relative to the wiki root (e.g.
/// `concepts/foo`, `overview`, `goals/explain-x`). This matches the OSC 8
/// URI `&file=<path>` form — Obsidian accepts forward slash on Windows.
#[derive(Debug, Clone, Default)]
pub struct SlugIndex {
    entries: HashMap<String, (SlugLocation, PathBuf)>,
}

impl SlugIndex {
    pub fn lookup(&self, slug: &str) -> Option<&(SlugLocation, PathBuf)> {
        self.entries.get(slug)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Test-only seam for populating an index without touching the
    /// filesystem. Marked `pub` (not `pub(crate)`) so unit tests in the
    /// terminal renderer crate-internal module can populate fixtures
    /// without scaffolding a tempdir vault, but `#[doc(hidden)]` to
    /// keep it out of the rustdoc surface — production callers should
    /// use [`build`].
    #[doc(hidden)]
    pub fn insert_for_test(&mut self, slug: String, loc: SlugLocation, path: PathBuf) {
        self.entries.insert(slug, (loc, path));
    }
}

const SPECIAL_PAGES: [&str; 3] = ["overview", "index", "log"];

/// Build the slug index by scanning the vault once.
///
/// Iteration order matches `PageType::ALL` (concepts → entities → modules →
/// processes → synthesis), then specials, then goals. When the same slug
/// appears in multiple typed folders, the LAST insertion wins — i.e.
/// later types in `PageType::ALL` overwrite earlier ones (entities beats
/// concepts, synthesis beats everything). This is documented behaviour;
/// the `duplicate_slug` lint rule already warns on cross-folder slug
/// collisions, so resolving them here is best-effort fallback.
///
/// Missing subdirectories (a fresh vault that never produced a Synthesis
/// page) are silently skipped — only real I/O errors (permission denied,
/// etc.) propagate. The OK arm always returns a complete scan: if any
/// folder fails I/O mid-walk, the whole build fails.
pub fn build(vault_paths: &VaultPaths) -> Result<SlugIndex, io::Error> {
    let mut entries: HashMap<String, (SlugLocation, PathBuf)> = HashMap::new();

    // 5 typed folders, in PageType::ALL order.
    for pt in PageType::ALL {
        let folder = vault_paths.folder_for(pt);
        let folder_name = pt.folder();
        scan_md_dir(folder, |slug| {
            entries.insert(
                slug.to_string(),
                (
                    SlugLocation::Type(pt),
                    PathBuf::from(format!("{folder_name}/{slug}")),
                ),
            );
        })?;
    }

    // 3 special root files: overview / index / log.
    for name in SPECIAL_PAGES {
        let candidate = vault_paths.wiki.join(format!("{name}.md"));
        match candidate.metadata() {
            Ok(meta) if meta.is_file() => {
                entries.insert(
                    name.to_string(),
                    (SlugLocation::Special, PathBuf::from(name)),
                );
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }

    // goals/<slug>.md.
    let goals_dir = vault_paths.wiki.join("goals");
    scan_md_dir(&goals_dir, |slug| {
        entries.insert(
            slug.to_string(),
            (
                SlugLocation::Goal,
                PathBuf::from(format!("goals/{slug}")),
            ),
        );
    })?;

    Ok(SlugIndex { entries })
}

/// Iterate `.md` files (non-hidden) directly under `dir` and call `on_slug`
/// with each file stem. Missing dir is OK (no-op); other I/O errors bubble.
fn scan_md_dir<F>(dir: &Path, mut on_slug: F) -> Result<(), io::Error>
where
    F: FnMut(&str),
{
    let read = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in read {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if stem.is_empty() || stem.starts_with('.') {
            continue;
        }
        on_slug(stem);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::layout::vault_paths;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "codebus-slug-index-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// Create the 5 type folders + goals/ under the vault wiki dir.
    fn scaffold_empty_vault(repo: &Path) {
        let vp = vault_paths(repo);
        for f in &vp.wiki_page_folders {
            fs::create_dir_all(f).unwrap();
        }
        fs::create_dir_all(vp.wiki.join("goals")).unwrap();
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "").unwrap();
    }

    #[test]
    fn empty_vault_returns_empty_index() {
        let repo = tmp("empty");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        let idx = build(&vp).unwrap();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn concept_page_resolves_to_typed_location() {
        let repo = tmp("concept");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        touch(&vp.wiki_concepts.join("foo.md"));
        let idx = build(&vp).unwrap();
        let (loc, path) = idx.lookup("foo").expect("foo should be indexed");
        assert_eq!(*loc, SlugLocation::Type(PageType::Concept));
        assert_eq!(path, &PathBuf::from("concepts/foo"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn special_page_overview_resolves_to_special() {
        let repo = tmp("specials");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        touch(&vp.wiki_overview);
        touch(&vp.wiki_index);
        touch(&vp.wiki_log);
        let idx = build(&vp).unwrap();
        for name in ["overview", "index", "log"] {
            let (loc, path) = idx
                .lookup(name)
                .unwrap_or_else(|| panic!("{name} should be indexed"));
            assert_eq!(*loc, SlugLocation::Special);
            assert_eq!(path, &PathBuf::from(name));
        }
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn goal_page_resolves_to_goal() {
        let repo = tmp("goal");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        touch(&vp.wiki.join("goals").join("explain-checkout.md"));
        let idx = build(&vp).unwrap();
        let (loc, path) = idx
            .lookup("explain-checkout")
            .expect("explain-checkout should be indexed");
        assert_eq!(*loc, SlugLocation::Goal);
        assert_eq!(path, &PathBuf::from("goals/explain-checkout"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn duplicate_slug_across_folders_takes_last() {
        // PageType::ALL = [Concept, Entity, Module, Process, Synthesis].
        // Later-iterated entries overwrite earlier ones, so entities beats
        // concepts. The duplicate_slug lint rule warns on this; this test
        // pins deterministic resolution behaviour for the renderer.
        let repo = tmp("dup");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        touch(&vp.wiki_concepts.join("foo.md"));
        touch(&vp.wiki_entities.join("foo.md"));
        let idx = build(&vp).unwrap();
        let (loc, path) = idx.lookup("foo").expect("foo should be indexed");
        assert_eq!(*loc, SlugLocation::Type(PageType::Entity));
        assert_eq!(path, &PathBuf::from("entities/foo"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn non_md_files_ignored() {
        let repo = tmp("nonmd");
        scaffold_empty_vault(&repo);
        let vp = vault_paths(&repo);
        touch(&vp.wiki_concepts.join("foo.txt"));
        touch(&vp.wiki_concepts.join(".hidden.md"));
        let idx = build(&vp).unwrap();
        assert!(idx.is_empty(), "expected empty, got {} entries", idx.len());
        assert!(idx.lookup("foo").is_none());
        assert!(idx.lookup(".hidden").is_none());
        assert!(idx.lookup("hidden").is_none());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn missing_subdirectory_does_not_error() {
        // Create the wiki root + just one type folder. Other 4 typed
        // folders, goals/, and the special files are absent. build()
        // must succeed and yield only what exists.
        let repo = tmp("missing");
        let vp = vault_paths(&repo);
        fs::create_dir_all(&vp.wiki_concepts).unwrap();
        touch(&vp.wiki_concepts.join("only.md"));
        let idx = build(&vp).unwrap();
        assert_eq!(idx.len(), 1);
        let (loc, path) = idx.lookup("only").unwrap();
        assert_eq!(*loc, SlugLocation::Type(PageType::Concept));
        assert_eq!(path, &PathBuf::from("concepts/only"));
        let _ = fs::remove_dir_all(&repo);
    }
}
