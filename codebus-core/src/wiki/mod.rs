//! Wiki domain — types, frontmatter parser, lint rules, fix loop.
//!
//! Ported from `legacy/v2-rust/codebus-core/src/wiki/` for v3-lint.
//! v3 augments v2 with: stable `rule_id` per lint issue (for JSON output
//! consumers like the fix agent), and vault root auto-detection so the
//! lint subcommand can run from either source repo cwd or vault cwd.

pub mod fix;
pub mod frontmatter;
pub mod lint;
pub mod types;

pub use frontmatter::{FrontmatterError, parse_page, serialize_page};
pub use lint::lint_wiki;
pub use types::{
    LintIssue, LintResult, LintSeverity, PageFrontmatter, PageType, ParsedPage, SourceRef,
};
