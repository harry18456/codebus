//! `unexpected_file` rule — flag entries under `wiki/` that don't fit the
//! expected layout: unrecognized folders at the root, non-`.md` files
//! inside type folders, nested sub-folders inside type folders.
//!
//! Hidden entries (names starting with `.`) are skipped silently — Obsidian
//! reserves those for tool config (`.obsidian/`, `.gitkeep`, etc.).

use crate::wiki::lint::rule::{LintRule, RECOGNIZED_ROOT_DIRS, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity, PageType};
use std::fs;
use std::path::Path;

pub struct UnexpectedFileRule;

impl UnexpectedFileRule {
    pub const NAME: &'static str = "unexpected_file";
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnexpectedFileRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for UnexpectedFileRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        if !ctx.wiki_root.exists() {
            return issues;
        }

        scan_root_dirs(&ctx.wiki_root, &mut issues);

        for t in PageType::ALL {
            let folder = t.folder();
            let folder_path = ctx.wiki_root.join(folder);
            if folder_path.exists() {
                scan_type_folder(&folder_path, folder, &mut issues);
            }
        }

        issues
    }
}

fn scan_root_dirs(wiki_root: &Path, issues: &mut Vec<LintIssue>) {
    let Ok(entries) = fs::read_dir(wiki_root) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() {
            continue;
        }
        if !RECOGNIZED_ROOT_DIRS.contains(&name.as_str()) {
            issues.push(LintIssue {
                path: name.clone(),
                severity: LintSeverity::Warn,
                message: format!("unrecognized folder under wiki/: {name}"),
            });
        }
    }
}

fn scan_type_folder(folder_path: &Path, folder: &str, issues: &mut Vec<LintIssue>) {
    let Ok(entries) = fs::read_dir(folder_path) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(name) = e.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            issues.push(LintIssue {
                path: format!("{folder}/{name}"),
                severity: LintSeverity::Warn,
                message: format!("nested sub-folder in type folder: {folder}/{name}"),
            });
        } else if ft.is_file() && !name.ends_with(".md") {
            issues.push(LintIssue {
                path: format!("{folder}/{name}"),
                severity: LintSeverity::Warn,
                message: format!("non-.md file in type folder: {folder}/{name}"),
            });
        }
    }
}
