//! Spawn `claude -p` with the canonical sandbox flags + slash command +
//! cwd at the vault root, inherit stdio, return the child's exit status.
//!
//! Sandbox triple flag was verified by 2026-05-09 spike (`_pii-toolgate-spike`,
//! 5 cells): `--tools` is the real toolset hard gate; `--allowedTools` is the
//! redundant auto-approval safety net; `--permission-mode acceptEdits` is
//! mandatory in `-p` mode (without it, all writes silently deny because there
//! is no terminal to prompt). All three must be set together.
//!
//! v3-fix-trust-agent simplification: dropped `SessionAction` + `--session-id`
//! / `--resume` flags. Single-shot fix model means no caller needs session
//! continuity helpers; if a future change does, re-introduce them then.

use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

/// Inputs for [`invoke`]. Caller (verb command modules) constructs this with
/// verb-specific values and a `'static` toolset slice.
pub struct InvokeAgentOptions {
    /// The slash command + arguments string passed to `claude -p`. Example
    /// for goal: `/codebus-goal "make a wiki for X"`.
    pub slash_command: String,
    /// Working directory for the spawned child. v3 always sets this to the
    /// `.codebus/` vault root so the agent's filesystem reach is naturally
    /// scoped (no `--add-dir`, no escape via cwd-relative `..`).
    pub vault_root: PathBuf,
    /// Toolset whitelist passed to BOTH `--tools` (hard gate) and
    /// `--allowedTools` (redundant safety net). Comma-joined when forming
    /// the args. `'static` because verbs hardcode the list at compile time.
    pub toolset: &'static [&'static str],
    /// Optional Bash permission specifier appended to the toolset CSV.
    /// Format: `Bash(<command-pattern>)` — e.g. `Bash(codebus lint *)` for
    /// the fix verb. `None` means no Bash access (goal / query default).
    /// Spike-verified 2026-05-09 against `claude --help` v2.1.137: the
    /// Tool(specifier) syntax is supported directly on `--allowedTools`.
    pub bash_whitelist: Option<&'static str>,
    /// Optional Claude CLI `--model` value forwarded as `--model <X>` in
    /// the spawned argv. Accepts an alias (e.g. `"sonnet"`, `"opus"`,
    /// `"haiku"`) or a full identifier (e.g. `"claude-opus-4-7"`); codebus
    /// does NOT validate against an enum so model upgrades require no code
    /// change. `None` omits the flag entirely (Claude CLI uses its default).
    pub model: Option<String>,
    /// Optional Claude CLI `--effort` value forwarded as `--effort <Y>`.
    /// Accepts strings the Claude CLI knows (`low` / `medium` / `high` /
    /// `xhigh` / `max` as of v2.1.137). `None` omits the flag.
    pub effort: Option<String>,
}

