//! [`ClaudeBackend`] — the Claude CLI implementation of [`AgentBackend`].
//!
//! Owns everything Claude-specific: the `claude` binary path
//! (`CODEBUS_CLAUDE_BIN` override), the `SpawnSpec` → `claude -p` argv
//! mapping (permission → `--tools` set, `command_allowance` → fine-grained
//! `--allowedTools` Bash specifier, MCP isolation flags), model/effort
//! resolution from its own config, and scoped env injection. The
//! provider-agnostic [`invoke`](super::claude_cli::invoke) loop drives it
//! through the three trait methods only.
//!
//! Byte-equivalence: `build_command` delegates to
//! [`compose_claude_cmd`](super::claude_cli::compose_claude_cmd), the same
//! argv composer the legacy `InvokeAgentOptions` path uses — so the argv is
//! identical to the pre-refactor spawn for every spawn. (The single
//! deliberate exception is quiz-generate: the pre-refactor toolset constant
//! redundantly listed bare `Bash` alongside the `command_allowance`,
//! producing a duplicate `Bash` in `--tools` and a too-broad bare `Bash` in
//! `--allowedTools`; the clean permission model drops the redundancy,
//! matching the `fix` verb's correct pattern. Functionally equivalent — the
//! quiz PreToolUse hook gates bash to `codebus` regardless.)

use crate::config::ClaudeCodeConfig;
use crate::stream::{StreamEvent, parse_claude_stream_line};
use std::process::Command;

use super::backend::AgentBackend;
use super::claude_cli::{compose_claude_cmd, sniff_init_session_id};
use super::env_overrides::EnvOverrides;
use super::spawn_spec::{Permission, SpawnSpec};

/// Read-only tool set (no write/edit). Matches the pre-refactor
/// `CHAT_TOOLSET` / `QUERY_TOOLSET` / `QUIZ_TOOLSET` / `GOAL_VERIFY_TOOLSET`.
const READ_ONLY_TOOLS: &[&str] = &["Read", "Glob", "Grep"];

/// Workspace tool set (adds Write/Edit). Matches the pre-refactor
/// `GOAL_TOOLSET` / `FIX_TOOLSET`.
const WORKSPACE_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

/// The Claude CLI backend. Holds the resolved endpoint config (for per-verb
/// model/effort resolution) and the scoped env overrides built by the verb
/// layer (which owns the keyring fallback + `KeyringMissing` error path).
pub struct ClaudeBackend {
    config: ClaudeCodeConfig,
    env: EnvOverrides,
}

impl ClaudeBackend {
    /// Construct from the loaded config and pre-built env overrides. The verb
    /// layer builds `env` via `build_env_overrides` (handling the azure
    /// keyring fallback / `KeyringMissing` error) before constructing the
    /// backend, so backend construction itself is infallible.
    pub fn new(config: ClaudeCodeConfig, env: EnvOverrides) -> Self {
        Self { config, env }
    }

    /// The base tool set for a permission posture.
    fn base_toolset(permission: Permission) -> &'static [&'static str] {
        match permission {
            Permission::ReadOnly => READ_ONLY_TOOLS,
            Permission::Workspace => WORKSPACE_TOOLS,
        }
    }
}

impl AgentBackend for ClaudeBackend {
    fn build_command(&self, spec: &SpawnSpec) -> Command {
        let claude_bin =
            std::env::var("CODEBUS_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());

        let toolset = Self::base_toolset(spec.permission);

        // `command_allowance` → Claude `--allowedTools` Bash specifier. The
        // compose helper appends bare `Bash` to `--tools` (hard gate) and the
        // `Bash(<prefix> *)` pattern to `--allowedTools` (auto-approval scope).
        let bash_whitelist = spec
            .command_allowance
            .as_ref()
            .map(|p| format!("Bash({} *)", p.joined()));

        let resolved = self.config.resolve(spec.verb);

        compose_claude_cmd(
            &claude_bin,
            &spec.prompt,
            spec.resume_session_id.as_deref(),
            toolset,
            bash_whitelist.as_deref(),
            resolved.model.as_deref(),
            resolved.effort.as_deref(),
            &self.env,
        )
    }

    fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent> {
        parse_claude_stream_line(line)
    }

    fn extract_session_id(&self, line: &str) -> Option<String> {
        sniff_init_session_id(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Verb;
    use crate::config::endpoint::{ClaudeCodeConfig, SystemModel};

    fn cmd_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let pos = args.iter().position(|a| a == flag)?;
        args.get(pos + 1).map(String::as_str)
    }

    fn backend() -> ClaudeBackend {
        ClaudeBackend::new(ClaudeCodeConfig::default(), EnvOverrides::for_system())
    }

    fn spec(verb: Verb, permission: Permission, allowance: Option<&[&str]>) -> SpawnSpec {
        SpawnSpec {
            verb,
            prompt: format!("/codebus-{verb:?} \"x\""),
            permission,
            command_allowance: allowance
                .map(|toks| super::super::spawn_spec::CommandPrefix::new(toks.iter().copied())),
            resume_session_id: None,
        }
    }

    /// `agent-backend` spec `Claude Backend Argv Equivalence`:
    /// "Read-only permission excludes write tools".
    #[test]
    fn read_only_permission_excludes_write_tools() {
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly, None));
        let args = cmd_args(&cmd);
        let tools = arg_after(&args, "--tools").expect("--tools present");
        assert_eq!(tools, "Read,Glob,Grep");
        assert!(!tools.contains("Write"));
        assert!(!tools.contains("Edit"));
        assert!(!tools.contains("Bash"));
    }

