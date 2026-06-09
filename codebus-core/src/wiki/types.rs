//! Wiki type definitions ported from the v2 implementation.
//!
//! v3 additions:
//! - `LintIssue.rule_id`: stable kebab-case identifier per rule, supplied to
//!   JSON output consumers (fix agent reads it to pick a repair strategy
//!   without parsing free-form `message` text).

use serde::{Deserialize, Serialize};

/// One entry under `frontmatter.sources[]`. `path` is a logical source-repo
/// path (no `raw/code/` prefix). `sha256` and `at_commit` are auto-filled
/// post-spawn — agents only ever write `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_commit: Option<String>,
}

/// Karpathy 5-bucket page taxonomy. Frontmatter `type` is authoritative;
/// the same-named folder under `wiki/` is an organizational hint for Obsidian
/// sidebar grouping, not a strict filing contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageType {
    Concept,
    Entity,
    Module,
    Process,
    Synthesis,
}

impl PageType {
    pub const ALL: [PageType; 5] = [
        PageType::Concept,
        PageType::Entity,
        PageType::Module,
        PageType::Process,
        PageType::Synthesis,
    ];

    pub const fn folder(self) -> &'static str {
        match self {
            PageType::Concept => "concepts",
            PageType::Entity => "entities",
            PageType::Module => "modules",
            PageType::Process => "processes",
            PageType::Synthesis => "synthesis",
        }
    }

    pub fn from_folder(folder: &str) -> Option<PageType> {
        match folder {
            "concepts" => Some(PageType::Concept),
            "entities" => Some(PageType::Entity),
            "modules" => Some(PageType::Module),
            "processes" => Some(PageType::Process),
            "synthesis" => Some(PageType::Synthesis),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFrontmatter {
    pub title: String,
    #[serde(rename = "type")]
    pub page_type: PageType,
    #[serde(default)]
    pub sources: Vec<SourceRef>,
    #[serde(default)]
    pub goals: Vec<String>,
    pub created: String,
    pub updated: String,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPage {
    pub frontmatter: PageFrontmatter,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    Error,
    Warn,
}

/// One lint finding. `path` is vault-relative (e.g. `concepts/foo.md`,
/// `index.md`); JSON output formatter joins with `vault_root` to produce
/// absolute paths for agent consumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintIssue {
    pub path: String,
    pub severity: LintSeverity,
    /// Stable kebab-case rule identifier (e.g. `frontmatter-parse`,
    /// `broken-wikilink-body`). Used by JSON consumers to dispatch on rule
    /// without parsing `message` text. v3 addition over the v2 type.
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintResult {
    pub pages_scanned: usize,
    pub nav_files_scanned: usize,
    pub issues: Vec<LintIssue>,
    pub error_count: usize,
    pub warn_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_type_folder_roundtrip() {
        for t in PageType::ALL {
            assert_eq!(PageType::from_folder(t.folder()), Some(t));
        }
    }

    #[test]
    fn page_type_unknown_folder_is_none() {
        assert_eq!(PageType::from_folder("scratch"), None);
        assert_eq!(PageType::from_folder("goals"), None);
        assert_eq!(PageType::from_folder(""), None);
    }

    #[test]
    fn page_type_serde_lowercase() {
        let json = serde_json::to_string(&PageType::Concept).unwrap();
        assert_eq!(json, "\"concept\"");
        let parsed: PageType = serde_json::from_str("\"synthesis\"").unwrap();
        assert_eq!(parsed, PageType::Synthesis);
    }

    #[test]
    fn frontmatter_optional_fields_default() {
        let yaml = "title: Foo\ntype: concept\ncreated: '2026-05-05'\nupdated: '2026-05-05'\n";
        let parsed: PageFrontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.title, "Foo");
        assert_eq!(parsed.page_type, PageType::Concept);
        assert!(parsed.sources.is_empty());
        assert!(parsed.related.is_empty());
        assert!(!parsed.stale);
    }

    #[test]
    fn lint_issue_serde_roundtrip_includes_rule_id() {
        let issue = LintIssue {
            path: "concepts/foo.md".into(),
            severity: LintSeverity::Error,
            rule_id: "frontmatter-parse".into(),
            message: "frontmatter parse failed".into(),
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("\"rule_id\":\"frontmatter-parse\""));
    }
}
