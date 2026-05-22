//! Runtime provider dispatch — maps the configured `active_provider` to a
//! concrete [`AgentBackend`]. Verbs construct their backend through this layer
//! instead of hard-coding [`ClaudeBackend`], so flipping `active_provider`
//! re-routes every verb's spawn (see `codex-backend` spec `Provider Dispatch
//! Selection`).
//!
//! [`ProviderConfig`] carries the loaded per-provider endpoint config and
//! offers provider-agnostic `resolve(verb)` so the verb layer records the
//! model/effort in its `RunLog` regardless of provider.

use std::fs;
use std::path::Path;

use crate::config::claude_code::build_env_overrides;
use crate::config::endpoint::{
    ActiveProfile, ParseOutcome, parse_claude_code_yaml, parse_codex_yaml, read_active_provider,
};
use crate::config::keyring::read_azure_key;
use crate::config::{
    ClaudeCodeConfig, CodexConfig, ConfigLoadError, KeyringError, ResolvedVerb, Verb,
};

use super::backend::AgentBackend;
use super::claude_backend::ClaudeBackend;
use super::codex_backend::CodexBackend;

/// The loaded endpoint config of the active provider.
pub enum ProviderConfig {
    Claude(ClaudeCodeConfig),
    Codex(CodexConfig),
}

impl ProviderConfig {
    /// Resolve a verb's model/effort under whichever provider is active.
    pub fn resolve(&self, verb: Verb) -> ResolvedVerb {
        match self {
            ProviderConfig::Claude(c) => c.resolve(verb),
            ProviderConfig::Codex(c) => c.resolve(verb),
        }
    }
}

impl Default for ProviderConfig {
    /// Default to the claude provider with built-in defaults — preserves the
    /// pre-multi-provider fallback used by verbs when no config file exists.
    fn default() -> Self {
        ProviderConfig::Claude(ClaudeCodeConfig::default())
    }
}

/// Load the active provider's endpoint config from `path`. A missing file (or
/// absent `agent` block) defaults to the claude provider — preserving the
/// pre-multi-provider behavior.
pub fn load_provider_config(path: &Path) -> Result<ProviderConfig, ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProviderConfig::Claude(ClaudeCodeConfig::default()));
        }
        Err(e) => return Err(ConfigLoadError::Io(e)),
    };
    parse_provider_config(&body)
}

/// Parse `~/.codebus/config.yaml` text into the active provider's config.
/// Routes on `agent.active_provider`: `codex` loads the codex block (error if
/// absent), anything else goes through the claude loader (which rejects names
/// other than `claude`/`codex`).
pub fn parse_provider_config(yaml: &str) -> Result<ProviderConfig, ConfigLoadError> {
    if read_active_provider(yaml)? == "codex" {
        return match parse_codex_yaml(yaml)? {
            Some(c) => Ok(ProviderConfig::Codex(c)),
            None => Err(ConfigLoadError::YamlParse(serde::de::Error::custom(
                "agent.active_provider: codex but `agent.providers.codex` block is missing",
            ))),
        };
    }
    match parse_claude_code_yaml(yaml)? {
        ParseOutcome::New(c) => Ok(ProviderConfig::Claude(c)),
        ParseOutcome::Missing => Ok(ProviderConfig::Claude(ClaudeCodeConfig::default())),
    }
}

/// Build the concrete backend for the loaded provider config. Reads the Azure
/// key (keyring → env fallback) when the active profile is azure; backend
/// construction is otherwise infallible.
pub fn build_backend(cfg: &ProviderConfig) -> Result<Box<dyn AgentBackend>, KeyringError> {
    match cfg {
        ProviderConfig::Claude(c) => {
            let env = build_env_overrides(c)?;
            Ok(Box::new(ClaudeBackend::new(c.clone(), env)))
        }
        ProviderConfig::Codex(c) => {
            let key = match c.active {
                ActiveProfile::Azure => {
                    let az = c
                        .azure
                        .as_ref()
                        .expect("azure profile populated when active=azure (validated on load)");
                    Some(read_azure_key(&az.keyring_service)?)
                }
                ActiveProfile::System => None,
            };
            Ok(Box::new(CodexBackend::new(c.clone(), key)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODEX_YAML: &str = "agent:\n  active_provider: codex\n  providers:\n    codex:\n      active: system\n      system:\n        goal:   { model: gpt-5.5, effort: high }\n        query:  { model: gpt-5.5, effort: low }\n        fix:    { model: gpt-5.5, effort: medium }\n        verify: { model: gpt-5.5, effort: high }\n";

    const CLAUDE_YAML: &str = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: high }\n        query:  { model: haiku-4-5,  effort: low }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high }\n";

    /// Spec: Codex provider routes to CodexBackend (config variant Codex).
    #[test]
    fn codex_provider_routes_to_codex() {
        let cfg = parse_provider_config(CODEX_YAML).unwrap();
        assert!(matches!(cfg, ProviderConfig::Codex(_)));
    }

    /// Spec: Claude provider routes to ClaudeBackend (config variant Claude).
    #[test]
    fn claude_provider_routes_to_claude() {
        let cfg = parse_provider_config(CLAUDE_YAML).unwrap();
        assert!(matches!(cfg, ProviderConfig::Claude(_)));
    }

    /// Spec: absent active_provider defaults to claude.
    #[test]
    fn absent_provider_defaults_to_claude() {
        let cfg = parse_provider_config("pii:\n  scanner: regex_basic\n").unwrap();
        assert!(matches!(cfg, ProviderConfig::Claude(_)));
    }

    /// build_backend constructs a backend for the codex system profile
    /// without needing a keyring entry.
    #[test]
    fn build_backend_codex_system_no_keyring() {
        let cfg = parse_provider_config(CODEX_YAML).unwrap();
        assert!(build_backend(&cfg).is_ok());
    }
}
