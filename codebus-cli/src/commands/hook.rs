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
use codebus_core::config::{HooksConfig, default_config_path, load_hooks_config};
use serde::Deserialize;
use std::io::{self, Read, Write};
use std::process::ExitCode;

#[derive(Subcommand, Debug)]
pub enum HookArgs {
    /// PreToolUse Bash hook: allow `codebus lint *` or
    /// `codebus quiz validate *`, block everything else. Reads JSON from
    /// stdin, prints decision JSON to stdout, always exits 0.
    CheckBash,
    /// PreToolUse Read hook: block image / binary file extensions whose
    /// contents would bypass the regex_basic PII filter. Reads JSON from
    /// stdin, prints decision JSON to stdout, always exits 0.
    CheckRead,
}

pub async fn run(args: HookArgs) -> ExitCode {
    match args {
        HookArgs::CheckBash => check_bash().await,
        HookArgs::CheckRead => check_read().await,
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
    #[serde(default)]
    file_path: Option<serde_json::Value>,
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

/// PreToolUse Read hook image / binary blocklist — file extensions whose
/// content would bypass `regex_basic` PII filtering (which only scans text).
/// Stored lowercase so callers compare against `to_ascii_lowercase()` output.
const IMAGE_BLOCKLIST: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "pdf", "ico", "heic", "heif",
    "avif",
];

/// Returns true when `path` ends in an extension on [`IMAGE_BLOCKLIST`].
///
/// Comparison is ASCII case-insensitive on every platform — this **deliberately
/// diverges** from [`is_codebus_binary`]'s OS-split behavior (Windows
/// case-insensitive, Unix case-sensitive). File extensions are conventionally
/// case-insensitive across all operating systems, and a POSIX case-sensitive
/// match would let `screenshot.PNG` bypass the blocklist on Linux. Do not
/// "fix" this to match `is_codebus_binary`'s pattern without first revisiting
/// the `pretooluse-image-block` change rationale.
///
/// Path separator handling: the directory portion is stripped using either
/// `/` or `\` as a separator so Unix paths, Windows native paths, and Windows
/// mixed-separator paths all yield the same extension.
fn is_image_path(path: &str) -> bool {
    let basename = path
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path);
    let Some(dot_pos) = basename.rfind('.') else {
        return false;
    };
    let ext = &basename[dot_pos + 1..];
    if ext.is_empty() {
        return false;
    }
    let ext_lower = ext.to_ascii_lowercase();
    IMAGE_BLOCKLIST.contains(&ext_lower.as_str())
}

/// Pure decision function for the PreToolUse Read hook — extracts the
/// block / allow decision from a stdin JSON body so the logic is unit
/// testable without subprocess stdin.
///
/// Returns `Some(reason)` when the agent's Read tool invocation MUST be
/// blocked (image extension hit OR fail-closed condition); `None` when
/// the Read invocation may proceed silently.
///
/// `hooks_cfg.read_image_block` gates the entire body (verify-stage-
/// independent-model-toggle change): when `false`, the function
/// short-circuits to `None` for ANY input — including malformed JSON,
/// empty stdin, and image extensions — because the user has explicitly
/// turned off the Read gate. When `true`, the function executes the
/// pre-toggle blocklist + fail-closed contract.
fn check_read_inner(stdin_body: &str, hooks_cfg: &HooksConfig) -> Option<String> {
    if !hooks_cfg.read_image_block {
        return None;
    }
    if stdin_body.trim().is_empty() {
        return Some("hook: empty stdin (no PreToolUse JSON received)".to_string());
    }
    let parsed: PreToolUseInput = match serde_json::from_str(stdin_body) {
        Ok(p) => p,
        Err(_) => return Some("hook: malformed PreToolUse JSON on stdin".to_string()),
    };
    let file_path_value = parsed.tool_input.as_ref().and_then(|t| t.file_path.as_ref());
    let path = match file_path_value {
        Some(serde_json::Value::String(s)) if !s.is_empty() => s.as_str(),
        _ => return Some("hook: tool_input.file_path absent or empty".to_string()),
    };
    if is_image_path(path) {
        return Some(format!(
            "hook: reading image / binary files is blocked to prevent PII bypass; received `{path}`"
        ));
    }
    None
}

