//! Lint rule factory. Returns the default set in conventional order so the
//! issue stream is stable across runs.

use crate::wiki::lint::rule::LintRule;
use crate::wiki::lint::rules::{
    broken_wikilink::BrokenWikilinkRule, duplicate_slug::DuplicateSlugRule,
    frontmatter_integrity::FrontmatterIntegrityRule, missing_nav::MissingNavRule,
    page_size::PageSizeRule, root_page::RootPageRule, unexpected_file::UnexpectedFileRule,
};

/// Default rule set. Order matches the legacy single-file `lint_wiki` flow:
/// catalog-first scans, then per-page rules, then root scans, then nav.
pub fn build_default_rules() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(UnexpectedFileRule::new()),
        Box::new(DuplicateSlugRule::new()),
        Box::new(PageSizeRule::new()),
        Box::new(FrontmatterIntegrityRule::new()),
        Box::new(BrokenWikilinkRule::new()),
        Box::new(RootPageRule::new()),
        Box::new(MissingNavRule::new()),
    ]
}
