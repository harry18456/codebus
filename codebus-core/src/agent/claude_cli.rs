//! Spawn `claude -p` with the canonical sandbox flags + slash command +
//! cwd at the vault root, inherit stdio, return the child's exit status.
//!
//! Sandbox triple flag was verified by 2026-05-09 spike (`_pii-toolgate-spike`,
//! 5 cells): `--tools` is the real toolset hard gate; `--allowedTools` is the
//! redundant auto-approval safety net; `--permission-mode acceptEdits` is
//! mandatory in `-p` mode (without it, all writes silently deny because there
//! is no terminal to prompt). All three must be set together.

use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

/// Inputs for [`invoke`]. Caller (verb command modules) constructs this with
/// verb-specific values and a `'static` toolset slice; for v3-goal the
/// toolset is `&["Read","Glob","Grep","Write","Edit"]`.
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
    let toolset_csv = opts.toolset.join(",");

    Command::new(&claude_bin)
        .arg("-p")
        .arg(&opts.slash_command)
        .arg("--tools")
        .arg(&toolset_csv)
        .arg("--allowedTools")
        .arg(&toolset_csv)
        .arg("--permission-mode")
        .arg("acceptEdits")
        .current_dir(&opts.vault_root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn invoke_options_struct_carries_required_fields() {
        // Lock-in: the struct shape MUST stay tight enough that adding a
        // field is a deliberate cross-cutting design move (per design.md
        // "Module 形狀" — toolset stays `&'static`, no model/effort yet).
        let opts = InvokeAgentOptions {
            slash_command: "/codebus-goal \"x\"".into(),
            vault_root: PathBuf::from("/tmp/v"),
            toolset: &["Read", "Glob", "Grep"],
        };
        let InvokeAgentOptions {
            slash_command,
            vault_root,
            toolset,
        } = opts;
        assert_eq!(slash_command, "/codebus-goal \"x\"");
        assert_eq!(vault_root, PathBuf::from("/tmp/v"));
        assert_eq!(toolset, &["Read", "Glob", "Grep"]);
    }

    #[test]
    fn invoke_returns_io_error_when_binary_missing() {
        // Spawn against a deliberately-nonexistent binary. The exact error
        // kind varies by platform (NotFound on Unix, kind 0 on Windows
        // sometimes), but invoke MUST return Err — never silently succeed.
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
        });
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        assert!(r.is_err(), "expected spawn err, got {r:?}");
    }
}
