//! [`LintRule`] trait + [`VaultContext`] passed to every rule.
//!
//! Rules are object-safe sync impls (Trait sync/async decision: lint is pure
//! CPU + local IO). The orchestrator in [`super`] owns the I/O cost — it
//! reads + parses every page once and hands rules a [`VaultContext`] with
//! pre-loaded results, so rules don't repeat work.

use crate::wiki::frontmatter::{FrontmatterError, parse_page};
use crate::wiki::types::{LintIssue, PageType, ParsedPage};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Filenames at `wiki/` root that are NOT flagged by `root_page` and ARE
/// valid `[[wikilink]]` targets when present. Other root-level `.md` files
/// (e.g. user-created `notes.md`) get flagged by `root_page`.
pub const SPECIAL_FILES: &[&str] = &["index.md", "log.md"];

/// Folder names directly under `wiki/` that the linter recognizes. Anything
/// else is flagged by `unexpected_file`.
pub const RECOGNIZED_ROOT_DIRS: &[&str] = &[
    "concepts",
    "entities",
    "modules",
    "processes",
    "synthesis",
    "goals",
];

/// Object-safe lint rule. Returns issues for the whole vault in one call —
/// rules are free to walk multiple pages or scan filesystem entries.
pub trait LintRule: Send + Sync {
    /// Stable identifier used for `disabled_rules` config and diagnostic
    /// output. Snake-case (e.g. `"page_size"`, `"broken_wikilink"`).
    fn name(&self) -> &str;

    /// Examine `ctx` and return any lint findings. Must be deterministic and
    /// pure (no global state, no fs writes).
    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue>;
}

/// One page successfully discovered under a type folder. Content + parse
/// results are pre-computed; rules consume the cached `Result` so each
/// page reads from disk + parses YAML at most once per `lint_wiki` call.
#[derive(Debug)]
pub struct LoadedPage {
    pub folder: &'static str,
    pub filename: String,
    pub slug: String,
    pub rel_path: String,
    pub full_path: PathBuf,
    /// `Err` for file read failures (rare; fs permission, transient IO).
    pub content_result: Result<String, io::Error>,
    /// `None` when content_result is Err (no parse attempted).
    /// `Some(Err)` when content was read but YAML parse failed.
    pub parsed_result: Option<Result<ParsedPage, FrontmatterError>>,
}

/// One special file at `wiki/` root (`index.md` / `log.md`). Pre-loaded so
/// rules can consume content without re-reading.
#[derive(Debug)]
pub struct NavFile {
    pub name: &'static str,
    pub present: bool,
    /// `Some` when present and read succeeded.
    pub content: Option<String>,
}

/// Pre-computed slug catalog used by [`super::rules::broken_wikilink`] +
/// [`super::rules::duplicate_slug`]. Building this once amortizes the
/// HashMap construction across rules that share the same view of pages.
#[derive(Debug, Default)]
pub struct Catalog {
    /// All valid wikilink targets: type-folder slugs + present SPECIAL_FILES
    /// minus their `.md` suffix.
    pub page_slugs: HashSet<String>,
    /// Per-slug list of indices into [`VaultContext::pages`]. Used to detect
    /// cross-folder collisions.
    pub slug_to_pages: HashMap<String, Vec<usize>>,
}

/// Snapshot of vault state passed to every rule. `pages` is in the order
/// pages were discovered (deterministic per filesystem traversal order).
#[derive(Debug)]
pub struct VaultContext {
    pub wiki_root: PathBuf,
    pub pages: Vec<LoadedPage>,
    pub nav_files: Vec<NavFile>,
    pub catalog: Catalog,
}

impl VaultContext {
    /// Walk `wiki_root`, read + parse every page in the 5 type folders,
    /// and pre-load nav files. Filesystem failures (missing wiki_root) are
    /// surfaced via empty `pages` / `nav_files` rather than panic.
    pub fn build(wiki_root: &Path) -> Self {
        let mut pages: Vec<LoadedPage> = Vec::new();
        let mut catalog = Catalog::default();

        if wiki_root.exists() {
            for t in PageType::ALL {
                let folder = t.folder();
                let folder_path = wiki_root.join(folder);
                if !folder_path.exists() {
                    continue;
                }
                let Ok(rd) = fs::read_dir(&folder_path) else {
                    continue;
                };
                for e in rd.flatten() {
                    let name = match e.file_name().into_string() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    if !name.ends_with(".md") {
                        continue;
                    }
                    let slug = name.trim_end_matches(".md").to_string();
                    let full_path = folder_path.join(&name);
                    let content_result = fs::read_to_string(&full_path);
                    let parsed_result = match &content_result {
                        Ok(s) => Some(parse_page(s)),
                        Err(_) => None,
                    };
                    let idx = pages.len();
                    catalog.page_slugs.insert(slug.clone());
                    catalog
                        .slug_to_pages
                        .entry(slug.clone())
                        .or_default()
                        .push(idx);
                    pages.push(LoadedPage {
                        folder,
                        filename: name.clone(),
                        slug,
                        rel_path: format!("{folder}/{name}"),
                        full_path,
                        content_result,
                        parsed_result,
                    });
                }
            }
        }

        // Special files: present + content snapshot. Their slug also enters
        // the catalog so [[index]] / [[log]] resolve correctly.
        let mut nav_files = Vec::new();
        for &name in SPECIAL_FILES {
            let path = wiki_root.join(name);
            let present = path.exists();
            let content = if present {
                fs::read_to_string(&path).ok()
            } else {
                None
            };
            if present {
                catalog
                    .page_slugs
                    .insert(name.trim_end_matches(".md").to_string());
            }
            nav_files.push(NavFile {
                name,
                present,
                content,
            });
        }

        Self {
            wiki_root: wiki_root.to_path_buf(),
            pages,
            nav_files,
            catalog,
        }
    }

    /// Count of pages whose YAML frontmatter parsed successfully. Used by
    /// the orchestrator to populate [`crate::wiki::types::LintResult::pages_scanned`].
    pub fn pages_scanned(&self) -> usize {
        self.pages
            .iter()
            .filter(|p| matches!(&p.parsed_result, Some(Ok(_))))
            .count()
    }

    /// Count of present nav files. Used by the orchestrator to populate
    /// [`crate::wiki::types::LintResult::nav_files_scanned`].
    pub fn nav_files_scanned(&self) -> usize {
        self.nav_files.iter().filter(|nf| nf.present).count()
    }
}
