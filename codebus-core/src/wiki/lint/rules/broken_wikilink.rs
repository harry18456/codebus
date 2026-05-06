//! `broken_wikilink` rule — flag `[[wikilink]]` references that don't
//! resolve to a known slug.
//!
//! Body scan honors markdown code regions (fenced + inline) — wikilinks
//! inside ``` fences or `inline code` are treated as literal display text
//! per Obsidian rendering, so they don't get flagged.
//!
//! `related[]` entries are checked as Errors (frontmatter is structural);
//! body wikilinks are Warns (might be intentional foreshadowing).
//!
//! Format violations in `related[]` (entries that aren't `[[slug]]`-shape)
//! are emitted by [`super::frontmatter_integrity`], not here.

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

// Body wikilink — matches [[slug]], [[slug|display]], [[slug#heading]],
// [[slug#heading|display]]; captures slug only. Slug class excludes
// backslash so markdown table escapes `[[slug\|alias]]` parse with
// slug=`slug` (not `slug\`); the alias separator accepts either `|` or `\|`.
static BODY_WIKILINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]|#\s\\]+)(?:#[^\]|]+)?(?:\\?\|[^\]]+)?\]\]").unwrap());

// `[[slug]]` in a `related:` array entry. Whole-string match.
static RELATED_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\[([^\]]+)\]\]\s*$").unwrap());

// Fenced code block (greedy across lines).
static FENCED_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());

// Inline code span — single line, no embedded backticks.
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
                    // Format violation — emitted by frontmatter_integrity.
                    continue;
                };
                let slug = caps.get(1).unwrap().as_str().trim().to_string();
                if !page_slugs.contains(&slug) {
                    issues.push(LintIssue {
                        path: page.rel_path.clone(),
                        severity: LintSeverity::Error,
                        message: format!(
                            "broken wikilink in related: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                        ),
                    });
                }
            }

            scan_body(&parsed.body, &page.rel_path, page_slugs, &mut issues);
        }

        for nav in &ctx.nav_files {
            let Some(content) = nav.content.as_ref() else {
                continue;
            };
            scan_body(content, nav.name, page_slugs, &mut issues);
        }

        issues
    }
}

fn scan_body(
    content: &str,
    rel_path: &str,
    page_slugs: &HashSet<String>,
    issues: &mut Vec<LintIssue>,
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
                message: format!(
                    "broken wikilink in body: [[{slug}]] (no page named {slug}.md in any wiki/<type>/ folder)"
                ),
            });
        }
    }
}

/// Strip markdown code regions (fenced first, then inline) so [[wikilink]]
/// occurrences inside them are not scanned. Obsidian renders these as
/// literal text. Order matters: fenced before inline.
fn strip_code_regions(content: &str) -> String {
    let no_fenced = FENCED_REGEX.replace_all(content, "");
    let stripped = INLINE_REGEX.replace_all(&no_fenced, "");
    stripped.into_owned()
}
