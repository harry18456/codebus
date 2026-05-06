//! LLM provider factory — single point that turns a [`ProviderConfig`] into a
//! `Box<dyn LlmProvider>`. Adding a new provider in this codebase is two
//! mechanical edits:
//!
//! 1. Add a variant to [`ProviderKind`].
//! 2. Add a match arm to [`build_provider`] (feature-gated when the impl
//!    pulls in heavy deps).
//!
//! The match is intentionally explicit — no `inventory` / `linkme` magic. A
//! reader who opens this file sees every supported provider in one place.

use crate::llm::provider::{LlmProvider, ProviderError};
use crate::llm::providers::claude_cli::ClaudeCliProvider;

/// Discriminator for which provider implementation to build.
///
/// All variants are always present in the enum so that the [`config`](crate::config)
/// loader (R6) can map config strings to a concrete kind regardless of which
/// cargo features are compiled. When a feature is off, the corresponding
/// match arm in [`build_provider`] returns
/// [`ProviderError::FeatureNotCompiled`] instead of constructing the impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderKind {
    /// `claude -p` subprocess. Always available — zero new deps.
    #[default]
    ClaudeCli,
    /// Direct Anthropic API (HTTP + SSE). Requires `llm-anthropic-api` feature.
    AnthropicApi,
    /// OpenAI-compatible API (HTTP + SSE). Requires `llm-openai` feature.
    OpenAi,
    /// Local Ollama server (HTTP). Reserved; impl lands in a future change.
    OllamaLocal,
}

/// Provider configuration. Fields not relevant to the chosen
/// [`ProviderKind`] are ignored. All fields default to `None`; the factory
/// supplies sensible defaults per kind.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    /// `claude_cli`: path to the `claude` CLI binary. `None` → `"claude"`.
    /// Empty string is also treated as unset (defensive against config typos
    /// like `binary_path: ""`).
    pub binary_path: Option<String>,
    /// Subprocess / HTTP timeout. Currently advisory — the `claude_cli`
    /// provider does not yet enforce it. Reserved for HTTP providers.
    pub timeout_secs: Option<u64>,
    /// API key for HTTP providers (`anthropic_api`, `openai`). Ignored by
    /// `claude_cli` (the CLI handles its own auth).
    pub api_key: Option<String>,
}

/// Build a provider from a [`ProviderConfig`].
///
/// Returns [`ProviderError::FeatureNotCompiled`] when the requested kind
/// maps to a feature that wasn't compiled into this binary.
pub fn build_provider(cfg: ProviderConfig) -> Result<Box<dyn LlmProvider>, ProviderError> {
    match cfg.kind {
        ProviderKind::ClaudeCli => {
            let binary = cfg
                .binary_path
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "claude".into());
            Ok(Box::new(ClaudeCliProvider::with_binary(binary)))
        }

        #[cfg(feature = "llm-anthropic-api")]
        ProviderKind::AnthropicApi => {
            unimplemented!("llm-anthropic-api impl lands in a follow-up change")
        }

        #[cfg(not(feature = "llm-anthropic-api"))]
        ProviderKind::AnthropicApi => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-anthropic-api",
            hint: "rebuild with: cargo install codebus --features llm-anthropic-api",
        }),

        #[cfg(feature = "llm-openai")]
        ProviderKind::OpenAi => {
            unimplemented!("llm-openai impl lands in a follow-up change")
        }

        #[cfg(not(feature = "llm-openai"))]
        ProviderKind::OpenAi => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-openai",
            hint: "rebuild with: cargo install codebus --features llm-openai",
        }),

        ProviderKind::OllamaLocal => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-ollama-local",
            hint: "ollama_local provider not yet implemented",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_claude_cli_kind() {
        let cfg = ProviderConfig::default();
        assert_eq!(cfg.kind, ProviderKind::ClaudeCli);
        assert!(cfg.binary_path.is_none());
        assert!(cfg.timeout_secs.is_none());
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn build_provider_returns_box_dyn_for_claude_cli_default() {
        let provider: Box<dyn LlmProvider> = build_provider(ProviderConfig {
            kind: ProviderKind::ClaudeCli,
            ..Default::default()
        })
        .expect("default ClaudeCli config should build");
        // Smoke: cancel is a no-op on idle ClaudeCliProvider — calling it
        // verifies we got a usable trait object.
        provider.cancel();
    }

    #[test]
    fn build_provider_honors_explicit_binary_path() {
        let provider = build_provider(ProviderConfig {
            kind: ProviderKind::ClaudeCli,
            binary_path: Some("/opt/claude/bin/claude".into()),
            ..Default::default()
        });
        assert!(provider.is_ok());
    }

    #[test]
    fn build_provider_treats_empty_binary_path_as_unset() {
        // Defensive: `binary_path: ""` in YAML is a likely typo. We default
        // to "claude" rather than spawning an empty path (which would fail
        // with a confusing error). Audit / Lazy-developer lens.
        let provider = build_provider(ProviderConfig {
            kind: ProviderKind::ClaudeCli,
            binary_path: Some(String::new()),
            ..Default::default()
        });
        assert!(provider.is_ok());
    }

    #[test]
    #[cfg(not(feature = "llm-anthropic-api"))]
    fn build_provider_returns_feature_not_compiled_for_anthropic_api() {
        let result = build_provider(ProviderConfig {
            kind: ProviderKind::AnthropicApi,
            ..Default::default()
        });
        match result {
            Ok(_) => panic!("expected FeatureNotCompiled, got Ok"),
            Err(ProviderError::FeatureNotCompiled { feature, .. }) => {
                assert_eq!(feature, "llm-anthropic-api");
            }
            Err(other) => panic!("expected FeatureNotCompiled, got {other:?}"),
        }
    }

    #[test]
    #[cfg(not(feature = "llm-openai"))]
    fn build_provider_returns_feature_not_compiled_for_openai() {
        let result = build_provider(ProviderConfig {
            kind: ProviderKind::OpenAi,
            ..Default::default()
        });
        match result {
            Ok(_) => panic!("expected FeatureNotCompiled, got Ok"),
            Err(ProviderError::FeatureNotCompiled { feature, .. }) => {
                assert_eq!(feature, "llm-openai");
            }
            Err(other) => panic!("expected FeatureNotCompiled, got {other:?}"),
        }
    }

    #[test]
    fn build_provider_returns_feature_not_compiled_for_ollama_local() {
        let result = build_provider(ProviderConfig {
            kind: ProviderKind::OllamaLocal,
            ..Default::default()
        });
        match result {
            Ok(_) => panic!("expected FeatureNotCompiled, got Ok"),
            Err(ProviderError::FeatureNotCompiled { feature, .. }) => {
                assert_eq!(feature, "llm-ollama-local");
            }
            Err(other) => panic!("expected FeatureNotCompiled, got {other:?}"),
        }
    }

    #[test]
    fn feature_not_compiled_display_includes_feature_and_hint() {
        let e = ProviderError::FeatureNotCompiled {
            feature: "llm-openai",
            hint: "rebuild with: cargo install codebus --features llm-openai",
        };
        let s = format!("{e}");
        assert!(s.contains("llm-openai"));
        assert!(s.contains("rebuild"));
    }
}
