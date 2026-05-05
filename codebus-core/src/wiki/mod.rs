pub mod date;
pub mod frontmatter;
pub mod lint;
pub mod page_merge;
pub mod stale_detect;
pub mod types;

pub use date::utc_today_iso;
pub use frontmatter::{FrontmatterError, parse_page, serialize_page};
pub use lint::lint_wiki;
pub use page_merge::merge_page;
pub use stale_detect::{StaleResult, detect_stale_sources};
pub use types::{
    LintIssue, LintResult, LintSeverity, PageFrontmatter, PageType, ParsedPage, SourceRef,
};