async fn check_read() -> ExitCode {
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return emit_block("hook: failed to read stdin");
    }
    // verify-stage-independent-model-toggle: resolve the runtime gate
    // BEFORE running the blocklist. Any failure to load the config
    // (file absent, malformed yaml, missing section) falls back to
    // `HooksConfig::default()` — fail-safe to block. `default_config_path`
    // returning None (no resolvable home dir) is also treated as "no
    // config → default", preserving the pre-toggle behavior.
    let hooks_cfg = match default_config_path() {
        Some(p) => load_hooks_config(&p).unwrap_or_default(),
        None => HooksConfig::default(),
    };
    match check_read_inner(&buf, &hooks_cfg) {
        Some(reason) => emit_block(&reason),
        None => ExitCode::from(0),
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

    // --- pretooluse-image-block task 1.1 (RED) — is_image_path predicate.
    // Implementation lands in task 1.2; these tests pin the behavior
    // contract from the `PII Image Read Hook Installation` requirement.

    #[test]
    fn is_image_path_blocks_lowercase_extensions() {
        for ext in IMAGE_BLOCKLIST {
            let path = format!("foo.{ext}");
            assert!(is_image_path(&path), "expected block for {path}");
        }
    }

    #[test]
    fn is_image_path_blocks_uppercase_extensions() {
        // ASCII case-insensitive on every platform — Linux too.
        let cases = ["foo.PNG", "foo.JPG", "foo.JPEG", "foo.PDF", "foo.HEIC"];
        for path in cases {
            assert!(is_image_path(path), "expected block for {path}");
        }
    }

    #[test]
    fn is_image_path_blocks_mixed_case_extensions() {
        assert!(is_image_path("photo.Jpeg"));
        assert!(is_image_path("icon.Avif"));
        assert!(is_image_path("doc.PdF"));
    }

    #[test]
    fn is_image_path_allows_text_extensions() {
        let allowed = [
            "foo.md",
            "foo.rs",
            "foo.svg",
            "foo.txt",
            "foo.json",
            "foo.yaml",
            "foo.toml",
        ];
        for path in allowed {
            assert!(!is_image_path(path), "expected allow for {path}");
        }
    }

    #[test]
    fn is_image_path_allows_no_extension() {
        assert!(!is_image_path("Makefile"));
        assert!(!is_image_path("script"));
        assert!(!is_image_path(""));
    }

    #[test]
    fn is_image_path_allows_hidden_files_with_non_image_extension() {
        // `.gitignore` etc. — the leading dot is part of the basename, the
        // extension extractor sees `gitignore` and must not match.
        assert!(!is_image_path(".gitignore"));
        assert!(!is_image_path("/repo/.gitignore"));
    }

    #[test]
    fn is_image_path_handles_unix_path_separator() {
        assert!(is_image_path("/repo/assets/img.png"));
        assert!(is_image_path("./relative/photo.JPG"));
        assert!(!is_image_path("/repo/src/main.rs"));
    }

    #[test]
    fn is_image_path_handles_windows_path_separator() {
        assert!(is_image_path("C:\\repo\\assets\\img.png"));
        assert!(is_image_path("D:\\photos\\IMG_001.HEIC"));
        assert!(!is_image_path("C:\\repo\\src\\main.rs"));
    }

    #[test]
    fn is_image_path_handles_mixed_path_separator() {
        // Windows commonly mixes `/` and `\`.
        assert!(is_image_path("C:/repo\\assets/img.png"));
        assert!(is_image_path("D:\\photos/snapshot.PNG"));
    }

    #[test]
    fn is_image_path_only_considers_basename_extension() {
        // Directory components containing dots SHALL NOT bleed into the
        // extension match; only the filename's final `.ext` counts.
        assert!(!is_image_path("a/b.png/file.md"));
        assert!(!is_image_path("C:\\repo.git\\notes\\daily.md"));
    }

    // --- pretooluse-image-block task 1.3 (RED) — check_read fail-closed
    // contract. Implementation lands in task 1.4; these tests pin the
    // fail-closed branches required by the `PII Image Read Hook
    // Installation` requirement (the subcommand SHALL NEVER silently allow
    // on parse failure or missing fields).

    #[test]
    fn check_read_fail_closed_on_empty_stdin() {
        let reason = check_read_inner("", &HooksConfig::default());
        assert!(reason.is_some(), "empty stdin must block");
        assert!(
            reason.as_ref().unwrap().contains("empty"),
            "block reason must identify empty stdin, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_whitespace_only_stdin() {
        // Mirror check_bash's `buf.trim().is_empty()` semantics.
        assert!(check_read_inner("   \n\t  ", &HooksConfig::default()).is_some());
    }

    #[test]
    fn check_read_fail_closed_on_malformed_json() {
        let reason = check_read_inner("{not valid json", &HooksConfig::default());
        assert!(reason.is_some(), "malformed JSON must block");
        assert!(
            reason.as_ref().unwrap().contains("malformed"),
            "block reason must identify malformed JSON, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_missing_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{}}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "missing file_path must block");
        assert!(
            reason.as_ref().unwrap().contains("file_path"),
            "block reason must identify file_path absence, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_non_string_file_path() {
        // Numeric file_path — type confusion / fuzzing case.
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":123}}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "non-string file_path must block");
    }

    #[test]
    fn check_read_fail_closed_on_null_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":null}}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "null file_path must block");
    }

    #[test]
    fn check_read_fail_closed_on_empty_string_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":""}}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "empty-string file_path must block");
        assert!(
            reason.as_ref().unwrap().contains("file_path"),
            "block reason must identify empty file_path, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_missing_tool_input() {
        // Whole `tool_input` object missing.
        let body = r#"{"tool_name":"Read"}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "missing tool_input must block");
    }

    // --- pretooluse-image-block task 1.4 — check_read positive contract:
    // image extensions hit the blocklist; non-image extensions pass through.

    #[test]
    fn check_read_blocks_image_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        let reason = check_read_inner(body, &HooksConfig::default());
        assert!(reason.is_some(), "image extension must block");
        let reason_str = reason.unwrap();
        assert!(
            reason_str.contains("flow.png"),
            "block reason must echo the file_path, got: {reason_str}"
        );
        assert!(
            reason_str.contains("PII bypass") || reason_str.contains("image"),
            "block reason must surface the policy intent, got: {reason_str}"
        );
    }

    #[test]
    fn check_read_blocks_image_extension_uppercase() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/logo.JPG"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default()).is_some());
    }

    #[test]
    fn check_read_blocks_windows_path_image() {
        let body =
            r#"{"tool_name":"Read","tool_input":{"file_path":"C:\\repo\\assets\\img.png"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default()).is_some());
    }

    #[test]
    fn check_read_allows_text_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/modules/uv-lib.md"}}"#;
        assert!(
            check_read_inner(body, &HooksConfig::default()).is_none(),
            "text file must pass through"
        );
    }

    #[test]
    fn check_read_allows_source_code() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"codebus-core/src/agent/claude_cli.rs"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default()).is_none());
    }

    #[test]
    fn check_read_allows_svg() {
        // SVG is XML, scannable by regex_basic — deliberately NOT blocked.
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagram.svg"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default()).is_none());
    }

    #[test]
    fn check_read_allows_no_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"Makefile"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default()).is_none());
    }

    #[test]
    fn check_read_block_reason_is_valid_json_after_emit() {
        // Make sure the block reason survives JSON escaping with a path that
        // contains backslashes (Windows).
        let body =
            r#"{"tool_name":"Read","tool_input":{"file_path":"C:\\repo\\img.png"}}"#;
        let reason = check_read_inner(body, &HooksConfig::default()).expect("must block");
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape(&reason)
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&payload).expect("decision payload must parse as JSON");
        assert_eq!(parsed["decision"], "block");
        assert!(
            parsed["reason"]
                .as_str()
                .unwrap()
                .contains("img.png")
        );
    }

    // --- pretooluse-image-block-toggle task 2.1 (RED) ---
    //
    // `check_read_inner` gains a `hooks_cfg: &HooksConfig` parameter.
    // When `hooks_cfg.read_image_block` is false, the function MUST
    // return None for any input (image / non-image / malformed JSON /
    // empty / missing fields); the blocklist + fail-closed branches
    // SHALL NOT execute. When true, behavior matches the unmodified
    // `check_read_inner` contract — exercised by existing tests.

    #[test]
    fn check_read_config_off_allows_image_extension() {
        let cfg = HooksConfig {
            read_image_block: false,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        assert!(
            check_read_inner(body, &cfg).is_none(),
            "read_image_block=false must allow image extensions"
        );
    }

    #[test]
    fn check_read_config_off_allows_uppercase_image() {
        let cfg = HooksConfig {
            read_image_block: false,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/logo.JPG"}}"#;
        assert!(check_read_inner(body, &cfg).is_none());
    }

    #[test]
    fn check_read_config_off_allows_pdf() {
        let cfg = HooksConfig {
            read_image_block: false,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"docs/manual.pdf"}}"#;
        assert!(check_read_inner(body, &cfg).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_empty_stdin() {
        // When the gate is off, even empty stdin (which would normally
        // be fail-closed → block) MUST be allowed — the entire stdin
        // processing branch is short-circuited.
        let cfg = HooksConfig {
            read_image_block: false,
        };
        assert!(check_read_inner("", &cfg).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_malformed_json() {
        let cfg = HooksConfig {
            read_image_block: false,
        };
        assert!(check_read_inner("{not valid json", &cfg).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_missing_file_path() {
        let cfg = HooksConfig {
            read_image_block: false,
        };
        let body = r#"{"tool_name":"Read","tool_input":{}}"#;
        assert!(check_read_inner(body, &cfg).is_none());
    }

    #[test]
    fn check_read_config_on_blocks_image_like_before() {
        // Mirror the existing blocks_image_extension test but with the
        // explicit `read_image_block: true` config; behavior must be
        // identical to the pre-toggle implementation.
        let cfg = HooksConfig {
            read_image_block: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        let reason = check_read_inner(body, &cfg);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("flow.png"));
    }

    #[test]
    fn check_read_config_on_fails_closed_on_empty_stdin_like_before() {
        let cfg = HooksConfig {
            read_image_block: true,
        };
        let reason = check_read_inner("", &cfg);
        assert!(reason.is_some());
        assert!(reason.as_ref().unwrap().contains("empty"));
    }
}
