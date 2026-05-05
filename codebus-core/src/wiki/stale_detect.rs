use crate::wiki::types::PageFrontmatter;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleResult {
    pub is_stale: bool,
    pub changed_sources: Vec<String>,
}

/// Compare frontmatter `sources[].sha256` with current raw hashes.
/// A source is "changed" when the current raw file is missing OR its hash
/// differs from the recorded one. A page is stale iff at least one source
/// is changed.
///
/// CRITICAL (iter-8 review lesson): callers must only enrich pages that
/// are missing sha256/at_commit; rewriting hashes unconditionally compares
/// same-vs-same forever and breaks this signal.
pub fn detect_stale_sources(fm: &PageFrontmatter, current_hashes: &HashMap<String, String>) -> StaleResult {
    let mut changed = Vec::new();
    for src in &fm.sources {
        let current = current_hashes.get(&src.path);
        let recorded = src.sha256.as_deref();
        let same = match (current, recorded) {
            (Some(c), Some(r)) => c == r,
            _ => false,
        };
        if !same {
            changed.push(src.path.clone());
        }
    }
    StaleResult {
        is_stale: !changed.is_empty(),
        changed_sources: changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::types::{PageType, SourceRef};

    fn fm(sources: Vec<(&str, Option<&str>)>) -> PageFrontmatter {
        PageFrontmatter {
            title: "X".into(),
            page_type: PageType::Concept,
            sources: sources
                .into_iter()
                .map(|(p, h)| SourceRef { path: p.into(), sha256: h.map(String::from), at_commit: None })
                .collect(),
            goals: vec![],
            related: vec![],
            created: "2026-05-01".into(),
            updated: "2026-05-01".into(),
            stale: false,
        }
    }

    fn hashes(items: &[(&str, &str)]) -> HashMap<String, String> {
        items.iter().map(|(p, h)| ((*p).into(), (*h).into())).collect()
    }

    #[test]
    fn all_hashes_match_returns_not_stale() {
        let fm = fm(vec![("a.rs", Some("hash-a")), ("b.rs", Some("hash-b"))]);
        let cur = hashes(&[("a.rs", "hash-a"), ("b.rs", "hash-b")]);
        let r = detect_stale_sources(&fm, &cur);
        assert_eq!(r.is_stale, false);
        assert!(r.changed_sources.is_empty());
    }

    #[test]
    fn one_hash_diff_marks_stale_with_only_that_path() {
        let fm = fm(vec![("a.rs", Some("hash-a")), ("b.rs", Some("hash-b"))]);
        let cur = hashes(&[("a.rs", "hash-a"), ("b.rs", "DIFFERENT")]);
        let r = detect_stale_sources(&fm, &cur);
        assert_eq!(r.is_stale, true);
        assert_eq!(r.changed_sources, vec!["b.rs".to_string()]);
    }

    #[test]
    fn missing_current_hash_counts_as_changed() {
        let fm = fm(vec![("a.rs", Some("hash-a"))]);
        let cur = hashes(&[]);
        let r = detect_stale_sources(&fm, &cur);
        assert_eq!(r.is_stale, true);
        assert_eq!(r.changed_sources, vec!["a.rs".to_string()]);
    }

    #[test]
    fn missing_recorded_sha256_counts_as_changed() {
        // Page that hasn't been enriched yet (sha256 is None) — must NOT
        // accidentally compare-equal to a missing current and be "fresh".
        let fm = fm(vec![("a.rs", None)]);
        let cur = hashes(&[("a.rs", "hash-a")]);
        let r = detect_stale_sources(&fm, &cur);
        assert_eq!(r.is_stale, true);
    }

    #[test]
    fn empty_sources_is_not_stale() {
        let fm = fm(vec![]);
        let cur = hashes(&[]);
        let r = detect_stale_sources(&fm, &cur);
        assert_eq!(r.is_stale, false);
        assert!(r.changed_sources.is_empty());
    }
}
