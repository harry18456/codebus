//! `broken_wikilink` rule — flag `[[wikilink]]` references that don't
//! resolve to a known slug. v3-lint Lint Rule Set rules 5 (related[]
//! resolution; Error) and 6 (body wikilinks; Warn).
//!
//! Body scan honors markdown code regions (fenced + inline) — wikilinks
//! inside ``` fences or `inline code` are treated as literal display text
//! per Obsidian rendering. Format violations in `related[]` (entries that
//! aren't `[[slug]]`-shape) are emitted by [`super::frontmatter_integrity`].

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static BODY_WIKILINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|#\s\\]+)(?:#[^\]|]+)?(?:\\?\|[^\]]+)?\]\]").unwrap());

static RELATED_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\[([^\]]+)\]\]\s*$").unwrap());

static FENCED_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());

static INLINE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]+`").unwrap());

pub struct BrokenWikilinkRule;

impl BrokenWikilinkRule {
    pub const NAME: &'static str = "broken_wikilink";
    pub fn new() -> Self {
        Self
    }
}

impl Default for BrokenWikilinkRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for BrokenWikilinkRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        let page_slugs = &ctx.catalog.page_slugs;

        for page in &ctx.pages {
            let Some(Ok(parsed)) = page.parsed_result.as_ref() else {
                continue;
            };

            for r in &parsed.frontmatter.related {
                let Some(caps) = RELATED_STRIP_REGEX.captures(r) else {
                    continue;
                };
                let slug = caps.get(1).unwrap().as_str().trim().to_string();
                if !page_slugs.contains(&slug) {
                    issues.push(LintIssue {
                        path: page.rel_path.clone(),
                        severity: LintSeverity::Error,
                        rule_id: "broken-wikilink-related".into(),
                        message: format!(
                            "broken wikilink in related: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                        ),
                    });
                }
            }

            scan_body(&parsed.body, &page.rel_path, page_slugs, &mut issues, "broken-wikilink-body");
        }

        for nav in &ctx.nav_files {
            let Some(content) = nav.content.as_ref() else {
                continue;
            };
            scan_body(content, nav.name, page_slugs, &mut issues, "broken-wikilink-nav");
        }

        issues
    }
}

fn scan_body(
    content: &str,
    rel_path: &str,
    page_slugs: &HashSet<String>,
    issues: &mut Vec<LintIssue>,
    rule_id: &'static str,
) {
    let stripped = strip_code_regions(content);
    let mut seen = HashSet::new();
    for caps in BODY_WIKILINK_REGEX.captures_iter(&stripped) {
        let slug = caps
            .get(1)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        if slug.is_empty() || !seen.insert(slug.clone()) {
            continue;
        }
        if !page_slugs.contains(&slug) {
            issues.push(LintIssue {
                path: rel_path.to_string(),
                severity: LintSeverity::Warn,
                rule_id: rule_id.into(),
                message: format!(
                    "broken wikilink in body: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                ),
            });
        }
    }
}

fn strip_code_regions(content: &str) -> String {
    let no_fenced = FENCED_REGEX.replace_all(content, "");
    let stripped = INLINE_REGEX.replace_all(&no_fenced, "");
    stripped.into_owned()
}
