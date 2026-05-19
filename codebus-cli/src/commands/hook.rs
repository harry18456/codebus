//! `codebus hook check-bash` — PreToolUse Bash hook for the fix sandbox.
//!
//! Internal interface, NOT a user-facing surface. Hidden from `--help`
//! via `#[command(hide = true)]` on the parent `Hook` enum variant.
//!
//! Per v3-fix-trust-agent's `Fix Bash Hook Installation` requirement:
//! - Stdin: PreToolUse hook JSON, e.g.
//!   `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"},...}`
//! - Allow: command's first argv token resolves to the codebus binary
//!   (`codebus` or `codebus.exe` basename, case-insensitive on Windows)
//!   AND the second argv token is exactly `lint`. Exit 0, no decision JSON.
//! - Block: anything else (other binary, wrong subcommand, parse error,
//!   missing fields). Exit 0 with stdout JSON `{"decision":"block","reason":"<msg>"}`.
//! - Fail-closed default — never silently allow on parse failure.

use clap::Subcommand;
use serde::Deserialize;
use std::io::{self, Read, Write};
use std::process::ExitCode;

#[derive(Subcommand, Debug)]
pub enum HookArgs {
    /// PreToolUse Bash hook: allow `codebus lint *` or
    /// `codebus quiz validate *`, block everything else. Reads JSON from
    /// stdin, prints decision JSON to stdout, always exits 0.
    CheckBash,
}

pub async fn run(args: HookArgs) -> ExitCode {
    match args {
        HookArgs::CheckBash => check_bash().await,
    }
}

async fn check_bash() -> ExitCode {
    // Read full stdin. Empty / unread stdin is a fail-closed condition.
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return emit_block("hook: failed to read stdin");
    }
    if buf.trim().is_empty() {
        return emit_block("hook: empty stdin (no PreToolUse JSON received)");
    }

    let parsed: PreToolUseInput = match serde_json::from_str(&buf) {
        Ok(p) => p,
        Err(_) => return emit_block("hook: malformed PreToolUse JSON on stdin"),
    };

    let cmd = parsed
        .tool_input
        .as_ref()
        .and_then(|t| t.command.as_deref())
        .unwrap_or("");
    if cmd.is_empty() {
        return emit_block("hook: tool_input.command absent or empty");
    }

    if is_allowed_bash_command(cmd) {
        // Allow: exit 0 with no decision JSON.
        ExitCode::from(0)
    } else {
        emit_block(&format!(
            "hook: only `codebus lint *` or `codebus quiz validate *` is permitted by the codebus agent sandbox; received `{cmd}`"
        ))
    }
}

/// PreToolUse hook input — minimal shape; unknown fields silently dropped.
#[derive(Deserialize)]
struct PreToolUseInput {
    #[serde(default)]
    #[allow(dead_code)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Option<ToolInput>,
}

#[derive(Deserialize)]
struct ToolInput {
    #[serde(default)]
    command: Option<String>,
}

/// Per `Fix Bash Hook Installation` allow rule:
///   - argv[0] basename (without directory) is exactly `codebus` (Unix
///     case-sensitive) or `codebus.exe` / `codebus.EXE` / etc. (Windows
///     case-insensitive)
///   - argv[1] is exactly `lint`
fn is_codebus_lint_command(cmd: &str) -> bool {
    let mut parts = cmd.split_whitespace();
    let Some(binary) = parts.next() else {
        return false;
    };
    if !is_codebus_binary(binary) {
        return false;
    }
    matches!(parts.next(), Some("lint"))
}

/// `codebus quiz validate ...` — the codebus-quiz generate agent's
/// self-validation form (spec `lint-feedback-loop` / Fix Bash Hook
/// Installation, allow rule (b)). Same argv strictness as the lint
/// form: binary basename must be `codebus`, then exactly `quiz` then
/// `validate`. `codebus quiz "<topic>"` (generate) does NOT match.
fn is_codebus_quiz_validate_command(cmd: &str) -> bool {
    let mut parts = cmd.split_whitespace();
    let Some(binary) = parts.next() else {
        return false;
    };
    if !is_codebus_binary(binary) {
        return false;
    }
    matches!(parts.next(), Some("quiz")) && matches!(parts.next(), Some("validate"))
}

/// Combined PreToolUse allow predicate: the codebus-fix agent's
/// `codebus lint ...` OR the codebus-quiz generate agent's
/// `codebus quiz validate ...`. Everything else is blocked.
fn is_allowed_bash_command(cmd: &str) -> bool {
    is_codebus_lint_command(cmd) || is_codebus_quiz_validate_command(cmd)
}

fn is_codebus_binary(token: &str) -> bool {
    // Strip directory portion — handle both `/` and `\` separators so this
    // works on Unix paths AND Windows mixed paths (e.g. `D:/x/codebus.exe`).
    let basename = token
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(token);

    if cfg!(target_os = "windows") {
        // Case-insensitive on Windows: `codebus`, `codebus.exe`, `Codebus.EXE`.
        let lower = basename.to_ascii_lowercase();
        lower == "codebus" || lower == "codebus.exe"
    } else {
        // Case-sensitive on Unix.
        basename == "codebus"
    }
}

fn emit_block(reason: &str) -> ExitCode {
    let payload = format!(
        "{{\"decision\":\"block\",\"reason\":{}}}",
        json_escape(reason)
    );
    let _ = writeln!(io::stdout(), "{payload}");
    ExitCode::from(0)
}

