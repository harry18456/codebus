//! Concrete [`super::rule::LintRule`] implementations. Order here is the
//! conventional reporting order — `factory::build_default_rules` returns
//! them in the same sequence so issue stream is stable.

pub mod broken_wikilink;
pub mod duplicate_slug;
pub mod frontmatter_integrity;
pub mod missing_nav;
pub mod page_size;
pub mod root_page;
pub mod unexpected_file;
