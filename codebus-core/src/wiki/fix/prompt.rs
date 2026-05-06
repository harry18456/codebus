//! Prompt construction for the lint-feedback-loop fix module.
//!
//! Each iteration's prompt batches all current lint issues into a single
//! XML-ish `<lint_issues>` block. From the second iteration onward we also
//! include a `<previous_attempt>` block whose body is the `git diff` of the
//! vault's `wiki/` subtree against the snapshot taken at loop start (see
//! [`crate::wiki::fix::memory::git_diff_summary`]).
//!
//! XML-ish tags are used because the agent's system prompt
//! (`schema/CLAUDE.md`) already speaks that style; nested `<issue>` entries
//! make per-rule fields machine-readable for the agent without committing
//! us to a strict schema.

use crate::wiki::types::{LintIssue, LintSeverity};

/// Build the user message for a fix iteration.
///
/// `prior_diff` is `None` on the first iteration (no previous attempt
/// exists) and `Some(diff)` thereafter. An empty `Some("")` is treated as
/// "no observable change since loop start" and therefore omitted from the
/// prompt body.
pub fn build_fix_prompt(issues: &[LintIssue], prior_diff: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("<lint_issues>\n");
    for issue in issues {
        out.push_str("  <issue path=\"");
        out.push_str(&xml_escape(&issue.path));
        out.push_str("\" severity=\"");
        out.push_str(severity_str(issue.severity));
        out.push_str("\">");
        out.push_str(&xml_escape(&issue.message));
        out.push_str("</issue>\n");
    }
    out.push_str("</lint_issues>\n\n");

    if let Some(diff) = prior_diff {
        if !diff.trim().is_empty() {
            out.push_str("<previous_attempt>\n");
            out.push_str("  <git_diff>\n");
            out.push_str(diff);
            if !diff.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("  </git_diff>\n");
            out.push_str("</previous_attempt>\n\n");
        }
    }

    out.push_str("<task>\n");
    out.push_str(FIX_TASK_INSTRUCTION);
    out.push_str("\n</task>\n");
    out
}

const FIX_TASK_INSTRUCTION: &str = "\
For each issue above, edit the corresponding wiki page or take the appropriate \
structural action (rename, move, merge). Use Read/Edit/Write/MultiEdit tools. \
Concrete fix hints by rule:\n\
- broken wikilink: either remove the link, or create the missing page in the \
  correct type folder; do not silently leave a dangling reference.\n\
- oversize page: split into multiple pages along natural section boundaries \
  and update cross-links so the original entry point still navigates.\n\
- missing nav (index.md / log.md): write a brief table-of-contents page \
  pointing at the existing wiki pages.\n\
- page in wiki/ root: move into one of concepts/ entities/ modules/ \
  processes/ synthesis/ based on the page's content.\n\
- frontmatter integrity: repair YAML structure and missing fields (title, \
  type, sources, goals, created, updated).\n\
- duplicate slug: pick which page to keep (or merge them) based on content; \
  rename or delete the loser; update inbound links.\n\
- unexpected file: classify by content and either move into the correct type \
  folder, rename to .md if it's prose, or delete if it doesn't belong.\n\n\
After your changes, the linter will re-run; if there are remaining issues, \
you will be asked to address them in the next iteration.";

fn severity_str(s: LintSeverity) -> &'static str {
    match s {
        LintSeverity::Error => "error",
        LintSeverity::Warn => "warn",
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(path: &str, severity: LintSeverity, message: &str) -> LintIssue {
        LintIssue {
            path: path.into(),
            severity,
            message: message.into(),
        }
    }

    #[test]
    fn first_iteration_has_no_previous_attempt_block() {
        // Spec scenario: "First iteration has no previous-attempt block"
        let issues = vec![issue(
            "concepts/foo.md",
            LintSeverity::Warn,
            "broken wikilink in body: [[ghost]]",
        )];
        let prompt = build_fix_prompt(&issues, None);
        assert!(prompt.contains("<lint_issues>"));
        assert!(prompt.contains("concepts/foo.md"));
        assert!(prompt.contains("[[ghost]]"));
        assert!(
            !prompt.contains("<previous_attempt>"),
            "first iteration must not include previous_attempt block"
        );
    }

    #[test]
    fn subsequent_iteration_includes_previous_attempt_block() {
        // Spec scenario: "Subsequent iterations include diff against the
        // snapshot taken at loop start"
        let issues = vec![issue("concepts/foo.md", LintSeverity::Warn, "still broken")];
        let diff = "diff --git a/wiki/concepts/foo.md b/wiki/concepts/foo.md\n\
                    --- a/wiki/concepts/foo.md\n\
                    +++ b/wiki/concepts/foo.md\n\
                    @@ -1 +1 @@\n\
                    -before\n\
                    +after\n";
        let prompt = build_fix_prompt(&issues, Some(diff));
        assert!(prompt.contains("<previous_attempt"));
        assert!(prompt.contains("<git_diff>"));
        assert!(prompt.contains("-before"));
        assert!(prompt.contains("+after"));
        assert!(prompt.contains("</previous_attempt>"));
    }

    #[test]
    fn prompt_lists_every_issue_in_order() {
        let issues = vec![
            issue("concepts/a.md", LintSeverity::Warn, "msg-a"),
            issue("entities/b.md", LintSeverity::Error, "msg-b"),
            issue("processes/c.md", LintSeverity::Warn, "msg-c"),
        ];
        let prompt = build_fix_prompt(&issues, None);
        let pos_a = prompt.find("concepts/a.md").expect("a present");
        let pos_b = prompt.find("entities/b.md").expect("b present");
        let pos_c = prompt.find("processes/c.md").expect("c present");
        assert!(pos_a < pos_b && pos_b < pos_c, "issues out of order");
        assert!(prompt.contains("msg-a"));
        assert!(prompt.contains("msg-b"));
        assert!(prompt.contains("msg-c"));
    }

    #[test]
    fn xml_special_chars_in_path_or_message_are_escaped() {
        let issues = vec![issue(
            "concepts/<weird>.md",
            LintSeverity::Warn,
            "contains & ampersand",
        )];
        let prompt = build_fix_prompt(&issues, None);
        assert!(prompt.contains("&lt;weird&gt;"));
        assert!(prompt.contains("&amp; ampersand"));
    }

    #[test]
    fn empty_prior_diff_is_omitted() {
        // An empty diff string means nothing observably changed; don't pad
        // the prompt with an empty block.
        let issues = vec![issue("concepts/foo.md", LintSeverity::Warn, "x")];
        let prompt = build_fix_prompt(&issues, Some(""));
        assert!(!prompt.contains("<previous_attempt>"));
    }

    #[test]
    fn task_block_appears_after_issues_and_diff() {
        let issues = vec![issue("concepts/foo.md", LintSeverity::Warn, "x")];
        let prompt = build_fix_prompt(&issues, Some("diff body"));
        let p_issues = prompt.find("<lint_issues>").unwrap();
        let p_prev = prompt.find("<previous_attempt>").unwrap();
        let p_task = prompt.find("<task>").unwrap();
        assert!(p_issues < p_prev && p_prev < p_task);
    }
}
