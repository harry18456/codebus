//! Lint rule factory. Returns the default set in conventional order so the
//! issue stream is stable across runs. Order matches v3-lint Lint Rule Set
//! reporting convention: catalog scans first, per-page checks, then nav.

use crate::wiki::lint::rule::LintRule;
use crate::wiki::lint::rules::{
    broken_wikilink::BrokenWikilinkRule, duplicate_slug::DuplicateSlugRule,
    frontmatter_integrity::FrontmatterIntegrityRule, missing_nav::MissingNavRule,
    root_page::RootPageRule, vault_gate_integrity::VaultGateIntegrityRule,
};

pub fn build_default_rules() -> Vec<Box<dyn LintRule>> {
    vec![
        Box::new(DuplicateSlugRule::new()),
        Box::new(FrontmatterIntegrityRule::new()),
        Box::new(BrokenWikilinkRule::new()),
        Box::new(RootPageRule::new()),
        Box::new(MissingNavRule::new()),
        Box::new(VaultGateIntegrityRule::new()),
    ]
}