    /// Workspace permission includes write tools (matches GOAL_TOOLSET / FIX_TOOLSET).
    #[test]
    fn workspace_permission_includes_write_tools() {
        let cmd = backend().build_command(&spec(Verb::Goal, Permission::Workspace, None));
        let args = cmd_args(&cmd);
        let tools = arg_after(&args, "--tools").expect("--tools present");
        assert_eq!(tools, "Read,Glob,Grep,Write,Edit");
    }

    /// `agent-backend` spec `Claude Backend Argv Equivalence`:
    /// "command_allowance maps to fine-grained Bash specifier".
    #[test]
    fn command_allowance_maps_to_bash_specifier() {
        let cmd = backend().build_command(&spec(
            Verb::Quiz,
            Permission::ReadOnly,
            Some(&["codebus", "quiz", "validate"]),
        ));
        let args = cmd_args(&cmd);
        let tools = arg_after(&args, "--tools").expect("--tools present");
        let allowed = arg_after(&args, "--allowedTools").expect("--allowedTools present");
        // Single bare Bash in --tools (hard gate).
        assert_eq!(tools, "Read,Glob,Grep,Bash");
        // --allowedTools carries ONLY the restricted specifier — no broad bare
        // Bash (the clean form; pre-refactor quiz-generate had a redundant
        // bare Bash here, defeating the restriction).
        assert_eq!(allowed, "Read,Glob,Grep,Bash(codebus quiz validate *)");
    }

    /// Resume id placed before the toolset flags.
    #[test]
    fn resume_id_before_toolset() {
        let mut s = spec(Verb::Chat, Permission::ReadOnly, None);
        s.resume_session_id = Some("abc-123".into());
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        let resume = args.iter().position(|a| a == "--resume").expect("--resume");
        let tools = args.iter().position(|a| a == "--tools").expect("--tools");
        assert_eq!(args.get(resume + 1).map(String::as_str), Some("abc-123"));
        assert!(resume < tools);
    }

    /// MCP isolation flags are always present.
    #[test]
    fn mcp_isolation_flags_present() {
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly, None));
        let args = cmd_args(&cmd);
        assert!(args.iter().any(|a| a == "--strict-mcp-config"));
        assert_eq!(
            arg_after(&args, "--mcp-config"),
            Some(r#"{"mcpServers":{}}"#)
        );
    }

    /// Model/effort resolved from the backend's own config via resolve(verb).
    /// Default config: query → haiku-4-5 / low.
    #[test]
    fn model_effort_resolved_from_config_by_verb() {
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly, None));
        let args = cmd_args(&cmd);
        assert_eq!(arg_after(&args, "--model"), Some("claude-haiku-4-5"));
        assert_eq!(arg_after(&args, "--effort"), Some("low"));
        // Sanity: default query model is haiku.
        let _ = SystemModel::Haiku4_5;
    }

    /// Goal spawn produces the full expected argv (Workspace toolset, model
    /// resolved from config, MCP isolation, stream-json flags) — the direct
    /// byte-level assertion that replaces the pre-refactor `build_claude_cmd`
    /// reference. Goal default model = opus-4-6 / high.
    #[test]
    fn goal_spawn_full_argv() {
        let cmd = backend().build_command(&spec(Verb::Goal, Permission::Workspace, None));
        let args = cmd_args(&cmd);
        assert_eq!(arg_after(&args, "-p"), Some("/codebus-Goal \"x\""));
        assert_eq!(arg_after(&args, "--tools"), Some("Read,Glob,Grep,Write,Edit"));
        assert_eq!(arg_after(&args, "--permission-mode"), Some("acceptEdits"));
        assert_eq!(arg_after(&args, "--output-format"), Some("stream-json"));
        assert!(args.iter().any(|a| a == "--verbose"));
        assert_eq!(arg_after(&args, "--model"), Some("claude-opus-4-6"));
        assert_eq!(arg_after(&args, "--effort"), Some("high"));
    }

    /// parse_stream_line / extract_session_id wrap the legacy free functions.
    #[test]
    fn parse_and_session_id_match_legacy() {
        let b = backend();
        let line = r#"{"type":"system","subtype":"init","session_id":"s-1"}"#;
        assert_eq!(b.extract_session_id(line), Some("s-1".to_string()));
        let assistant = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        assert_eq!(
            b.parse_stream_line(assistant),
            parse_claude_stream_line(assistant)
        );
    }
}
