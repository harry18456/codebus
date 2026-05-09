//! `duplicate_slug` rule — flag pages whose slug collides across type
//! folders. Wikilink `[[slug]]` becomes ambiguous in Obsidian. v3-lint
//! Lint Rule Set rule 2.

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};

pub struct DuplicateSlugRule;

impl DuplicateSlugRule {
    pub const NAME: &'static str = "duplicate_slug";
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuplicateSlugRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for DuplicateSlugRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        for (slug, indices) in &ctx.catalog.slug_to_pages {
            if indices.len() <= 1 {
                continue;
            }
            let others: Vec<&str> = indices
                .iter()
                .map(|&i| ctx.pages[i].rel_path.as_str())
                .collect();
            let others_str = others.join(", ");
            for &i in indices {
                issues.push(LintIssue {
                    path: ctx.pages[i].rel_path.clone(),
                    severity: LintSeverity::Warn,
                    rule_id: "duplicate-slug".into(),
                    message: format!(
                        "duplicate slug '{slug}' across folders: {others_str} — wikilink [[{slug}]] becomes ambiguous"
                    ),
                });
            }
        }
        issues
    }
}