/// Minimal JSON string escape — covers the chars we'd emit in our reason
/// strings (no need for full Unicode handling; reasons are ASCII / simple).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_bare_codebus_lint() {
        assert!(is_codebus_lint_command("codebus lint"));
        assert!(is_codebus_lint_command("codebus lint --format json"));
        assert!(is_codebus_lint_command("codebus lint --repo /some/path"));
    }

    // --- quiz-validate-repair task 3.3: hook also allows
    // `codebus quiz validate ...` (spec lint-feedback-loop / Fix Bash
    // Hook Installation, new allow form). `is_allowed_bash_command` is
    // the combined predicate; `codebus lint *` OR `codebus quiz
    // validate *` is allowed, everything else blocked.

    #[test]
    fn allow_codebus_quiz_validate() {
        assert!(is_allowed_bash_command("codebus quiz validate -"));
        assert!(is_allowed_bash_command(
            "codebus quiz validate draft.md --json"
        ));
        if !cfg!(target_os = "windows") {
            assert!(is_allowed_bash_command(
                "/usr/local/bin/codebus quiz validate draft.md"
            ));
        }
    }

    #[test]
    fn block_codebus_quiz_generate_form() {
        // `codebus quiz "topic"` (generate) is NOT the validate
        // sub-action and MUST stay blocked.
        assert!(!is_allowed_bash_command("codebus quiz topic"));
        assert!(!is_allowed_bash_command("codebus quiz \"some topic\""));
        assert!(!is_allowed_bash_command("codebus quiz"));
    }

    #[test]
    fn combined_predicate_keeps_lint_allow_and_other_block() {
        assert!(is_allowed_bash_command("codebus lint --format json"));
        assert!(!is_allowed_bash_command("codebus fix --no-fix"));
        assert!(!is_allowed_bash_command("echo MARKER"));
        assert!(!is_allowed_bash_command(""));
    }

    #[test]
    fn allow_codebus_via_unix_absolute_path() {
        if !cfg!(target_os = "windows") {
            assert!(is_codebus_lint_command("/usr/local/bin/codebus lint"));
            assert!(is_codebus_lint_command(
                "/home/user/.cargo/bin/codebus lint --format json"
            ));
        }
    }

    #[test]
    fn allow_codebus_exe_via_windows_path() {
        if cfg!(target_os = "windows") {
            assert!(is_codebus_lint_command("D:/dev/codebus.exe lint"));
            assert!(is_codebus_lint_command(
                "D:\\dev\\codebus.exe lint --repo C:\\repo"
            ));
            assert!(is_codebus_lint_command("D:/dev/codebus.EXE lint"));
            assert!(is_codebus_lint_command("D:/dev/Codebus.exe lint"));
        }
    }

    #[test]
    fn block_non_codebus_binary() {
        assert!(!is_codebus_lint_command("echo MARKER"));
        assert!(!is_codebus_lint_command("rm -rf /tmp/x"));
        assert!(!is_codebus_lint_command("git status"));
        assert!(!is_codebus_lint_command("cargo lint"));
        assert!(!is_codebus_lint_command("/usr/bin/echo lint"));
    }

    #[test]
    fn block_codebus_other_subcommands() {
        assert!(!is_codebus_lint_command("codebus init"));
        assert!(!is_codebus_lint_command("codebus goal hello"));
        assert!(!is_codebus_lint_command("codebus fix"));
        assert!(!is_codebus_lint_command("codebus query something"));
        assert!(!is_codebus_lint_command("codebus hook check-bash"));
    }

    #[test]
    fn block_codebus_alone_no_subcommand() {
        // Per spec: argv[1] MUST be exactly `lint` — bare `codebus` is blocked.
        assert!(!is_codebus_lint_command("codebus"));
        assert!(!is_codebus_lint_command("codebus "));
    }

    #[test]
    fn block_lookalikes() {
        // Names that contain `codebus` but aren't exactly the basename.
        assert!(!is_codebus_lint_command("codebusx lint"));
        assert!(!is_codebus_lint_command("xcodebus lint"));
        assert!(!is_codebus_lint_command("codebus-fake lint"));
    }

    #[test]
    fn block_empty_or_whitespace_only_command() {
        assert!(!is_codebus_lint_command(""));
        assert!(!is_codebus_lint_command("   "));
        assert!(!is_codebus_lint_command("\t\n"));
    }

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape("hi"), "\"hi\"");
        assert_eq!(json_escape("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_escape("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_escape("line1\nline2"), "\"line1\\nline2\"");
    }

    #[test]
    fn block_emits_valid_decision_json() {
        // We can't easily capture stdout in unit tests, but verify the
        // payload format that emit_block constructs.
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape("test message")
        );
        assert_eq!(
            payload,
            "{\"decision\":\"block\",\"reason\":\"test message\"}"
        );
        // Confirm the payload parses as JSON.
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["decision"], "block");
        assert_eq!(parsed["reason"], "test message");
    }

    #[test]
    fn block_reason_with_command_containing_quotes_stays_valid_json() {
        let cmd_with_quote = r#"echo "hello""#;
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape(&format!(
                "hook: only `codebus lint *` is permitted by codebus fix sandbox; received `{cmd_with_quote}`"
            ))
        );
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["decision"], "block");
        assert!(parsed["reason"].as_str().unwrap().contains("hello"));
    }
}
