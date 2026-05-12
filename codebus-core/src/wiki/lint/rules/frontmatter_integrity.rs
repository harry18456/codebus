//! `frontmatter_integrity` rule — surface YAML parse failures and
//! `related[]` format violations as Errors. Maps to v3-lint Lint Rule Set
//! rules 1 (frontmatter parse) and 4 (related[] format).

use crate::wiki::lint::rule::{LintRule, VaultContext};
use crate::wiki::types::{LintIssue, LintSeverity};
use regex::Regex;
use std::sync::LazyLock;

static RELATED_STRIP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\[([^\]]+)\]\]\s*$").unwrap());

pub struct FrontmatterIntegrityRule;

impl FrontmatterIntegrityRule {
    pub const NAME: &'static str = "frontmatter_integrity";
    pub fn new() -> Self {
        Self
    }
}

impl Default for FrontmatterIntegrityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl LintRule for FrontmatterIntegrityRule {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn check(&self, ctx: &VaultContext) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        for page in &ctx.pages {
            if let Err(e) = &page.content_result {
                issues.push(LintIssue {
                    path: page.rel_path.clone(),
                    severity: LintSeverity::Error,
                    rule_id: "file-read".into(),
                    message: format!("file read failed: {e}"),
                });
                continue;
            };
            match &page.parsed_result {
                Some(Err(e)) => {
                    issues.push(LintIssue {
                        path: page.rel_path.clone(),
                        severity: LintSeverity::Error,
                        rule_id: "frontmatter-parse".into(),
                        message: format!("frontmatter parse failed: {e}"),
                    });
                    continue;
                }
                Some(Ok(parsed)) => {
                    for r in &parsed.frontmatter.related {
                        if RELATED_STRIP_REGEX.captures(r).is_none() {
                            issues.push(LintIssue {
                                path: page.rel_path.clone(),
                                severity: LintSeverity::Error,
                                rule_id: "related-format".into(),
                                message: format!("related[] entry not in [[wikilink]] format: {r}"),
                            });
                        }
                    }
                }
                None => {}
            }
        }
        issues
    }
}
