//! Prompt for the fix agent.
//!
//! v3-fix-trust-agent: single-shot model, agent loads `codebus-fix` SKILL.md
//! when its slash command is `/codebus-fix`. Agent freely runs `codebus lint`
//! itself (subject to PreToolUse Bash hook) within its session — no
//! follow-up prompts injected by the CLI.

/// Slash command for the fix-loop spawn. The agent loads SKILL.md by skill
/// name (`codebus-fix`); SKILL.md tells it to invoke `codebus lint --format
/// json` itself to obtain the issue list.
pub fn initial_prompt() -> String {
    "/codebus-fix".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_prompt_is_just_slash_command() {
        assert_eq!(initial_prompt(), "/codebus-fix");
    }
}
