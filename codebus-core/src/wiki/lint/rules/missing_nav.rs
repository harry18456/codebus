//! `missing_nav` rule — flag `index.md` / `log.md` absent at `wiki/` root.
//!
//! `overview.md` is intentionally NOT required: vault may not have a
//! curated overview yet, and one will appear inside a type folder when it
//! does (`wiki/<type>/overview.md`).

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};

pub struct MissingNavRule;

impl MissingNavRule {
    pub const NAME: &'static str = "missing_nav";
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingNavRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for MissingNavRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        for nav in &ctx.nav_files {
            if !nav.present {
                issues.push(LintIssue {
                    path: nav.name.to_string(),
                    severity: LintSeverity::Warn,
                    message: format!(
                        "{name} missing — schema §3 expects this special file",
                        name = nav.name
                    ),
                });
            }
        }
        issues
    }
}
