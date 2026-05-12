//! Scoped environment overrides for the agent sub-process.
//!
//! `EnvOverrides` carries a deterministic `(name, value)` map of env vars
//! injected into the child process via `Command::envs(...)`. The parent
//! shell environment SHALL NOT be modified — codebus never calls
//! `std::env::set_var` in the spawn path. See spec
//! `claude-code-config / Scoped Environment Injection At Spawn`.
//!
//! Two builders cover the active-profile cases:
//!
//! - [`EnvOverrides::for_system`] — empty map. The child inherits the
//!   parent env unchanged (no Azure routing, no advisor-tool override).
//! - [`EnvOverrides::for_azure`] — exactly three keys: `ANTHROPIC_BASE_URL`
//!   pointing at the Azure-compatible endpoint, `ANTHROPIC_API_KEY` from
//!   the keyring / env fallback chain, and `CLAUDE_CODE_DISABLE_ADVISOR_TOOL`
//!   set to the literal string `"1"` (v2 strategy memo §8 verified this
//!   undocumented env is required — Azure rejects the `anthropic-beta:
//!   advisor-tool-2026-03-01` header with HTTP 400).

use std::collections::BTreeMap;

/// `ANTHROPIC_BASE_URL` — points Claude CLI at the configured endpoint.
pub const ENV_ANTHROPIC_BASE_URL: &str = "ANTHROPIC_BASE_URL";

/// `ANTHROPIC_API_KEY` — auth key. Forwarded from the keyring fallback
/// chain (see `config::keyring::read_azure_key`).
pub const ENV_ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

/// `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` — undocumented Claude Code env that
/// suppresses the `anthropic-beta: advisor-tool-2026-03-01` header. Azure
/// Anthropic-compatible endpoints reject that header with HTTP 400, so
/// codebus forces this flag on whenever it injects Azure routing.
pub const ENV_DISABLE_ADVISOR_TOOL: &str = "CLAUDE_CODE_DISABLE_ADVISOR_TOOL";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EnvOverrides {
    entries: BTreeMap<String, String>,
}

impl EnvOverrides {
    /// Empty map. The system profile injects no env vars — the spawned
    /// child inherits the parent shell verbatim.
    pub fn for_system() -> Self {
        Self::default()
    }

    /// Azure profile injection: `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`,
    /// `CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1`. The advisor-tool override is
    /// non-negotiable; v2 strategy memo §8 proved Azure 400s without it.
    pub fn for_azure(base_url: &str, api_key: &str) -> Self {
        let mut entries = BTreeMap::new();
        entries.insert(ENV_ANTHROPIC_BASE_URL.to_string(), base_url.to_string());
        entries.insert(ENV_ANTHROPIC_API_KEY.to_string(), api_key.to_string());
        entries.insert(ENV_DISABLE_ADVISOR_TOOL.to_string(), "1".to_string());
        Self { entries }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Look up a key. Used by tests + by `claude_cli::invoke` debug
    /// paths.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: System profile injects no env.
    #[test]
    fn for_system_returns_empty_map() {
        let env = EnvOverrides::for_system();
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
        assert!(env.iter().next().is_none());
    }

    /// Spec: Azure profile injects exactly three env vars.
    #[test]
    fn for_azure_has_exactly_three_keys() {
        let env = EnvOverrides::for_azure(
            "https://example.cognitiveservices.azure.com/anthropic",
            "sk-test",
        );
        assert_eq!(env.len(), 3);
        let keys: Vec<&String> = env.iter().map(|(k, _)| k).collect();
        assert!(keys.iter().any(|k| *k == ENV_ANTHROPIC_BASE_URL));
        assert!(keys.iter().any(|k| *k == ENV_ANTHROPIC_API_KEY));
        assert!(keys.iter().any(|k| *k == ENV_DISABLE_ADVISOR_TOOL));
    }

    /// Spec: `CLAUDE_CODE_DISABLE_ADVISOR_TOOL` value SHALL be the literal
    /// string `"1"` (not "true", not "on"). Claude Code only honours `1`.
    #[test]
    fn for_azure_disable_advisor_tool_value_is_literal_one() {
        let env = EnvOverrides::for_azure("https://x.example.com/anthropic", "sk");
        assert_eq!(env.get(ENV_DISABLE_ADVISOR_TOOL), Some("1"));
    }

    /// Values flow verbatim — codebus does not normalise URL or key.
    #[test]
    fn for_azure_values_flow_verbatim() {
        let env = EnvOverrides::for_azure(
            "https://example.cognitiveservices.azure.com/anthropic",
            "sk-secret-12345",
        );
        assert_eq!(
            env.get(ENV_ANTHROPIC_BASE_URL),
            Some("https://example.cognitiveservices.azure.com/anthropic"),
        );
        assert_eq!(env.get(ENV_ANTHROPIC_API_KEY), Some("sk-secret-12345"));
    }

    /// Iteration order is deterministic (BTreeMap by name) — relied on
    /// by spawn-path snapshot tests that capture the env list as a Vec.
    #[test]
    fn for_azure_iteration_order_is_sorted_by_key() {
        let env = EnvOverrides::for_azure("u", "k");
        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                ENV_ANTHROPIC_API_KEY,
                ENV_ANTHROPIC_BASE_URL,
                ENV_DISABLE_ADVISOR_TOOL,
            ]
        );
    }
}
