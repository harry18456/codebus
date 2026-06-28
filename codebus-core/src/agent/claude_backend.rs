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

use crate::config::{ClaudeCodeConfig, Verb};
use crate::stream::{StreamEvent, parse_claude_stream_line};
use std::process::Command;

use super::backend::AgentBackend;
use super::claude_cli::{compose_claude_cmd, sniff_init_session_id};
use super::env_overrides::EnvOverrides;
use super::spawn_spec::{Permission, SpawnSpec, verb_bundle_name};

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

/// Resolve the claude CLI binary name: the `CODEBUS_CLAUDE_BIN` override when
/// set, otherwise the bare `claude` (found via `PATH`). Shared by
/// `build_command` and the app's one-click MCP client install so detection and
/// invocation agree on which binary is used.
pub fn claude_bin() -> String {
    std::env::var("CODEBUS_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string())
}

impl AgentBackend for ClaudeBackend {
    fn build_command(&self, spec: &SpawnSpec) -> Command {
        let claude_bin = claude_bin();

        let toolset = Self::base_toolset(spec.permission);

        // `command_allowance` → Claude `--allowedTools` Bash specifier. The
        // compose helper appends bare `Bash` to `--tools` (hard gate) and the
        // `Bash(<prefix> *)` pattern to `--allowedTools` (auto-approval scope).
        let bash_whitelist = spec
            .command_allowance
            .as_ref()
            .map(|p| format!("Bash({} *)", p.joined()));

        // Per agent-backend spec `SpawnSpec Provider-Neutral Intent`:
        // resolve model/effort via `config_key()` (which returns `resolve_as`
        // when set, else `verb`) so cross-flow verify spawns pick the
        // dedicated `verify` config sub-block while still invoking the
        // goal/quiz SKILL bundle.
        let resolved = self.config.resolve(spec.config_key());

        // Assemble the Claude-form invocation from sub_mode + input + verb.
        // Per spec scenarios "Claude backend assembles slash-prefix invocation
        // from SpawnSpec fields": Some(mode) → `/codebus-<bundle> <mode>: <input>`
        // (no quote wrap); None → `/codebus-<bundle> "<input>"` (free-text
        // quote wrap, preserves the claude-side convention from the pre-Phase-3
        // verb compose sites).
        let bundle = verb_bundle_name(spec.verb);
        let prompt = match &spec.sub_mode {
            Some(mode) => format!("/codebus-{bundle} {mode}: {}", spec.input),
            None => format!("/codebus-{bundle} \"{}\"", spec.input),
        };

        // Session persistence gating: every verb except Chat is single-shot
        // and never resumes, so suppress the Claude session rollout. Chat keeps
        // persistence so the next turn can `--resume`. Mirrors the codex
        // backend's `Verb::Chat` `--ephemeral` gate.
        let no_session_persistence = !matches!(spec.verb, Verb::Chat);

        compose_claude_cmd(
            &claude_bin,
            &prompt,
            spec.resume_session_id.as_deref(),
            no_session_persistence,
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
    use crate::config::endpoint::ClaudeCodeConfig;

    fn cmd_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let pos = args.iter().position(|a| a == flag)?;
        args.get(pos + 1).map(String::as_str)
    }

    /// Whether the command has an explicitly-set env var named `key` (case
    /// -insensitive, so it works whether Windows reports `PATH` or `Path`).
    fn has_env(cmd: &Command, key: &str) -> bool {
        cmd.get_envs()
            .any(|(k, v)| v.is_some() && k.to_string_lossy().eq_ignore_ascii_case(key))
    }

    /// Value of an explicitly-set env var named `key`, if present.
    fn env_val(cmd: &Command, key: &str) -> Option<String> {
        cmd.get_envs()
            .find(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case(key))
            .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
    }

    fn backend() -> ClaudeBackend {
        ClaudeBackend::new(ClaudeCodeConfig::default(), EnvOverrides::for_system())
    }

    fn spec(verb: Verb, permission: Permission, allowance: Option<&[&str]>) -> SpawnSpec {
        SpawnSpec {
            verb,
            resolve_as: None,
            sub_mode: None,
            input: "x".to_string(),
            permission,
            command_allowance: allowance
                .map(|toks| super::super::spawn_spec::CommandPrefix::new(toks.iter().copied())),
            resume_session_id: None,
        }
    }

    /// Claude emits one `result` usage event per `-p` run, so it uses the
    /// default Delta semantics (sum) — distinct from codex's Cumulative.
    #[test]
    fn claude_declares_delta_token_usage_semantics() {
        assert_eq!(
            backend().token_usage_semantics(),
            crate::log::TokenUsageSemantics::Delta
        );
    }

    /// Single-shot verbs (goal/query/fix/quiz) never resume, so their claude
    /// spawn carries `--no-session-persistence` (mirrors codex `--ephemeral`).
    #[test]
    fn single_shot_verbs_include_no_session_persistence() {
        for verb in [Verb::Goal, Verb::Query, Verb::Fix, Verb::Quiz] {
            let cmd = backend().build_command(&spec(verb, Permission::ReadOnly, None));
            let args = cmd_args(&cmd);
            assert!(
                args.iter().any(|a| a == "--no-session-persistence"),
                "{verb:?} argv must include --no-session-persistence: {args:?}"
            );
        }
    }

    /// Chat is multi-turn and resumes via `--resume`, so its spawn MUST retain
    /// session persistence — no `--no-session-persistence` flag.
    #[test]
    fn chat_verb_omits_no_session_persistence() {
        let cmd = backend().build_command(&spec(Verb::Chat, Permission::ReadOnly, None));
        let args = cmd_args(&cmd);
        assert!(
            !args.iter().any(|a| a == "--no-session-persistence"),
            "chat argv must NOT include --no-session-persistence: {args:?}"
        );
    }

    /// Chat with a resume id keeps `--resume <id>` and still omits the flag.
    #[test]
    fn chat_resume_keeps_resume_flag_and_omits_no_session_persistence() {
        let chat_spec = SpawnSpec {
            verb: Verb::Chat,
            resolve_as: None,
            sub_mode: None,
            input: "x".to_string(),
            permission: Permission::ReadOnly,
            command_allowance: None,
            resume_session_id: Some("abc-123".to_string()),
        };
        let cmd = backend().build_command(&chat_spec);
        let args = cmd_args(&cmd);
        assert_eq!(arg_after(&args, "--resume"), Some("abc-123"));
        assert!(
            !args.iter().any(|a| a == "--no-session-persistence"),
            "chat resume argv must NOT include --no-session-persistence: {args:?}"
        );
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
    }

    /// Goal spawn produces the full expected argv (Workspace toolset, model
    /// resolved from config, MCP isolation, stream-json flags) — the direct
    /// byte-level assertion that replaces the pre-refactor `build_claude_cmd`
    /// reference. Goal default model = opus-4-6 / high.
    #[test]
    fn goal_spawn_full_argv() {
        let cmd = backend().build_command(&spec(Verb::Goal, Permission::Workspace, None));
        let args = cmd_args(&cmd);
        assert_eq!(arg_after(&args, "-p"), Some("/codebus-goal \"x\""));
        assert_eq!(arg_after(&args, "--tools"), Some("Read,Glob,Grep,Write,Edit"));
        assert_eq!(arg_after(&args, "--permission-mode"), Some("acceptEdits"));
        assert_eq!(arg_after(&args, "--output-format"), Some("stream-json"));
        assert!(args.iter().any(|a| a == "--verbose"));
        assert_eq!(arg_after(&args, "--setting-sources"), Some("project,local"));
        assert_eq!(arg_after(&args, "--model"), Some("claude-opus-4-6"));
        assert_eq!(arg_after(&args, "--effort"), Some("high"));
    }

    /// Spawn env scrub (claude side of `Scoped Environment Injection At
    /// Spawn`): `build_command` `env_clear`s and re-injects the
    /// system-essential allowlist, and the azure provider key is injected
    /// AFTER the clear so it survives. Asserts the child env carries the
    /// passthrough member `PATH` and the azure `ANTHROPIC_API_KEY`.
    #[test]
    fn build_command_scrubs_env_keeps_path_and_provider_key() {
        let azure_backend = ClaudeBackend::new(
            ClaudeCodeConfig::default(),
            EnvOverrides::for_azure("https://x.example.com/anthropic", "sk-wire-test"),
        );
        let cmd = azure_backend.build_command(&spec(Verb::Query, Permission::ReadOnly, None));
        assert!(has_env(&cmd, "PATH"), "PATH must pass through the scrub");
        assert_eq!(
            env_val(&cmd, "ANTHROPIC_API_KEY").as_deref(),
            Some("sk-wire-test"),
            "azure provider key must survive env_clear (injected after it)"
        );
    }

    // Phase 3 / prompt-surface-layer-3-spawnspec-restructure: assembly tests.
    // Spec scenario "Claude backend assembles slash-prefix invocation from
    // SpawnSpec fields".

    #[test]
    fn claude_assembly_free_text_chat_quoted() {
        let mut s = spec(Verb::Chat, Permission::ReadOnly, None);
        s.input = "explain the auth flow".to_string();
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        // Spec Example: claude assembly for chat verb free-text.
        assert_eq!(arg_after(&args, "-p"), Some("/codebus-chat \"explain the auth flow\""));
    }

    #[test]
    fn claude_assembly_sub_mode_quiz_plan_no_quote() {
        let mut s = spec(Verb::Quiz, Permission::ReadOnly, None);
        s.sub_mode = Some("plan".to_string());
        s.input = "auth middleware".to_string();
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        // Spec Example: claude assembly for quiz verb plan sub-mode.
        assert_eq!(arg_after(&args, "-p"), Some("/codebus-quiz plan: auth middleware"));
    }

    #[test]
    fn claude_assembly_verb_name_is_lowercase_bundle() {
        // Verb::Goal → /codebus-goal, matching the .claude/skills/codebus-goal/
        // bundle directory; NOT /codebus-Goal (Debug-format).
        let mut s = spec(Verb::Goal, Permission::Workspace, None);
        s.input = "draft payments overview".to_string();
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        assert_eq!(arg_after(&args, "-p"), Some("/codebus-goal \"draft payments overview\""));
    }

    #[test]
    fn claude_assembly_sub_mode_input_with_newlines_preserved() {
        // sub-mode spawns carry structured multiline input (e.g. goal verify
        // body has CHANGED PAGES:\n... section). Assembly preserves \n.
        let mut s = spec(Verb::Goal, Permission::ReadOnly, None);
        s.sub_mode = Some("verify".to_string());
        s.input = "goal=X\n\nCHANGED PAGES:\nwiki/a.md\nwiki/b.md".to_string();
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        assert_eq!(
            arg_after(&args, "-p"),
            Some("/codebus-goal verify: goal=X\n\nCHANGED PAGES:\nwiki/a.md\nwiki/b.md")
        );
    }

    #[test]
    fn claude_resolve_as_overrides_model_lookup() {
        // verify-stage-independent-model: verb=Goal but resolve_as=Some(Verify)
        // → bundle name "goal" (SKILL bundle invocation) AND model resolved
        // via Verb::Verify config sub-block.
        let mut s = spec(Verb::Goal, Permission::ReadOnly, None);
        s.resolve_as = Some(Verb::Verify);
        s.sub_mode = Some("verify".to_string());
        s.input = "test".to_string();
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        // bundle name still "goal" — the SKILL bundle being invoked
        assert!(
            arg_after(&args, "-p").unwrap().starts_with("/codebus-goal verify:"),
            "bundle name is goal (the SKILL bundle), not verify"
        );
        // model resolved via Verb::Verify config sub-block. We assert it
        // matches `ClaudeCodeConfig::default().resolve(Verb::Verify)` so the
        // test is robust against config defaults shifting AND specifically
        // verifies the resolve_as override path went through Verify.
        let expected = ClaudeCodeConfig::default().resolve(Verb::Verify);
        assert_eq!(arg_after(&args, "--model"), expected.model.as_deref());
        assert_eq!(arg_after(&args, "--effort"), expected.effort.as_deref());
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