/// Spawn the configured `claude -p` child process and wait for it to exit.
///
/// The claude binary path is read from env var `CODEBUS_CLAUDE_BIN` (test
/// override hook for integration tests; production path: env unset, falls
/// back to the literal `"claude"` and relies on PATH lookup).
///
/// Stdin is closed (`Stdio::null`) so the child does not block on input.
/// Stdout and stderr inherit so the user sees agent progress in real time.
///
/// Returns the child's `ExitStatus` on successful spawn-and-wait. Errors
/// here mean the spawn itself failed (binary not found, fork error, etc.) —
/// the caller distinguishes spawn failure from agent non-zero exit.
pub fn invoke(opts: InvokeAgentOptions) -> io::Result<ExitStatus> {
    let claude_bin = std::env::var("CODEBUS_CLAUDE_BIN")
        .unwrap_or_else(|_| "claude".to_string());
    let tools_csv = build_tools_csv(opts.toolset, opts.bash_whitelist);
    let allowed_tools_csv = build_allowed_tools_csv(opts.toolset, opts.bash_whitelist);

    let mut cmd = Command::new(&claude_bin);
    cmd.arg("-p")
        .arg(&opts.slash_command)
        .arg("--tools")
        .arg(&tools_csv)
        .arg("--allowedTools")
        .arg(&allowed_tools_csv)
        .arg("--permission-mode")
        .arg("acceptEdits");

    // Per Agent Spawn Model and Effort Forwarding: append --model / --effort
    // when the corresponding config field has a value. None → flag omitted
    // entirely so Claude CLI applies its own default.
    if let Some(model) = opts.model.as_deref() {
        cmd.arg("--model").arg(model);
    }
    if let Some(effort) = opts.effort.as_deref() {
        cmd.arg("--effort").arg(effort);
    }

    cmd.current_dir(&opts.vault_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
}

/// Compose the `--tools` value: bare tool names (the toolset hard-gate).
/// When `bash_whitelist` is supplied, the bare `Bash` token is added to
/// the toolset so the agent has Bash in its toolset; the restriction
/// pattern itself goes into `--allowedTools` (see [`build_allowed_tools_csv`]).
///
/// Spike-verified 2026-05-09 against `claude --help` v2.1.137: passing
/// `Bash(codebus lint *)` to `--tools` does NOT grant Bash access — the
/// agent reports "no Bash tool available". The fine-grained pattern only
/// belongs in `--allowedTools` (or settings.json `permissions.allow`).
pub(crate) fn build_tools_csv(
    toolset: &[&str],
    bash_whitelist: Option<&str>,
) -> String {
    let mut parts: Vec<&str> = toolset.to_vec();
    if bash_whitelist.is_some() {
        parts.push("Bash");
    }
    parts.join(",")
}

/// Compose the `--allowedTools` value: bare tool names (auto-approval) plus
/// any fine-grained permission specifiers. When `bash_whitelist` is supplied,
/// only the restricted pattern (e.g. `Bash(codebus lint *)`) is included —
/// not the bare `Bash` — so commands outside the pattern still trigger the
/// permission prompt (which fails in `-p` non-interactive mode, denying
/// the unwanted Bash invocation).
pub(crate) fn build_allowed_tools_csv(
    toolset: &[&str],
    bash_whitelist: Option<&str>,
) -> String {
    match bash_whitelist {
        None => toolset.join(","),
        Some(spec) if toolset.is_empty() => spec.to_string(),
        Some(spec) => format!("{},{}", toolset.join(","), spec),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn invoke_options_struct_carries_required_fields() {
        let opts = InvokeAgentOptions {
            slash_command: "/codebus-goal \"x\"".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read", "Glob", "Grep"],
            bash_whitelist: None,
            model: None,
            effort: None,
        };
        let InvokeAgentOptions {
            slash_command,
            vault_root,
            toolset,
            bash_whitelist,
            model,
            effort,
        } = opts;
        assert_eq!(slash_command, "/codebus-goal \"x\"");
        assert_eq!(vault_root, PathBuf::from("/tmp/v"));
        assert_eq!(toolset, &["Read", "Glob", "Grep"]);
        assert!(bash_whitelist.is_none());
        assert!(model.is_none());
        assert!(effort.is_none());
    }

    #[test]
    fn build_tools_csv_no_bash_whitelist() {
        let csv = build_tools_csv(&["Read", "Glob", "Grep"], None);
        assert_eq!(csv, "Read,Glob,Grep");
    }

    #[test]
    fn build_tools_csv_appends_bare_bash_when_whitelist_supplied() {
        let csv = build_tools_csv(
            &["Read", "Glob", "Grep", "Write", "Edit"],
            Some("Bash(codebus lint *)"),
        );
        assert_eq!(csv, "Read,Glob,Grep,Write,Edit,Bash");
    }

    #[test]
    fn build_allowed_tools_csv_no_bash_whitelist() {
        let csv = build_allowed_tools_csv(&["Read", "Glob", "Grep"], None);
        assert_eq!(csv, "Read,Glob,Grep");
    }

    #[test]
    fn build_allowed_tools_csv_appends_restricted_bash_pattern() {
        let csv = build_allowed_tools_csv(
            &["Read", "Glob", "Grep", "Write", "Edit"],
            Some("Bash(codebus lint *)"),
        );
        assert_eq!(csv, "Read,Glob,Grep,Write,Edit,Bash(codebus lint *)");
    }

    #[test]
    fn build_csvs_diverge_on_bash_when_whitelist_supplied() {
        let toolset = &["Read", "Glob", "Grep", "Write", "Edit"];
        let whitelist = Some("Bash(codebus lint *)");
        let tools = build_tools_csv(toolset, whitelist);
        let allowed = build_allowed_tools_csv(toolset, whitelist);
        assert_ne!(tools, allowed);
        assert!(tools.ends_with(",Bash"));
        assert!(allowed.ends_with(",Bash(codebus lint *)"));
    }

    #[test]
    fn invoke_returns_io_error_when_binary_missing() {
        unsafe {
            std::env::set_var(
                "CODEBUS_CLAUDE_BIN",
                "/nonexistent/path/to/no-such-claude-binary-xyz",
            );
        }
        let r = invoke(InvokeAgentOptions {
            slash_command: "/x".into(),
            vault_root: std::env::temp_dir(),
            toolset: &["Read"],
            bash_whitelist: None,
            model: None,
            effort: None,
        });
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        assert!(r.is_err(), "expected spawn err, got {r:?}");
    }

    /// Spec: "Agent Spawn Model and Effort Forwarding" — when both fields
    /// are Some, both flags appear on argv. We use a binary that prints argv
    /// to capture-friendly stderr (printenv-like). Cross-platform-safe path:
    /// invoke a deliberately-missing binary and rely on the fact that even
    /// though spawn fails, the Command construction has already happened —
    /// so we instead test the argv-construction logic at the unit level by
    /// calling the helper function that builds it. But invoke() doesn't
    /// expose that. Pragmatic alternative: smoke-test via a real binary
    /// proxy is outside this unit test's scope. Here we structurally assert
    /// the field plumbing through the spawn path by setting CODEBUS_CLAUDE_BIN
    /// to an executable echo wrapper if available, otherwise skip.
    /// For now we assert the struct shape; argv-content verification lives
    /// in the CLI integration tests (goal_flow / query_flow / fix_flow)
    /// which have access to a real wrapper binary.
    #[test]
    fn invoke_options_accepts_model_and_effort_some() {
        let opts = InvokeAgentOptions {
            slash_command: "/x".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read"],
            bash_whitelist: None,
            model: Some("opus".into()),
            effort: Some("high".into()),
        };
        assert_eq!(opts.model.as_deref(), Some("opus"));
        assert_eq!(opts.effort.as_deref(), Some("high"));
    }
}
