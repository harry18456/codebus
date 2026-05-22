//! [`CodexBackend`] — the OpenAI Codex CLI implementation of [`AgentBackend`].
//!
//! Owns everything codex-specific: the `codex` binary path
//! (`CODEBUS_CODEX_BIN` override), the `SpawnSpec` → `codex exec` argv mapping
//! (the spike-verified per-spawn isolation recipe, sandbox mapping, model /
//! effort flags, Azure Responses-API provider override), codex JSONL → neutral
//! [`StreamEvent`] parsing, and `thread.started` session-id extraction. The
//! provider-agnostic [`invoke`](super::claude_cli::invoke) loop drives it
//! through the three trait methods only.
//!
//! Isolation recipe (verified 2026-05-22, docs/2026-05-14-...backlog.md §4(F)):
//! every spawn carries `--ignore-user-config` (drop user global config/MCP),
//! `--disable apps` (drop plugin/codex_apps tools), `--ignore-rules` (drop
//! execpolicy), and `-c project_root_markers=['<marker>']` (pin the codex
//! project root to the vault, excluding the analyzed repo's `.codex/` and
//! `AGENTS.md`). All per-spawn — never mutates the user's codex config.

use std::process::{Command, Stdio};

use crate::config::{CodexConfig, Verb};
use crate::config::endpoint::ActiveProfile;
use crate::stream::{StreamEvent, parse_codex_stream_line};

use super::backend::AgentBackend;
use super::spawn_spec::{Permission, SpawnSpec};

/// Env var carrying the Azure OpenAI API key into the codex child process.
/// The Azure provider override references it via `env_key` / `env_http_headers`
/// (Azure auth uses an `api-key` header, not `Authorization: Bearer`).
pub const CODEX_AZURE_KEY_ENV: &str = "CODEBUS_CODEX_AZURE_KEY";

/// Vault-unique marker file name. `build_command` names it in the codex
/// `project_root_markers` override so codex pins its project root to the
/// `.codebus/` vault (materialized by the skill-bundle layer). Excludes any
/// `.codex/` or `AGENTS.md` in the analyzed repository above the vault.
pub const CODEX_VAULT_MARKER: &str = ".codebus-vault";

/// The OpenAI Codex CLI backend. Holds the resolved codex endpoint config
/// (for per-verb model/effort + Azure routing) and, when the azure profile is
/// active, the API key read by the verb layer (keyring → env fallback) before
/// construction — so backend construction itself is infallible.
pub struct CodexBackend {
    config: CodexConfig,
    azure_key: Option<String>,
}

impl CodexBackend {
    /// Construct from the loaded codex config and the pre-read Azure key
    /// (`Some` when `config.active == Azure`, `None` otherwise).
    pub fn new(config: CodexConfig, azure_key: Option<String>) -> Self {
        Self { config, azure_key }
    }

    fn sandbox_flag(permission: Permission) -> &'static str {
        match permission {
            Permission::ReadOnly => "read-only",
            Permission::Workspace => "workspace-write",
        }
    }
}

/// Default codex binary name when `CODEBUS_CODEX_BIN` is unset. On Windows the
/// npm-installed codex is a `codex.cmd` batch shim; `Command::new("codex")`
/// does NOT resolve it via `PATHEXT` (it errors `program not found`), so we
/// default to `codex.cmd` which Rust resolves through `PATH` and executes via
/// the cmd shell. On Unix the bare `codex` name is correct.
fn default_codex_bin() -> &'static str {
    if cfg!(windows) {
        "codex.cmd"
    } else {
        "codex"
    }
}

