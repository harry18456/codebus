//! `duplicate_slug` rule — flag pages whose slug collides across type
//! folders (e.g. `concepts/cart.md` AND `entities/cart.md`). Wikilink
//! `[[cart]]` becomes ambiguous in Obsidian.

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
                    message: format!(
                        "duplicate slug '{slug}' across folders: {others_str} — wikilink [[{slug}]] becomes ambiguous"
                    ),
                });
            }
        }
        issues
    }
}
