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

/// Karpathy-style 5-bucket page taxonomy. Frontmatter `type` is authoritative;
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
    /// All five variants in display order. Use this to iterate over the
    /// canonical set instead of hand-listing.
    pub const ALL: [PageType; 5] = [
        PageType::Concept,
        PageType::Entity,
        PageType::Module,
        PageType::Process,
        PageType::Synthesis,
    ];

    /// Pluralised wiki/ subfolder name for this type. `synthesis` has no
    /// plural form, so type and folder name happen to match.
    pub const fn folder(self) -> &'static str {
        match self {
            PageType::Concept => "concepts",
            PageType::Entity => "entities",
            PageType::Module => "modules",
            PageType::Process => "processes",
            PageType::Synthesis => "synthesis",
        }
    }

    /// Inverse of [`folder`]. Returns `None` for unrecognized folder names
    /// so lint can flag them as `unrecognized folder under wiki/`.
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

/// YAML frontmatter of a wiki knowledge page. Field names match the TS 0.1.0
/// schema verbatim so the same `.md` file deserializes identically across
/// the legacy reference impl and this Rust port.
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

/// Result of splitting `<frontmatter>\n---\n<body>` from a `.md` file.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintIssue {
    /// Path relative to `wiki/` (e.g. `"concepts/foo.md"`, `"index.md"`).
    pub path: String,
    pub severity: LintSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintResult {
    /// Knowledge pages with frontmatter, found under the 5 type folders and
    /// successfully parsed. Parse-failed files appear as errors instead.
    pub pages_scanned: usize,
    /// Navigation files actually read at `wiki/` root (`index.md`, `log.md`).
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
    fn source_ref_skips_none_fields_on_serialize() {
        let s = SourceRef {
            path: "src/a.rs".into(),
            sha256: None,
            at_commit: None,
        };
        let yaml = serde_yaml::to_string(&s).unwrap();
        assert!(yaml.contains("path:"));
        assert!(!yaml.contains("sha256"));
        assert!(!yaml.contains("at_commit"));
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
}
