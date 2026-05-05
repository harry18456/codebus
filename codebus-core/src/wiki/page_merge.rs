use crate::wiki::types::{ParsedPage, PageFrontmatter, SourceRef};

fn unique_sources(a: &[SourceRef], b: &[SourceRef]) -> Vec<SourceRef> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(a.len() + b.len());
    for s in a.iter().chain(b.iter()) {
        if seen.insert(s.path.clone()) {
            out.push(s.clone());
        }
    }
    out
}

fn unique_strings(lists: &[&[String]]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for list in lists {
        for s in list.iter() {
            if seen.insert(s.clone()) {
                out.push(s.clone());
            }
        }
    }
    out
}

/// Merge an existing page with incoming content from a new goal run.
///
/// Locked fields (preserved from existing): `title`, `type`, `created`, `stale`.
/// Array fields (sources, goals, related) are unioned in order, dedup by
/// `SourceRef.path` for sources and by full string for goals/related.
/// `goals` unions three lists per iter-8 review: existing + `[goal_text]`
/// + incoming.goals (the earlier impl forgot the third source).
/// `updated` is set to `today`. Body appends a `## from goal: <X> (YYYY-MM-DD)`
/// section after a blank line.
pub fn merge_page(existing: &ParsedPage, incoming: &ParsedPage, goal_text: &str, today: &str) -> ParsedPage {
    let sources = unique_sources(&existing.frontmatter.sources, &incoming.frontmatter.sources);
    let goal_text_vec = vec![goal_text.to_string()];
    let goals = unique_strings(&[
        &existing.frontmatter.goals,
        &goal_text_vec,
        &incoming.frontmatter.goals,
    ]);
    let related = unique_strings(&[&existing.frontmatter.related, &incoming.frontmatter.related]);

    let section_header = format!("## from goal: {goal_text} ({today})");
    let body = format!(
        "{}\n\n{}\n\n{}\n",
        existing.body.trim_end(),
        section_header,
        incoming.body.trim()
    );

    ParsedPage {
        frontmatter: PageFrontmatter {
            title: existing.frontmatter.title.clone(),
            page_type: existing.frontmatter.page_type,
            created: existing.frontmatter.created.clone(),
            sources,
            goals,
            related,
            updated: today.to_string(),
            stale: existing.frontmatter.stale,
        },
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::types::PageType;

    fn page(title: &str, sources: Vec<&str>, goals: Vec<&str>, related: Vec<&str>, body: &str) -> ParsedPage {
        ParsedPage {
            frontmatter: PageFrontmatter {
                title: title.into(),
                page_type: PageType::Concept,
                sources: sources.into_iter().map(|p| SourceRef { path: p.into(), sha256: None, at_commit: None }).collect(),
                goals: goals.into_iter().map(String::from).collect(),
                related: related.into_iter().map(String::from).collect(),
                created: "2026-05-01".into(),
                updated: "2026-05-01".into(),
                stale: false,
            },
            body: body.into(),
        }
    }

    #[test]
    fn locked_fields_preserved_from_existing() {
        let existing = page("Original Title", vec![], vec![], vec![], "# original\n");
        let mut incoming = page("Different Title", vec![], vec![], vec![], "# new\n");
        incoming.frontmatter.page_type = PageType::Module;
        incoming.frontmatter.created = "2099-01-01".into();
        incoming.frontmatter.stale = true;

        let merged = merge_page(&existing, &incoming, "goal", "2026-05-05");
        assert_eq!(merged.frontmatter.title, "Original Title");
        assert_eq!(merged.frontmatter.page_type, PageType::Concept);
        assert_eq!(merged.frontmatter.created, "2026-05-01");
        assert_eq!(merged.frontmatter.stale, false);
    }

    #[test]
    fn updated_set_to_today() {
        let existing = page("X", vec![], vec![], vec![], "");
        let incoming = page("X", vec![], vec![], vec![], "");
        let merged = merge_page(&existing, &incoming, "goal", "2026-05-05");
        assert_eq!(merged.frontmatter.updated, "2026-05-05");
    }

    #[test]
    fn sources_union_dedup_by_path_preserve_order() {
        let existing = page("X", vec!["a.rs", "b.rs"], vec![], vec![], "");
        let incoming = page("X", vec!["b.rs", "c.rs"], vec![], vec![], "");
        let merged = merge_page(&existing, &incoming, "g", "today");
        let paths: Vec<&str> = merged.frontmatter.sources.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(paths, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn goals_union_three_lists_existing_goaltext_incoming() {
        let existing = page("X", vec![], vec!["g1"], vec![], "");
        let incoming = page("X", vec![], vec!["g3"], vec![], "");
        let merged = merge_page(&existing, &incoming, "g2", "today");
        // iter-8 lesson: incoming.goals must NOT be dropped
        assert_eq!(merged.frontmatter.goals, vec!["g1".to_string(), "g2".into(), "g3".into()]);
    }

    #[test]
    fn goals_dedup_when_goal_text_already_in_existing() {
        let existing = page("X", vec![], vec!["g1", "g2"], vec![], "");
        let incoming = page("X", vec![], vec![], vec![], "");
        let merged = merge_page(&existing, &incoming, "g2", "today");
        assert_eq!(merged.frontmatter.goals, vec!["g1".to_string(), "g2".into()]);
    }

    #[test]
    fn related_unioned_dedup_preserve_order() {
        let existing = page("X", vec![], vec![], vec!["[[a]]", "[[b]]"], "");
        let incoming = page("X", vec![], vec![], vec!["[[b]]", "[[c]]"], "");
        let merged = merge_page(&existing, &incoming, "g", "today");
        assert_eq!(merged.frontmatter.related, vec!["[[a]]".to_string(), "[[b]]".into(), "[[c]]".into()]);
    }

    #[test]
    fn body_appends_section_header_with_goal_and_date() {
        let existing = page("X", vec![], vec![], vec![], "# heading\n\noriginal body\n");
        let incoming = page("X", vec![], vec![], vec![], "## new section\nincoming body");
        let merged = merge_page(&existing, &incoming, "explore X", "2026-05-05");
        assert!(merged.body.contains("# heading"));
        assert!(merged.body.contains("## from goal: explore X (2026-05-05)"));
        assert!(merged.body.contains("## new section"));
        assert!(merged.body.contains("incoming body"));
        // existing body's trailing whitespace removed before the join
        assert!(!merged.body.contains("body\n\n\n## from"));
    }

    #[test]
    fn body_format_exact_shape() {
        let existing = page("X", vec![], vec![], vec![], "alpha");
        let incoming = page("X", vec![], vec![], vec![], "beta");
        let merged = merge_page(&existing, &incoming, "g", "d");
        assert_eq!(merged.body, "alpha\n\n## from goal: g (d)\n\nbeta\n");
    }
}
