//! Prompts passed to the fix agent.
//!
//! The agent loads `codebus-fix` SKILL.md from `<vault>/.claude/skills/`
//! when its slash command is `/codebus-fix`. The slash command itself is
//! the prompt; SKILL.md content steers the workflow.
//!
//! For follow-up pings (`--resume <uuid>`), we pass the remaining lint
//! issues directly in the prompt body so the agent doesn't have to re-run
//! lint just to re-discover what's still broken.

use crate::wiki::lint::format_json;
use crate::wiki::types::LintResult;
use std::path::Path;

/// Initial slash command for the first fix-loop spawn. The agent loads
/// SKILL.md by skill name (`codebus-fix`); SKILL.md tells it to run
/// `codebus lint --format json` itself if no issues are pre-supplied.
pub fn initial_prompt() -> String {
    "/codebus-fix".to_string()
}

/// Follow-up prompt for `--resume <uuid>` pings. Embeds the current
/// remaining lint issues as JSON so the agent can act on them directly
/// without re-running lint.
///
/// `remaining` is the lint result the CLI got AFTER the previous round.
/// `vault_root` is the `.codebus/` directory (used to compute absolute
/// paths in the embedded JSON, mirroring `codebus lint --format json`).
pub fn followup_prompt(remaining: &LintResult, vault_root: &Path) -> String {
    let issues_json = format_json(remaining, vault_root)
        .unwrap_or_else(|_| "{\"issues\":[]}".to_string());
    format!(
        "/codebus-fix\n\n\
         The previous repair round left the following lint issues unresolved. \
         Apply one more round of repairs against these specific issues, then \
         exit. Do not re-run `codebus lint`; the issues below are authoritative \
         for this round:\n\n\
         ```json\n{issues_json}\n```\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::types::{LintIssue, LintSeverity};

    fn issue(path: &str, sev: LintSeverity, rule: &str, msg: &str) -> LintIssue {
        LintIssue {
            path: path.into(),
            severity: sev,
            rule_id: rule.into(),
            message: msg.into(),
        }
    }

    #[test]
    fn initial_prompt_is_just_slash_command() {
        assert_eq!(initial_prompt(), "/codebus-fix");
    }

    #[test]
    fn followup_prompt_starts_with_slash_command() {
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![issue("concepts/foo.md", LintSeverity::Warn, "broken-wikilink-body", "msg")],
            error_count: 0,
            warn_count: 1,
        };
        let p = followup_prompt(&result, Path::new("/v/.codebus"));
        assert!(p.starts_with("/codebus-fix"));
    }

    #[test]
    fn followup_prompt_embeds_remaining_issues_json() {
        let result = LintResult {
            pages_scanned: 1,
            nav_files_scanned: 0,
            issues: vec![issue("concepts/foo.md", LintSeverity::Error, "frontmatter-parse", "parse failed")],
            error_count: 1,
            warn_count: 0,
        };
        let p = followup_prompt(&result, Path::new("/v/.codebus"));
        assert!(p.contains("frontmatter-parse"));
        assert!(p.contains("concepts/foo.md"));
        assert!(p.contains("```json"));
    }

    #[test]
    fn followup_prompt_instructs_no_re_lint() {
        let result = LintResult {
            pages_scanned: 0,
            nav_files_scanned: 0,
            issues: vec![],
            error_count: 0,
            warn_count: 0,
        };
        let p = followup_prompt(&result, Path::new("/v/.codebus"));
        assert!(p.contains("authoritative"));
    }
}
