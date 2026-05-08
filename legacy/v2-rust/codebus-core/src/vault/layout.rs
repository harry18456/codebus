use crate::wiki::types::PageType;
use std::path::{Path, PathBuf};

/// Single source of truth for `.codebus/<sub>` paths. Code that touches
/// vault paths MUST go through `VaultPaths` rather than concatenating
/// strings — otherwise `flagStalePages` and `enrichSourceMetadata` drift
/// across modules (iter-7 lesson).
#[derive(Debug, Clone)]
pub struct VaultPaths {
    pub root: PathBuf,
    pub git: PathBuf,
    pub gitignore: PathBuf,
    pub goals_jsonl: PathBuf,
    pub schema_md: PathBuf,
    pub raw: PathBuf,
    pub raw_code: PathBuf,
    pub wiki: PathBuf,
    pub wiki_overview: PathBuf,
    pub wiki_index: PathBuf,
    pub wiki_log: PathBuf,
    pub wiki_concepts: PathBuf,
    pub wiki_entities: PathBuf,
    pub wiki_modules: PathBuf,
    pub wiki_processes: PathBuf,
    pub wiki_synthesis: PathBuf,
    /// Iteration order for the 5 page folders — callers that need to scan
    /// every type bucket use this slice instead of hand-listing.
    pub wiki_page_folders: [PathBuf; 5],
    pub output: PathBuf,
    pub lock: PathBuf,
}

impl VaultPaths {
    pub fn folder_for(&self, page_type: PageType) -> &Path {
        match page_type {
            PageType::Concept => &self.wiki_concepts,
            PageType::Entity => &self.wiki_entities,
            PageType::Module => &self.wiki_modules,
            PageType::Process => &self.wiki_processes,
            PageType::Synthesis => &self.wiki_synthesis,
        }
    }
}

pub fn vault_paths(repo_root: impl AsRef<Path>) -> VaultPaths {
    let root = repo_root.as_ref().join(".codebus");
    let wiki = root.join("wiki");
    let raw = root.join("raw");
    let wiki_concepts = wiki.join(PageType::Concept.folder());
    let wiki_entities = wiki.join(PageType::Entity.folder());
    let wiki_modules = wiki.join(PageType::Module.folder());
    let wiki_processes = wiki.join(PageType::Process.folder());
    let wiki_synthesis = wiki.join(PageType::Synthesis.folder());
    VaultPaths {
        git: root.join(".git"),
        gitignore: root.join(".gitignore"),
        goals_jsonl: root.join("goals.jsonl"),
        schema_md: root.join("CLAUDE.md"),
        raw_code: raw.join("code"),
        raw: raw.clone(),
        wiki_overview: wiki.join("overview.md"),
        wiki_index: wiki.join("index.md"),
        wiki_log: wiki.join("log.md"),
        wiki_page_folders: [
            wiki_concepts.clone(),
            wiki_entities.clone(),
            wiki_modules.clone(),
            wiki_processes.clone(),
            wiki_synthesis.clone(),
        ],
        wiki_concepts,
        wiki_entities,
        wiki_modules,
        wiki_processes,
        wiki_synthesis,
        wiki: wiki.clone(),
        output: root.join("output"),
        lock: root.join(".lock"),
        root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_built_under_dot_codebus() {
        let p = vault_paths("/tmp/repo");
        assert!(p.root.ends_with(".codebus"));
        assert!(p.wiki.ends_with("wiki"));
        assert!(p.raw.ends_with("raw"));
        assert!(p.raw_code.ends_with("code"));
    }

    #[test]
    fn page_folders_match_type_enum() {
        let p = vault_paths("/r");
        assert!(p.wiki_concepts.ends_with("concepts"));
        assert!(p.wiki_entities.ends_with("entities"));
        assert!(p.wiki_modules.ends_with("modules"));
        assert!(p.wiki_processes.ends_with("processes"));
        assert!(p.wiki_synthesis.ends_with("synthesis"));
    }

    #[test]
    fn wiki_page_folders_iteration_order() {
        let p = vault_paths("/r");
        let folders: Vec<_> = p
            .wiki_page_folders
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(
            folders,
            vec!["concepts", "entities", "modules", "processes", "synthesis"]
        );
    }

    #[test]
    fn folder_for_returns_each_type_bucket() {
        let p = vault_paths("/r");
        assert_eq!(p.folder_for(PageType::Concept), p.wiki_concepts.as_path());
        assert_eq!(
            p.folder_for(PageType::Synthesis),
            p.wiki_synthesis.as_path()
        );
    }

    #[test]
    fn nav_files_pinned_at_wiki_root() {
        let p = vault_paths("/r");
        assert!(p.wiki_index.ends_with("index.md"));
        assert!(p.wiki_log.ends_with("log.md"));
        assert!(p.wiki_overview.ends_with("overview.md"));
    }

    #[test]
    fn lock_and_goals_jsonl_at_root() {
        let p = vault_paths("/r");
        assert!(p.lock.ends_with(".lock"));
        assert!(p.goals_jsonl.ends_with("goals.jsonl"));
        assert!(p.schema_md.ends_with("CLAUDE.md"));
    }
}
