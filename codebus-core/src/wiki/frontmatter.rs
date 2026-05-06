use crate::wiki::types::{PageFrontmatter, ParsedPage, SourceRef};
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

/// Loose intermediate shape: parse YAML into `serde_yaml::Value`-friendly raw
/// fields, then validate per-field. Mirrors TS `parsePage` which throws
/// distinct errors for "Missing required field" vs "Invalid page type"
/// rather than a single serde-style error.
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

/// Parse a wiki knowledge page's `---\n<yaml>\n---\n<body>` shape into
/// frontmatter struct + body string. Body preserves the leading newline
/// after the closing delimiter (matching gray-matter's `content` field
/// in TS reference impl, verified against uv fixture roundtrip).
pub fn parse_page(content: &str) -> Result<ParsedPage, FrontmatterError> {
    let after_open = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
        .ok_or(FrontmatterError::MissingOpeningDelimiter)?;

    // Closing delimiter is `---` on its own line. Search for `\n---\n`,
    // `\n---\r\n`, or `\n---` immediately followed by EOF. Eat the leading
    // `\n` as part of the close delim (so yaml ends with no trailing nl).
    let (yaml, body) =
        split_at_closing(after_open).ok_or(FrontmatterError::MissingClosingDelimiter)?;

    let raw: RawFrontmatter = serde_yaml::from_str(yaml).map_err(FrontmatterError::YamlParse)?;

    let title = required_string(&raw.title, "title")?;
    let page_type_str = required_string(&raw.page_type, "type")?;
    let page_type = match page_type_str.as_str() {
        "concept" => crate::wiki::types::PageType::Concept,
        "entity" => crate::wiki::types::PageType::Entity,
        "module" => crate::wiki::types::PageType::Module,
        "process" => crate::wiki::types::PageType::Process,
        "synthesis" => crate::wiki::types::PageType::Synthesis,
        other => return Err(FrontmatterError::InvalidPageType(other.to_string())),
    };
    let sources = normalize_sources(&raw.sources);
    let goals = normalize_string_list(&raw.goals, "goals")?;
    let created = required_string(&raw.created, "created")?;
    let updated = required_string(&raw.updated, "updated")?;
    let related = normalize_string_list(&raw.related, "related")?;
    let stale = match &raw.stale {
        Some(serde_yaml::Value::Bool(b)) => *b,
        Some(_) => false, // TS coerces stale === true; non-bool → false
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

/// Serialize back to `---\n<yaml>\n---\n<body>` form. Body is appended
/// verbatim — caller is responsible for the leading newline (parse_page
/// preserves it).
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
    // Walk lines: yaml is everything up to the first line consisting solely
    // of `---` (with optional CR before LF). Body is everything after that
    // line's trailing newline, INCLUDING any subsequent leading newline.
    let mut byte_offset = 0usize;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" {
            // yaml is bytes [0..byte_offset), with the previous line's
            // terminating `\n` retained — strip exactly one trailing nl
            // for the yaml view.
            let yaml_end = byte_offset.saturating_sub(0);
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
        // For sequences/mappings we fall back to a YAML serialization;
        // TS coerces via String(...) which would give "[object Object]"
        // for objects but we never hit that path for the documented schema.
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
    use crate::wiki::types::PageType;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        // codebus-core/src/wiki/frontmatter.rs → 4 levels up = repo root
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/uv-vault-snapshot/uv-wiki-snapshot")
    }

    fn type_folders() -> [&'static str; 5] {
        ["concepts", "entities", "modules", "processes", "synthesis"]
    }

    #[test]
    fn parses_every_uv_fixture_page() {
        let root = fixture_root();
        let mut count = 0;
        for folder in type_folders() {
            let dir = root.join(folder);
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                let content = fs::read_to_string(entry.path()).unwrap();
                let parsed = parse_page(&content)
                    .unwrap_or_else(|e| panic!("failed to parse {:?}: {e}", entry.path()));
                assert!(!parsed.frontmatter.title.is_empty());
                assert!(!parsed.frontmatter.created.is_empty());
                assert!(!parsed.frontmatter.updated.is_empty());
                count += 1;
            }
        }
        assert!(count > 0, "no fixture pages found at {:?}", root);
    }

    #[test]
    fn body_preserves_leading_newline_after_closing_delim() {
        let content = "---\ntitle: X\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n\n# heading\n";
        let parsed = parse_page(content).unwrap();
        assert!(
            parsed.body.starts_with('\n'),
            "body should start with the post-delimiter newline, got: {:?}",
            &parsed.body
        );
        assert!(parsed.body.contains("# heading"));
    }

    #[test]
    fn parse_then_serialize_then_parse_roundtrips_to_equivalent_struct() {
        let root = fixture_root();
        for folder in type_folders() {
            let dir = root.join(folder);
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                let content = fs::read_to_string(entry.path()).unwrap();
                let parsed = parse_page(&content).unwrap();
                let reserialized = serialize_page(&parsed.frontmatter, &parsed.body);
                let reparsed = parse_page(&reserialized).unwrap_or_else(|e| {
                    panic!(
                        "reparse failed for {:?}: {e}\nreserialized:\n{reserialized}",
                        entry.path()
                    )
                });
                assert_eq!(
                    parsed.frontmatter,
                    reparsed.frontmatter,
                    "roundtrip frontmatter mismatch for {:?}",
                    entry.path()
                );
                assert_eq!(
                    parsed.body,
                    reparsed.body,
                    "roundtrip body mismatch for {:?}",
                    entry.path()
                );
            }
        }
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
    fn sources_with_only_path_parses_clean() {
        let content = "---\ntitle: x\ntype: concept\nsources:\n  - path: src/a.rs\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: false\n---\n";
        let p = parse_page(content).unwrap();
        assert_eq!(p.frontmatter.sources.len(), 1);
        assert_eq!(p.frontmatter.sources[0].path, "src/a.rs");
        assert_eq!(p.frontmatter.sources[0].sha256, None);
        assert_eq!(p.frontmatter.sources[0].at_commit, None);
    }

    #[test]
    fn related_with_quoted_wikilinks_parses_to_bracketed_strings() {
        let content = "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated:\n  - '[[a]]'\n  - '[[b]]'\nstale: false\n---\n";
        let p = parse_page(content).unwrap();
        assert_eq!(
            p.frontmatter.related,
            vec!["[[a]]".to_string(), "[[b]]".to_string()]
        );
    }

    #[test]
    fn stale_non_bool_coerces_to_false() {
        // TS does `data.stale === true` — anything else (including string
        // 'false', number 0, etc.) becomes false. Replicate that quirk.
        let content = "---\ntitle: x\ntype: concept\nsources: []\ngoals: []\ncreated: '2026-05-05'\nupdated: '2026-05-05'\nrelated: []\nstale: 'false'\n---\n";
        let p = parse_page(content).unwrap();
        assert!(!p.frontmatter.stale);
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
}
