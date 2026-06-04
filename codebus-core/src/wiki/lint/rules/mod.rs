//! Concrete [`super::rule::LintRule`] implementations. Order in [`mod@super::factory`]
//! is the conventional reporting order.

pub mod broken_wikilink;
pub mod duplicate_slug;
pub mod frontmatter_integrity;
pub mod missing_nav;
pub mod root_page;
pub mod vault_gate_integrity;
