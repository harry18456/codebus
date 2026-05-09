//! YAML frontmatter parser ported from `legacy/v2-rust/codebus-core/src/wiki/frontmatter.rs`.
//!
//! Required by lint rules that need to inspect `related[]`, `sources[]`,
//! `stale`, and detect parse failures (rule `frontmatter-parse`).

use crate::wiki::types::{PageFrontmatter, PageType, ParsedPage, SourceRef};
use serde::Deserialize;
use std::fmt;

#[derive(Debug)]
pub enum FrontmatterError {
    MissingOpeningDelimiter,
    MissingClosingDelimiter,
    YamlParse(serde_yaml::Error),
    MissingRequiredField(&'static str),
    InvalidPageType(String),
}

impl fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrontmatterError::MissingOpeningDelimiter => {
                write!(f, "frontmatter parse failed: missing opening --- delimiter")
            }
            FrontmatterError::MissingClosingDelimiter => {
                write!(f, "frontmatter parse failed: missing closing --- delimiter")
            }
            FrontmatterError::YamlParse(e) => write!(f, "frontmatter parse failed: {e}"),
            FrontmatterError::MissingRequiredField(field) => {
                write!(f, "Missing required field in frontmatter: {field}")
            }
            FrontmatterError::InvalidPageType(t) => {
                write!(
                    f,
                    "Invalid page type: {t} (must be one of concept|entity|module|process|synthesis)"
                )
            }
        }
    }
}

impl std::error::Error for FrontmatterError {}

#[derive(Deserialize)]
struct RawFrontmatter {
    title: Option<serde_yaml::Value>,
    #[serde(rename = "type")]
    page_type: Option<serde_yaml::Value>,
    sources: Option<serde_yaml::Value>,
    goals: Option<serde_yaml::Value>,
    created: Option<serde_yaml::Value>,
    updated: Option<serde_yaml::Value>,
    related: Option<serde_yaml::Value>,
    stale: Option<serde_yaml::Value>,
}

pub fn parse_page(content: &str) -> Result<ParsedPage, FrontmatterError> {
    let after_open = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
        .ok_or(FrontmatterError::MissingOpeningDelimiter)?;

    let (yaml, body) =
        split_at_closing(after_open).ok_or(FrontmatterError::MissingClosingDelimiter)?;

    let raw: RawFrontmatter = serde_yaml::from_str(yaml).map_err(FrontmatterError::YamlParse)?;

    let title = required_string(&raw.title, "title")?;
    let page_type_str = required_string(&raw.page_type, "type")?;
    let page_type = match page_type_str.as_str() {
        "concept" => PageType::Concept,
        "entity" => PageType::Entity,
        "module" => PageType::Module,
        "process" => PageType::Process,
        "synthesis" => PageType::Synthesis,
        other => return Err(FrontmatterError::InvalidPageType(other.to_string())),
    };
    let sources = normalize_sources(&raw.sources);
    let goals = normalize_string_list(&raw.goals, "goals")?;
    let created = required_string(&raw.created, "created")?;
    let updated = required_string(&raw.updated, "updated")?;
    let related = normalize_string_list(&raw.related, "related")?;
    let stale = match &raw.stale {
        Some(serde_yaml::Value::Bool(b)) => *b,
        Some(_) => false,
        None => return Err(FrontmatterError::MissingRequiredField("stale")),
    };

    Ok(ParsedPage {
        frontmatter: PageFrontmatter {
            title,
            page_type,
            sources,
            goals,
            created,
            updated,
            related,
            stale,
        },
        body: body.to_string(),
    })
}

pub fn serialize_page(frontmatter: &PageFrontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).expect("PageFrontmatter is always serializable");
    let mut out = String::with_capacity(yaml.len() + body.len() + 16);
    out.push_str("---\n");
    out.push_str(&yaml);
    if !yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n");
    out.push_str(body);
    out
}

fn split_at_closing(s: &str) -> Option<(&str, &str)> {
    let mut byte_offset = 0usize;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            let yaml_end = byte_offset;
            let body_start = byte_offset + line.len();
            let yaml = &s[..yaml_end];
            let yaml = yaml.strip_suffix('\n').unwrap_or(yaml);
            let yaml = yaml.strip_suffix('\r').unwrap_or(yaml);
            let body = &s[body_start..];
            return Some((yaml, body));
        }
        byte_offset += line.len();
    }
    None
}

