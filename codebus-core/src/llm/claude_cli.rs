//! `claude -p` subprocess provider — the Phase 1 [`LlmProvider`] impl.
//!
//! Sandbox semantics (iter-9 spike-verified, hard-won lesson):
//!
//! - `--permission-mode acceptEdits`: auto-accepts every tool in the toolset
//!   under `-p` mode (no terminal to deny prompts). Confirmed by manual test
//!   that Bash calls succeeded when the toolset still contained Bash, even
//!   under `--allowedTools='Read,Glob,Grep'`. So `acceptEdits` alone is NOT
//!   a sandbox.
//! - `--tools <list>`: WHITELIST. Tools not in this list are not visible to
//!   the agent at all. THIS is the lever that keeps Bash / WebFetch / future
//!   tools out. Iter-1 through iter-8 wrongly relied on `--allowedTools`.
//! - `--allowedTools <list>`: redundant safety net mirroring `--tools` so
//!   future Claude Code permission-mode changes don't accidentally hang on
//!   a prompt with no terminal.
//! - `cwd = .codebus/` (system-level isolation from user source repo,
//!   confirmed by spike E). No `--add-dir`: it widens, not narrows.

use crate::llm::provider::{EventStream, InvokeOptions, LlmMode, LlmProvider, ProviderError};
use crate::stream::{parse_claude_stream_line, StreamEvent};
use futures_util::StreamExt;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio_stream::wrappers::LinesStream;

/// Tools every mode allows: read-only discovery + read.
pub const ALWAYS_ALLOWED: &[&str] = &["Read", "Glob", "Grep"];

/// Additional tools allowed only in [`LlmMode::Ingest`] (writes wiki pages).
pub const INGEST_EXTRA: &[&str] = &["Write", "Edit"];

/// Tools that MUST never reach the agent under any mode. Sentinel set used
/// by the iter-9 negative assertion test — if any of these ever appears in
/// argv, the sandbox is broken.
pub const FORBIDDEN_TOOLS: &[&str] = &["Bash", "WebFetch", "WebSearch", "TodoWrite", "NotebookEdit", "BashOutput", "KillShell"];

