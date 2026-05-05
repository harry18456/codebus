pub mod date;
pub mod frontmatter;
pub mod lint;
pub mod page_merge;
pub mod stale_detect;
pub mod types;

pub use date::utc_today_iso;
pub use frontmatter::{parse_page, serialize_page, FrontmatterError};
pub use lint::lint_wiki;
pub use page_merge::merge_page;
pub use stale_detect::{detect_stale_sources, StaleResult};
pub use types::{
    LintIssue, LintResult, LintSeverity, PageFrontmatter, PageType, ParsedPage, SourceRef,
};