impl AgentBackend for CodexBackend {
    fn build_command(&self, spec: &SpawnSpec) -> Command {
        let codex_bin =
            std::env::var("CODEBUS_CODEX_BIN").unwrap_or_else(|_| default_codex_bin().to_string());
        let mut cmd = Command::new(codex_bin);
        cmd.arg("exec");

        // Resume the prior thread when continuing a multi-turn conversation.
        if let Some(id) = spec.resume_session_id.as_deref() {
            cmd.arg("resume").arg(id);
        }

        // Per-spawn isolation recipe (verified §4(F)): drop user config/MCP,
        // plugins, execpolicy; pin the project root to the vault so the
        // analyzed repo's `.codex/` and `AGENTS.md` are excluded.
        cmd.arg("--json")
            .arg("--ignore-user-config")
            .arg("--disable")
            .arg("apps")
            .arg("--ignore-rules")
            .arg("--skip-git-repo-check")
            .arg("-c")
            .arg(format!("project_root_markers=['{CODEX_VAULT_MARKER}']"));

        // Session persistence: `chat` is a multi-turn conversation — each turn
        // MUST persist its session rollout so the next turn can `exec resume
        // <id>`. `--ephemeral` (no session files) makes resume fail with
        // "no rollout found for thread id" (verified against real codex). The
        // single-shot verbs (goal / query / fix / quiz) never resume, so they
        // keep `--ephemeral` to avoid leaving session files behind.
        if !matches!(spec.verb, Verb::Chat) {
            cmd.arg("--ephemeral");
        }

        // Sandbox: `codex exec` takes `-s <mode>`, but `codex exec resume` does
        // NOT accept `-s` (verified against `codex exec resume --help`) — it
        // rejects the flag and aborts. On the resume path pass the equivalent
        // `-c sandbox_mode=<mode>` config override instead (resume accepts
        // `-c`; the value is unquoted so codex's TOML literal fallback keeps
        // `read-only` / `workspace-write` intact, like the azure `-c` args).
        let sandbox = Self::sandbox_flag(spec.permission);
        if spec.resume_session_id.is_some() {
            cmd.arg("-c").arg(format!("sandbox_mode={sandbox}"));
        } else {
            cmd.arg("-s").arg(sandbox);
        }

        // Model + effort come from CLI flags, NOT the (trust-gated) vault
        // `.codex/config.toml`. Effort is passed unquoted — codex falls back
        // to a literal string when the value is not valid TOML.
        let resolved = self.config.resolve(spec.verb);
        if let Some(model) = resolved.model.as_deref() {
            cmd.arg("-m").arg(model);
        }
        if let Some(effort) = resolved.effort.as_deref() {
            cmd.arg("-c").arg(format!("model_reasoning_effort={effort}"));
        }

        // Azure OpenAI: route through a custom provider hitting the Responses
        // API with an `api-key` header (not Bearer), api-version query param,
        // and the deployment name carried by `-m` above. The key is injected
        // into the child env and referenced by `env_key` / `env_http_headers`.
        if self.config.active == ActiveProfile::Azure {
            if let Some(az) = self.config.azure.as_ref() {
                // No embedded double-quotes anywhere: codex parses each `-c`
                // value as TOML and falls back to a literal string when it is
                // not valid TOML, and TOML bare keys permit hyphens. Quoting
                // the `api-version` / `api-key` segments would leak literal
                // quotes into the request (observed: `?"api-version"=...` → 404)
                // and is fragile across the Windows `.cmd` shim's re-quoting.
                // Trim surrounding whitespace: a stray leading tab/space in the
                // GUI-entered base_url / api_version (observed in a real config:
                // `"\t https://…"`) would otherwise leak into the request URL and
                // make every azure call fail. Codex parses each `-c` value as
                // TOML literal, so the trimmed bare string is used verbatim.
                let base_url = az.base_url.trim();
                let api_version = az.api_version.trim();
                cmd.arg("-c")
                    .arg("model_provider=azure")
                    .arg("-c")
                    .arg("model_providers.azure.name=azure")
                    .arg("-c")
                    .arg(format!("model_providers.azure.base_url={base_url}"))
                    .arg("-c")
                    .arg("model_providers.azure.wire_api=responses")
                    .arg("-c")
                    .arg(format!("model_providers.azure.env_key={CODEX_AZURE_KEY_ENV}"))
                    .arg("-c")
                    .arg(format!(
                        "model_providers.azure.query_params.api-version={api_version}"
                    ))
                    .arg("-c")
                    .arg(format!(
                        "model_providers.azure.env_http_headers.api-key={CODEX_AZURE_KEY_ENV}"
                    ));
                if let Some(key) = self.azure_key.as_deref() {
                    cmd.env(CODEX_AZURE_KEY_ENV, key);
                }
            }
        }

        // `command_allowance` has no codex equivalent (sandbox `-s` governs
        // command execution). Degrade with a single warning — no hard gate.
        if spec.command_allowance.is_some() {
            eprintln!(
                "warning: codex backend has no per-command allowance; ignoring command_allowance (sandbox -s governs command execution)"
            );
        }

        cmd.arg(&spec.prompt);
        // Close stdin: codex exec blocks waiting on stdin when it is an open
        // non-TTY pipe (verified — a background spawn hung 60s+ with no input).
        cmd.stdin(Stdio::null());
        cmd
    }

    fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent> {
        parse_codex_stream_line(line)
    }

    fn extract_session_id(&self, line: &str) -> Option<String> {
        // delegated to the stream parser's session sniffer
        crate::stream::sniff_codex_thread_id(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Verb;
    use crate::config::codex::{CodexAzureProfile, CodexSystemProfile, CodexVerbConfig};

    fn verb_cfg(model: &str, effort: &str) -> CodexVerbConfig {
        CodexVerbConfig {
            model: model.to_string(),
            effort: effort.to_string(),
        }
    }

    fn system_config() -> CodexConfig {
        CodexConfig {
            active: ActiveProfile::System,
            system: Some(CodexSystemProfile {
                goal: verb_cfg("gpt-5.5", "high"),
                query: verb_cfg("gpt-5.5", "low"),
                fix: verb_cfg("gpt-5.5", "medium"),
                verify: verb_cfg("gpt-5.5", "high"),
            }),
            azure: None,
        }
    }

    fn backend() -> CodexBackend {
        CodexBackend::new(system_config(), None)
    }

    fn spec(verb: Verb, permission: Permission) -> SpawnSpec {
        SpawnSpec {
            verb,
            prompt: format!("/codebus-{verb:?} x"),
            permission,
            command_allowance: None,
            resume_session_id: None,
        }
    }

    fn cmd_args(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    /// On Windows the default codex binary is `codex.cmd` (the npm shim that
    /// `Command::new("codex")` cannot resolve via PATHEXT); elsewhere `codex`.
    /// Regression for the GUI "select codex → chat no response" bug.
    #[test]
    fn default_codex_bin_resolves_windows_cmd_shim() {
        if cfg!(windows) {
            assert_eq!(default_codex_bin(), "codex.cmd");
        } else {
            assert_eq!(default_codex_bin(), "codex");
        }
    }

    /// With `CODEBUS_CODEX_BIN` unset, `build_command`'s program is the
    /// platform default (not the bare `codex` that fails to spawn on Windows).
    #[test]
    fn build_command_uses_platform_default_bin_when_env_unset() {
        // Only meaningful when the override is not present in this process.
        if std::env::var("CODEBUS_CODEX_BIN").is_ok() {
            return;
        }
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly));
        assert_eq!(
            cmd.get_program().to_string_lossy(),
            default_codex_bin(),
        );
    }

    /// Spec: Codex Backend Argv Composition — read-only permission maps to
    /// the read-only sandbox.
    #[test]
    fn read_only_maps_to_read_only_sandbox() {
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly));
        let args = cmd_args(&cmd);
        let pos = args.iter().position(|a| a == "-s").expect("-s present");
        assert_eq!(args.get(pos + 1).map(String::as_str), Some("read-only"));
        assert!(!args.iter().any(|a| a == "workspace-write"));
    }

    /// Workspace permission maps to workspace-write.
    #[test]
    fn workspace_maps_to_workspace_write() {
        let cmd = backend().build_command(&spec(Verb::Goal, Permission::Workspace));
        let args = cmd_args(&cmd);
        let pos = args.iter().position(|a| a == "-s").expect("-s present");
        assert_eq!(args.get(pos + 1).map(String::as_str), Some("workspace-write"));
    }

    /// Isolation flags are always present.
    #[test]
    fn isolation_flags_always_present() {
        let cmd = backend().build_command(&spec(Verb::Query, Permission::ReadOnly));
        let args = cmd_args(&cmd);
        assert!(args.iter().any(|a| a == "--ignore-user-config"));
        assert!(args.iter().any(|a| a == "--disable"));
        assert!(args.iter().any(|a| a == "apps"));
        assert!(args.iter().any(|a| a == "--ignore-rules"));
        assert!(
            args.iter().any(|a| a.contains("project_root_markers")),
            "project_root_markers override present; got {args:?}"
        );
    }

    /// Model and effort are passed as CLI flags (config.toml is trust-gated).
    #[test]
    fn model_and_effort_passed_as_flags() {
        // default system config: goal → gpt-5.5 / high
        let cmd = backend().build_command(&spec(Verb::Goal, Permission::Workspace));
        let args = cmd_args(&cmd);
        let m = args.iter().position(|a| a == "-m").expect("-m present");
        assert_eq!(args.get(m + 1).map(String::as_str), Some("gpt-5.5"));
        assert!(
            args.iter().any(|a| a.contains("model_reasoning_effort=high")),
            "effort override present; got {args:?}"
        );
    }

    /// Resume id uses the `exec resume <id>` subcommand form.
    #[test]
    fn resume_id_uses_resume_subcommand() {
        let mut s = spec(Verb::Chat, Permission::ReadOnly);
        s.resume_session_id = Some("019e-abc".to_string());
        let cmd = backend().build_command(&s);
        let args = cmd_args(&cmd);
        let exec = args.iter().position(|a| a == "exec").expect("exec present");
        let resume = args.iter().position(|a| a == "resume").expect("resume present");
        assert!(exec < resume, "exec before resume; got {args:?}");
        assert!(args.iter().any(|a| a == "019e-abc"));
    }

    /// `codex exec resume` rejects `-s`, so the resume path SHALL pass the
    /// sandbox as `-c sandbox_mode=<mode>` and SHALL NOT emit `-s`. Regression
    /// for the GUI/CLI multi-turn chat "unexpected argument '-s'" abort.
    #[test]
    fn resume_passes_sandbox_via_config_not_dash_s() {
        let mut s = spec(Verb::Chat, Permission::ReadOnly);
        s.resume_session_id = Some("019e-abc".to_string());
        let args = cmd_args(&backend().build_command(&s));
        assert!(!args.iter().any(|a| a == "-s"), "resume must NOT use -s; got {args:?}");
        assert!(
            args.iter().any(|a| a == "sandbox_mode=read-only"),
            "resume must set sandbox via -c sandbox_mode=; got {args:?}"
        );
    }

    /// `chat` is multi-turn → it MUST NOT be `--ephemeral` (resume needs the
    /// persisted rollout; otherwise codex aborts "no rollout found"). The
    /// single-shot verbs keep `--ephemeral`. Regression for the resume bug.
    #[test]
    fn chat_omits_ephemeral_single_shot_keeps_it() {
        let chat = cmd_args(&backend().build_command(&spec(Verb::Chat, Permission::ReadOnly)));
        assert!(
            !chat.iter().any(|a| a == "--ephemeral"),
            "chat MUST NOT be ephemeral (resume needs the rollout); got {chat:?}"
        );
        let query = cmd_args(&backend().build_command(&spec(Verb::Query, Permission::ReadOnly)));
        assert!(
            query.iter().any(|a| a == "--ephemeral"),
            "single-shot query keeps --ephemeral; got {query:?}"
        );
    }

    /// The non-resume (fresh) path keeps the dedicated `-s <mode>` flag.
    #[test]
    fn fresh_spawn_uses_dash_s_sandbox() {
        let args = cmd_args(&backend().build_command(&spec(Verb::Chat, Permission::Workspace)));
        let pos = args.iter().position(|a| a == "-s").expect("-s present on fresh spawn");
        assert_eq!(args.get(pos + 1).map(String::as_str), Some("workspace-write"));
        assert!(!args.iter().any(|a| a.starts_with("sandbox_mode=")));
    }

    /// command_allowance has no codex equivalent → spawn proceeds (no panic,
    /// no hard gate). Behaviour: build succeeds and produces a valid command.
    #[test]
    fn command_allowance_does_not_block() {
        let mut s = spec(Verb::Quiz, Permission::ReadOnly);
        s.command_allowance =
            Some(super::super::spawn_spec::CommandPrefix::new(["codebus", "quiz", "validate"]));
        let cmd = backend().build_command(&s);
        // Builds without panicking and still includes the sandbox flag.
        assert!(cmd_args(&cmd).iter().any(|a| a == "-s"));
    }

    fn azure_backend() -> CodexBackend {
        let cfg = CodexConfig {
            active: ActiveProfile::Azure,
            system: None,
            azure: Some(CodexAzureProfile {
                base_url: "https://x.cognitiveservices.azure.com/openai".to_string(),
                api_version: "2025-04-01-preview".to_string(),
                keyring_service: "codebus-azure".to_string(),
                goal: verb_cfg("gpt-5.4", "high"),
                query: verb_cfg("gpt-5.4", "low"),
                fix: verb_cfg("gpt-5.4", "medium"),
                verify: verb_cfg("gpt-5.4", "high"),
            }),
        };
        CodexBackend::new(cfg, Some("sk-azure-test".to_string()))
    }

    /// Azure provider override is composed with the Responses API + api-key
    /// header. Regression guard for the e2e-found bug: `-c` values MUST NOT
    /// carry embedded double-quotes (a quoted `query_params."api-version"`
    /// leaked literal quotes into the request URL → 404). Bare keys + unquoted
    /// values survive the Windows `.cmd` shim's re-quoting.
    #[test]
    fn azure_provider_override_uses_unquoted_bare_keys() {
        let cmd = azure_backend().build_command(&spec(Verb::Query, Permission::ReadOnly));
        let args = cmd_args(&cmd);
        assert!(args.iter().any(|a| a == "model_provider=azure"));
        assert!(args.iter().any(|a| a == "model_providers.azure.wire_api=responses"));
        assert!(
            args.iter()
                .any(|a| a == "model_providers.azure.query_params.api-version=2025-04-01-preview"),
            "api-version must be a bare unquoted key/value; got {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| *a == format!("model_providers.azure.env_http_headers.api-key={CODEX_AZURE_KEY_ENV}")),
            "api-key header maps to the env key, unquoted; got {args:?}"
        );
        // No `-c` value may contain a literal double-quote.
        assert!(
            !args.iter().any(|a| a.contains('"')),
            "azure -c args must carry no embedded double-quotes; got {args:?}"
        );
    }

    /// A stray leading tab/space in the GUI-entered base_url / api_version
    /// (observed verbatim in a real config: `"\t https://…"`) is trimmed before
    /// composing the `-c` overrides, so it never corrupts the request URL.
    /// Regression for the "switch to codex azure → no response" bug.
    #[test]
    fn azure_base_url_and_api_version_are_trimmed() {
        let cfg = CodexConfig {
            active: ActiveProfile::Azure,
            system: None,
            azure: Some(CodexAzureProfile {
                base_url: "\t https://x.cognitiveservices.azure.com/openai ".to_string(),
                api_version: "  2025-04-01-preview\t".to_string(),
                keyring_service: "codebus-codex-azure".to_string(),
                goal: verb_cfg("gpt-5.4", "high"),
                query: verb_cfg("gpt-5.4", "low"),
                fix: verb_cfg("gpt-5.4", "medium"),
                verify: verb_cfg("gpt-5.4", "high"),
            }),
        };
        let args = cmd_args(
            &CodexBackend::new(cfg, Some("sk".into()))
                .build_command(&spec(Verb::Query, Permission::ReadOnly)),
        );
        assert!(
            args.iter()
                .any(|a| a == "model_providers.azure.base_url=https://x.cognitiveservices.azure.com/openai"),
            "base_url must be trimmed; got {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| a == "model_providers.azure.query_params.api-version=2025-04-01-preview"),
            "api_version must be trimmed; got {args:?}"
        );
        assert!(
            !args.iter().any(|a| a.contains('\t')),
            "no arg may carry a tab; got {args:?}"
        );
    }

    /// Azure key is injected into the child env (referenced by env_key /
    /// env_http_headers), never inlined into argv.
    #[test]
    fn azure_key_injected_via_env_not_argv() {
        let cmd = azure_backend().build_command(&spec(Verb::Query, Permission::ReadOnly));
        let env_has_key = cmd
            .get_envs()
            .any(|(k, v)| k == CODEX_AZURE_KEY_ENV && v == Some("sk-azure-test".as_ref()));
        assert!(env_has_key, "azure key must be set in the child env");
        assert!(
            !cmd_args(&cmd).iter().any(|a| a.contains("sk-azure-test")),
            "azure key must NOT appear in argv"
        );
    }
}