/// Build the argv passed to `claude -p`. Pure function; tests pin both the
/// positive list (every allowed tool present) and the negative list (no
/// forbidden tool anywhere).
pub fn build_argv(mode: LlmMode, _vault_root: &Path) -> Vec<String> {
    let mut allowed: Vec<&str> = ALWAYS_ALLOWED.to_vec();
    if matches!(mode, LlmMode::Ingest) {
        allowed.extend_from_slice(INGEST_EXTRA);
    }
    let list = allowed.join(",");
    vec![
        "-p".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--permission-mode".into(),
        "acceptEdits".into(),
        "--tools".into(),
        list.clone(),
        "--allowedTools".into(),
        list,
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitVerdict {
    Success,
    OauthNeeded,
    GenericError,
}

/// Classify the exit code + stderr buffer into one of three verdicts.
/// Mirrors TS `classifyExit`. The OAuth heuristic (regex over stderr) is
/// the same — if it ever drifts, both impls have to update together.
pub fn classify_exit(code: i32, stderr: &str) -> ExitVerdict {
    if code == 0 {
        return ExitVerdict::Success;
    }
    let lower = stderr.to_lowercase();
    if lower.contains("unauthen")
        || lower.contains("authentication")
        || lower.contains("authenticated")
        || lower.contains("authentic")
        || lower.contains("token")
        || lower.contains("login")
    {
        return ExitVerdict::OauthNeeded;
    }
    ExitVerdict::GenericError
}

/// Concrete [`LlmProvider`] backed by the `claude` CLI subprocess.
/// Sandbox argv built by [`build_argv`]; spawn cwd = `opts.cwd` provides
/// system-level isolation per spike E.
pub struct ClaudeCliProvider {
    binary: String,
}

impl ClaudeCliProvider {
    pub fn new() -> Self {
        Self { binary: "claude".into() }
    }

    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self { binary: binary.into() }
    }
}

impl Default for ClaudeCliProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LlmProvider for ClaudeCliProvider {
    async fn invoke(&self, opts: InvokeOptions) -> Result<EventStream, ProviderError> {
        let argv = build_argv(opts.mode, &opts.vault_root);

        let mut child = Command::new(&self.binary)
            .args(&argv)
            .current_dir(&opts.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ProviderError::Setup {
                message: format!(
                    "Failed to spawn `{binary}`: {e}. Install Claude CLI from https://claude.ai/code and verify it is on PATH.",
                    binary = self.binary
                ),
            })?;

        let mut stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        // Build the single user-turn message. Real schema:
        // {type:"user", message:{role:"user", content:"..."}}.
        let payload = serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": format!("{}\n\n{}", opts.system_prompt, opts.user_message)
            }
        });
        let line = format!("{payload}\n");

        tokio::spawn(async move {
            let _ = stdin.write_all(line.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });

        let line_stream = LinesStream::new(BufReader::new(stdout).lines());
        let event_stream = line_stream
            .flat_map(|line_result| {
                let events = line_result
                    .ok()
                    .map(|l| parse_claude_stream_line(&l))
                    .unwrap_or_default();
                futures_util::stream::iter(events)
            })
            .chain(futures_util::stream::once(async move {
                // Reap the child so it's not zombified. We ignore the exit
                // verdict here — Phase C goal/query commands inspect their
                // own provider stream and stderr separately to surface
                // OAuth / generic-error verdicts to the user. The Done
                // event terminates the stream cleanly regardless.
                let _ = child.wait().await;
                StreamEvent::Done
            }));

        Ok(Box::pin(event_stream))
    }

    fn cancel(&self) {
        // Cancellation is realized by dropping the consumed stream — the
        // chained `child.wait()` future drops with it, and the Child's
        // `kill_on_drop(true)` setting sends SIGTERM. If a future caller
        // needs an explicit out-of-band cancel handle, store the Child in
        // an Arc<Mutex<Option<Child>>> on self and SIGTERM here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // === iter-9 sandbox argv tests — DOUBLE assertions per task 3.10 ===

    #[test]
    fn ingest_argv_contains_full_allowed_set() {
        let argv = build_argv(LlmMode::Ingest, &PathBuf::from("/v"));
        let joined = argv.join(" ");
        // Both --tools and --allowedTools mirror each other and contain
        // the same list of allowed names (iter-9 redundant safety net).
        assert!(joined.contains("--tools Read,Glob,Grep,Write,Edit"));
        assert!(joined.contains("--allowedTools Read,Glob,Grep,Write,Edit"));
    }

    fn flag_value<'a>(argv: &'a [String], flag: &str) -> &'a str {
        let pos = argv.iter().position(|a| a == flag).expect(flag);
        argv[pos + 1].as_str()
    }

    #[test]
    fn query_argv_excludes_write_edit() {
        let argv = build_argv(LlmMode::Query, &PathBuf::from("/v"));
        // Inspect the --tools / --allowedTools VALUES specifically — the
        // overall argv legitimately contains the substring "Edit" via
        // `acceptEdits` (the permission mode), which is unrelated to the
        // toolset whitelist.
        assert_eq!(flag_value(&argv, "--tools"), "Read,Glob,Grep");
        assert_eq!(flag_value(&argv, "--allowedTools"), "Read,Glob,Grep");
        let tools = flag_value(&argv, "--tools");
        assert!(!tools.contains("Write"));
        assert!(!tools.contains("Edit"));
    }

    #[test]
    fn forbidden_tools_never_appear_in_argv_under_any_mode() {
        // Iter-9 lesson hard-pinned: the sandbox MUST NOT leak any of these
        // tool names into any position in the argv (not as values to any
        // flag, not as positional). Anything in FORBIDDEN_TOOLS that ever
        // shows up here means the toolset whitelist regressed.
        for mode in [LlmMode::Ingest, LlmMode::Query] {
            let argv = build_argv(mode, &PathBuf::from("/v"));
            for forbidden in FORBIDDEN_TOOLS {
                for arg in &argv {
                    assert!(
                        !arg.contains(forbidden),
                        "forbidden tool {forbidden:?} found in argv for mode {mode:?}: {argv:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn argv_uses_p_flag_with_stream_json_io() {
        let argv = build_argv(LlmMode::Query, &PathBuf::from("/v"));
        assert!(argv.contains(&"-p".to_string()));
        assert!(argv.contains(&"--output-format".to_string()));
        assert!(argv.contains(&"stream-json".to_string()));
        assert!(argv.contains(&"--input-format".to_string()));
        assert!(argv.contains(&"--verbose".to_string()));
    }

    #[test]
    fn argv_pins_permission_mode_to_accept_edits() {
        let argv = build_argv(LlmMode::Ingest, &PathBuf::from("/v"));
        let joined = argv.join(" ");
        assert!(joined.contains("--permission-mode acceptEdits"));
    }

    // === classify_exit tests ===

    #[test]
    fn exit_code_zero_is_success() {
        assert_eq!(classify_exit(0, ""), ExitVerdict::Success);
        assert_eq!(classify_exit(0, "irrelevant stderr"), ExitVerdict::Success);
    }

    #[test]
    fn auth_phrases_in_stderr_trigger_oauth_needed() {
        for stderr in [
            "Error: Unauthenticated request",
            "AUTHENTICATION required",
            "authentic mismatch",
            "no token found",
            "please login first",
        ] {
            assert_eq!(classify_exit(1, stderr), ExitVerdict::OauthNeeded, "stderr: {stderr}");
        }
    }

    #[test]
    fn unrelated_failure_is_generic_error() {
        assert_eq!(classify_exit(1, "Out of memory"), ExitVerdict::GenericError);
        assert_eq!(classify_exit(2, "syntax error"), ExitVerdict::GenericError);
        assert_eq!(classify_exit(127, "command not found"), ExitVerdict::GenericError);
    }
}
