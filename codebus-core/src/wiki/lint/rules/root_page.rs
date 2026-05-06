//! `root_page` rule — flag `.md` files placed directly under `wiki/` root
//! that aren't a recognized special file. Pages should live inside a type
//! folder (`wiki/<type>/foo.md`).

use crate::wiki::lint::rule::{LintRule, SPECIAL_FILES, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};
use std::fs;

pub struct RootPageRule;

impl RootPageRule {
    pub const NAME: &'static str = "root_page";
    pub fn new() -> Self {
        Self
    }
}

impl Default for RootPageRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for RootPageRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        let Ok(rd) = fs::read_dir(&ctx.wiki_root) else {
            return issues;
        };
        for e in rd.flatten() {
            let Ok(name) = e.file_name().into_string() else {
                continue;
            };
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_file() || !name.ends_with(".md") {
                continue;
            }
            if SPECIAL_FILES.contains(&name.as_str()) {
                continue;
            }
            issues.push(LintIssue {
                path: name.clone(),
                severity: LintSeverity::Warn,
                message: format!(
                    "page lives in wiki/ root — schema §3 expects wiki/<type>/{name} (one of: concepts, entities, modules, processes, synthesis)"
                ),
            });
        }
        issues
    }
}
