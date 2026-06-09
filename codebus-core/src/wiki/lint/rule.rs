//! [`LintRule`] trait + [`VaultContext`] passed to every rule.
//!
//! Rules are object-safe sync impls. Orchestrator owns I/O cost — reads +
//! parses every page once and hands rules a [`VaultContext`] with pre-loaded
//! results. Ported from the v2 implementation.

use crate::wiki::frontmatter::{FrontmatterError, parse_page};
use crate::wiki::types::{LintIssue, PageType, ParsedPage};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Filenames at `wiki/` root that are NOT flagged by `root_page` and ARE
/// valid `[[wikilink]]` targets when present.
pub const SPECIAL_FILES: &[&str] = &["index.md", "log.md"];

/// Folder names directly under `wiki/` that the linter recognizes.
pub const RECOGNIZED_ROOT_DIRS: &[&str] =
    &["concepts", "entities", "modules", "processes", "synthesis"];

pub trait LintRule: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue>;
}

#[derive(Debug)]
pub struct LoadedPage {
    pub folder: &'static str,
    pub filename: String,
    pub slug: String,
    pub rel_path: String,
    pub full_path: PathBuf,
    pub content_result: Result<String, io::Error>,
    pub parsed_result: Option<Result<ParsedPage, FrontmatterError>>,
}

#[derive(Debug)]
pub struct NavFile {
    pub name: &'static str,
    pub present: bool,
    pub content: Option<String>,
}

#[derive(Debug, Default)]
pub struct Catalog {
    pub page_slugs: HashSet<String>,
    pub slug_to_pages: HashMap<String, Vec<usize>>,
}

#[derive(Debug)]
pub struct VaultContext {
    /// The vault root (`.codebus/`) — the parent of `wiki_root`. Used by
    /// rules that read vault-internal files outside the `wiki/` subtree
    /// (e.g. the `vault-gate-integrity` rule reads
    /// `vault_root/.claude/settings.json`). Falls back to `wiki_root` when
    /// `wiki_root` has no parent.
    pub vault_root: PathBuf,
    pub wiki_root: PathBuf,
    pub pages: Vec<LoadedPage>,
    pub nav_files: Vec<NavFile>,
    pub catalog: Catalog,
}

impl VaultContext {
    pub fn build(wiki_root: &Path) -> Self {
        let vault_root = wiki_root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| wiki_root.to_path_buf());
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
            vault_root,
            wiki_root: wiki_root.to_path_buf(),
            pages,
            nav_files,
            catalog,
        }
    }

    pub fn pages_scanned(&self) -> usize {
        self.pages
            .iter()
            .filter(|p| matches!(&p.parsed_result, Some(Ok(_))))
            .count()
    }

    pub fn nav_files_scanned(&self) -> usize {
        self.nav_files.iter().filter(|nf| nf.present).count()
    }
}
