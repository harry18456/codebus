//! `page_size` rule — flag `.md` files exceeding type-specific thresholds.
//!
//! Thresholds (bytes, strict greater-than):
//!   - `index.md`            1024
//!   - `wiki/synthesis/*.md` 5120
//!   - `wiki/{concepts,entities,modules,processes}/*.md` 8192
//!   - `log.md` + `overview.md` — unlimited (log grows by design; overview
//!     is special).

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};

const INDEX_MD_THRESHOLD: usize = 1024;
const SYNTHESIS_THRESHOLD: usize = 5120;
const TYPE_FOLDER_THRESHOLD: usize = 8192;

pub struct PageSizeRule;

impl PageSizeRule {
    pub const NAME: &'static str = "page_size";
    pub fn new() -> Self {
        Self
    }
}

impl Default for PageSizeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for PageSizeRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();

        for page in &ctx.pages {
            let Ok(content) = page.content_result.as_ref() else {
                continue;
            };
            push_if_oversize(&page.rel_path, content.len(), &mut issues);
        }

        for nav in &ctx.nav_files {
            let Some(content) = nav.content.as_ref() else {
                continue;
            };
            push_if_oversize(nav.name, content.len(), &mut issues);
        }

        issues
    }
}

fn push_if_oversize(rel_path: &str, size: usize, out: &mut Vec<LintIssue>) {
    let Some(threshold) = threshold_for(rel_path) else {
        return;
    };
    if size > threshold {
        out.push(LintIssue {
            path: rel_path.to_string(),
            severity: LintSeverity::Warn,
            message: format!(
                "oversize page (size {size} bytes, threshold {threshold} bytes) — split or extract sub-page"
            ),
        });
    }
}

fn threshold_for(rel_path: &str) -> Option<usize> {
    if rel_path == "index.md" {
        return Some(INDEX_MD_THRESHOLD);
    }
    if rel_path == "log.md" || rel_path == "overview.md" {
        return None;
    }
    let folder = rel_path.split('/').next()?;
    match folder {
        "synthesis" => Some(SYNTHESIS_THRESHOLD),
        "concepts" | "entities" | "modules" | "processes" => Some(TYPE_FOLDER_THRESHOLD),
        _ => None,
    }
}