fn required_string(
    v: &Option<serde_yaml::Value>,
    field: &'static str,
) -> Result<String, FrontmatterError> {
    match v {
        Some(serde_yaml::Value::String(s)) => Ok(s.clone()),
        Some(other) => Ok(yaml_value_to_string(other)),
        None => Err(FrontmatterError::MissingRequiredField(field)),
    }
}

fn yaml_value_to_string(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        _ => serde_yaml::to_string(v)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

fn normalize_sources(v: &Option<serde_yaml::Value>) -> Vec<SourceRef> {
    let Some(serde_yaml::Value::Sequence(seq)) = v else {
        return Vec::new();
    };
    seq.iter()
        .filter_map(|entry| {
            let serde_yaml::Value::Mapping(map) = entry else {
                return None;
            };
            let path = map
                .get(serde_yaml::Value::String("path".into()))
                .and_then(|p| p.as_str())
                .map(|s| s.to_string())?;
            let sha256 = map
                .get(serde_yaml::Value::String("sha256".into()))
                .and_then(|p| p.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let at_commit = map
                .get(serde_yaml::Value::String("at_commit".into()))
                .and_then(|p| p.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            Some(SourceRef {
                path,
                sha256,
                at_commit,
            })
        })
        .collect()
}

fn normalize_string_list(
    v: &Option<serde_yaml::Value>,
    field: &'static str,
) -> Result<Vec<String>, FrontmatterError> {
    match v {
        None => Err(FrontmatterError::MissingRequiredField(field)),
        Some(serde_yaml::Value::Sequence(seq)) => {
            Ok(seq.iter().map(yaml_value_to_string).collect())
        }
        Some(_) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_preserves_leading_newline_after_closing_delim() {
        let content = "---\ntitle: X\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n\n# heading\n";
        let parsed = parse_page(content).unwrap();
        assert!(parsed.body.starts_with('\n'));
        assert!(parsed.body.contains("# heading"));
    }

    #[test]
    fn missing_opening_delimiter_errors() {
        let content = "title: x\n";
        match parse_page(content) {
            Err(FrontmatterError::MissingOpeningDelimiter) => {}
            other => panic!("expected MissingOpeningDelimiter, got {other:?}"),
        }
    }

    #[test]
    fn missing_closing_delimiter_errors() {
        let content = "---\ntitle: x\ntype: concept\n";
        match parse_page(content) {
            Err(FrontmatterError::MissingClosingDelimiter) => {}
            other => panic!("expected MissingClosingDelimiter, got {other:?}"),
        }
    }

    #[test]
    fn missing_required_field_reports_specific_field() {
        let content = "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\n---\nbody";
        match parse_page(content) {
            Err(FrontmatterError::MissingRequiredField("stale")) => {}
            other => panic!("expected MissingRequiredField(stale), got {other:?}"),
        }
    }

    #[test]
    fn invalid_page_type_errors_with_value() {
        let content = "---\ntitle: x\ntype: gibberish\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\nbody";
        match parse_page(content) {
            Err(FrontmatterError::InvalidPageType(t)) => assert_eq!(t, "gibberish"),
            other => panic!("expected InvalidPageType(gibberish), got {other:?}"),
        }
    }

    #[test]
    fn page_type_parses_all_five_variants() {
        for (s, expected) in [
            ("concept", PageType::Concept),
            ("entity", PageType::Entity),
            ("module", PageType::Module),
            ("process", PageType::Process),
            ("synthesis", PageType::Synthesis),
        ] {
            let content = format!(
                "---\ntitle: x\ntype: {s}\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n"
            );
            let p = parse_page(&content).unwrap();
            assert_eq!(p.frontmatter.page_type, expected);
        }
    }

    #[test]
    fn related_with_quoted_wikilinks_parses_to_bracketed_strings() {
        let content = "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated:\n  - '[[a]]'\n  - '[[b]]'\nstale: false\n---\n";
        let p = parse_page(content).unwrap();
        assert_eq!(p.frontmatter.related, vec!["[[a]]".to_string(), "[[b]]".to_string()]);
    }

    #[test]
    fn parse_then_serialize_then_parse_roundtrips() {
        let yaml = "title: X\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n";
        let content = format!("---\n{yaml}---\n\n# heading\n");
        let parsed = parse_page(&content).unwrap();
        let reserialized = serialize_page(&parsed.frontmatter, &parsed.body);
        let reparsed = parse_page(&reserialized).unwrap();
        assert_eq!(parsed.frontmatter, reparsed.frontmatter);
        assert_eq!(parsed.body, reparsed.body);
    }
}
