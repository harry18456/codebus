//! LLM provider factory — single point that turns a [`ProviderConfig`] into a
//! `Box<dyn LlmProvider>`. Adding a new provider in this codebase is two
//! mechanical edits:
//!
//! 1. Add a variant to [`ProviderConfig`] (carrying any provider-specific
//!    fields directly on the variant — `binary_path` for a CLI subprocess,
//!    `api_key`/`timeout_secs` for HTTP providers, etc.).
//! 2. Add a match arm to [`build_provider`] (feature-gated when the impl
//!    pulls in heavy deps).
//!
//! The match is intentionally explicit — no `inventory` / `linkme` magic. A
//! reader who opens this file sees every supported provider in one place.
//!
//! ## Tagged-enum config shape
//!
//! [`ProviderConfig`] is a serde-tagged enum: the YAML/JSON discriminator is
//! the `provider:` field, and each variant carries only the fields that
//! actually mean something to that provider. This is a deliberate move away
//! from the earlier "flat struct + `ProviderKind` discriminator" shape — that
//! design let `binary_path` coexist with `api_key` in the type system even
//! though no real provider uses both, which made the contract harder to read
//! and harder to extend with model/effort/fallback knobs in follow-up
//! changes.
//!
//! Example YAML:
//!
//! ```yaml
//! llm:
//!   provider: claude_cli
//!   binary_path: /opt/claude/bin/claude
//! ```
//!
//! ```yaml
//! llm:
//!   provider: anthropic_api
//!   api_key: sk-ant-...
//!   timeout_secs: 60
//! ```

use serde::{Deserialize, Serialize};

use crate::llm::provider::{LlmProvider, ProviderError};
use crate::llm::providers::claude_cli::ClaudeCliProvider;

/// Provider configuration as a serde-tagged enum.
///
/// The YAML/JSON discriminator is the `provider` field (`snake_case`):
/// `claude_cli`, `anthropic_api`, `openai`, `ollama_local`. Each variant
/// carries only the fields that actually apply to it — there is no shared
/// flat struct, so e.g. `binary_path` is unreachable in YAML when `provider:
/// anthropic_api` is selected.
///
/// All variants are always present in the enum (regardless of cargo
/// features) so that the [`config`](crate::config) loader can deserialize
/// any valid config; when a feature is off, the corresponding match arm in
/// [`build_provider`] returns [`ProviderError::FeatureNotCompiled`] instead
/// of constructing the impl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum ProviderConfig {
    /// `claude -p` subprocess. Always available — zero new deps.
    ///
    /// - `binary_path`: path to the `claude` CLI binary. `None` (or omitted
    ///   in YAML) → `"claude"` (rely on `$PATH`). An empty string is also
    ///   treated as unset (defensive against config typos like
    ///   `binary_path: ""`).
    /// - `model`: optional Claude CLI `--model` value (alias like `sonnet`,
    ///   `opus`, `haiku` or full model id). `None` → omit the flag, let the
    ///   CLI pick its default. Stored as opaque string; codebus does not
    ///   validate the value (alias list evolves with the CLI).
    /// - `effort`: optional Claude CLI `--effort` value (`low`, `medium`,
    ///   `high`, `xhigh`, `max`). Same opaque-forward semantics as `model`.
    ClaudeCli {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        binary_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<String>,
    },
    /// Direct Anthropic API (HTTP + SSE). Requires `llm-anthropic-api`
    /// feature.
    ///
    /// `api_key`: Anthropic API key. `timeout_secs`: HTTP request timeout
    /// (advisory until the impl lands in a follow-up change).
    AnthropicApi {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
    /// OpenAI-compatible API (HTTP + SSE). Requires `llm-openai` feature.
    ///
    /// `api_key`: OpenAI API key. `timeout_secs`: HTTP request timeout
    /// (advisory until the impl lands in a follow-up change).
    ///
    /// Variant name is `Openai` (not `OpenAi`) so that
    /// `#[serde(rename_all = "snake_case")]` yields the discriminator value
    /// `openai` rather than `open_ai`.
    Openai {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
    /// Local Ollama server (HTTP). Reserved; impl lands in a future change.
    OllamaLocal {},
}

impl Default for ProviderConfig {
    /// `claude_cli` with `$PATH` lookup. This matches the Phase 1 default
    /// (any user with the Claude CLI installed gets a working setup with no
    /// configuration).
    fn default() -> Self {
        Self::ClaudeCli {
            binary_path: None,
            model: None,
            effort: None,
        }
    }
}

/// Build a provider from a [`ProviderConfig`].
///
/// Returns [`ProviderError::FeatureNotCompiled`] when the requested variant
/// maps to a feature that wasn't compiled into this binary.
pub fn build_provider(cfg: ProviderConfig) -> Result<Box<dyn LlmProvider>, ProviderError> {
    match cfg {
        ProviderConfig::ClaudeCli {
            binary_path,
            model: _,
            effort: _,
        } => {
            // model + effort are not consumed at provider construction time;
            // they're carried per-invocation via InvokeOptions and turned
            // into argv flags by build_argv. Keeping the destructure
            // exhaustive so future additions to the variant are caught at
            // compile time.
            let binary = binary_path
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "claude".into());
            Ok(Box::new(ClaudeCliProvider::with_binary(binary)))
        }

        #[cfg(feature = "llm-anthropic-api")]
        ProviderConfig::AnthropicApi { .. } => {
            unimplemented!("llm-anthropic-api impl lands in a follow-up change")
        }

        #[cfg(not(feature = "llm-anthropic-api"))]
        ProviderConfig::AnthropicApi { .. } => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-anthropic-api",
            hint: "rebuild with: cargo install codebus --features llm-anthropic-api",
        }),

        #[cfg(feature = "llm-openai")]
        ProviderConfig::Openai { .. } => {
            unimplemented!("llm-openai impl lands in a follow-up change")
        }

        #[cfg(not(feature = "llm-openai"))]
        ProviderConfig::Openai { .. } => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-openai",
            hint: "rebuild with: cargo install codebus --features llm-openai",
        }),

        ProviderConfig::OllamaLocal {} => Err(ProviderError::FeatureNotCompiled {
            feature: "llm-ollama-local",
            hint: "ollama_local provider not yet implemented",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Default ----------

    #[test]
    fn default_config_is_claude_cli_with_no_binary_path() {
        match ProviderConfig::default() {
            ProviderConfig::ClaudeCli {
                binary_path,
                model,
                effort,
            } => {
                assert!(binary_path.is_none(), "default binary_path should be None");
                assert!(model.is_none(), "default model should be None");
                assert!(effort.is_none(), "default effort should be None");
            }
            other => panic!("expected ClaudeCli default, got {other:?}"),
        }
    }

    // ---------- YAML deserialization (round-trip the tagged-enum shape) ----------

    #[test]
    fn deserializes_claude_cli_with_binary_path_from_yaml() {
        let yaml = r#"
provider: claude_cli
binary_path: /opt/claude/bin/claude
"#;
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        match cfg {
            ProviderConfig::ClaudeCli {
                binary_path,
                model,
                effort,
            } => {
                assert_eq!(binary_path.as_deref(), Some("/opt/claude/bin/claude"));
                assert!(model.is_none());
                assert!(effort.is_none());
            }
            other => panic!("expected ClaudeCli, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_claude_cli_without_binary_path_from_yaml() {
        // `binary_path` is `#[serde(default)]` — omitting it must yield None,
        // not a deserialization error.
        let yaml = "provider: claude_cli\n";
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        match cfg {
            ProviderConfig::ClaudeCli {
                binary_path,
                model,
                effort,
            } => {
                assert!(binary_path.is_none());
                assert!(model.is_none());
                assert!(effort.is_none());
            }
            other => panic!("expected ClaudeCli, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_claude_cli_with_model_and_effort_from_yaml() {
        let yaml = r#"
provider: claude_cli
model: sonnet
effort: high
"#;
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        match cfg {
            ProviderConfig::ClaudeCli {
                binary_path,
                model,
                effort,
            } => {
                assert!(binary_path.is_none());
                assert_eq!(model.as_deref(), Some("sonnet"));
                assert_eq!(effort.as_deref(), Some("high"));
            }
            other => panic!("expected ClaudeCli, got {other:?}"),
        }
    }

    #[test]
    fn round_trips_claude_cli_with_all_fields() {
        let original = ProviderConfig::ClaudeCli {
            binary_path: Some("/opt/claude".into()),
            model: Some("opus".into()),
            effort: Some("xhigh".into()),
        };
        let yaml = serde_yaml::to_string(&original).expect("serialize");
        let parsed: ProviderConfig = serde_yaml::from_str(&yaml).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn deserializes_anthropic_api_from_yaml() {
        let yaml = r#"
provider: anthropic_api
api_key: sk-ant-test
timeout_secs: 60
"#;
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        match cfg {
            ProviderConfig::AnthropicApi {
                api_key,
                timeout_secs,
            } => {
                assert_eq!(api_key.as_deref(), Some("sk-ant-test"));
                assert_eq!(timeout_secs, Some(60));
            }
            other => panic!("expected AnthropicApi, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_openai_from_yaml() {
        let yaml = r#"
provider: openai
api_key: sk-openai-test
timeout_secs: 30
"#;
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        match cfg {
            ProviderConfig::Openai {
                api_key,
                timeout_secs,
            } => {
                assert_eq!(api_key.as_deref(), Some("sk-openai-test"));
                assert_eq!(timeout_secs, Some(30));
            }
            other => panic!("expected Openai, got {other:?}"),
        }
    }

    #[test]
    fn deserializes_ollama_local_from_yaml() {
        let yaml = "provider: ollama_local\n";
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("should deserialize");
        assert!(matches!(cfg, ProviderConfig::OllamaLocal {}));
    }

    #[test]
    fn unknown_provider_tag_fails_to_deserialize() {
        // Strict variant matching — typoed `provider:` values must surface as
        // a config error rather than silently picking a default.
        let yaml = "provider: not_a_real_provider\n";
        let result: Result<ProviderConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "unknown provider tag should fail");
    }

    // ---------- build_provider dispatch ----------

    #[test]
    fn build_provider_returns_box_dyn_for_claude_cli_default() {
        let provider: Box<dyn LlmProvider> = build_provider(ProviderConfig::default())
            .expect("default ClaudeCli config should build");
        // Smoke: cancel is a no-op on idle ClaudeCliProvider — calling it
        // verifies we got a usable trait object.
        provider.cancel();
    }

    #[test]
    fn build_provider_honors_explicit_binary_path() {
        let provider = build_provider(ProviderConfig::ClaudeCli {
            binary_path: Some("/opt/claude/bin/claude".into()),
            model: None,
            effort: None,
        });
        assert!(provider.is_ok());
    }

    #[test]
    fn build_provider_treats_empty_binary_path_as_unset() {
        // Defensive: `binary_path: ""` in YAML is a likely typo. We default
        // to "claude" rather than spawning an empty path (which would fail
        // with a confusing error). Audit / Lazy-developer lens.
        let provider = build_provider(ProviderConfig::ClaudeCli {
            binary_path: Some(String::new()),
            model: None,
            effort: None,
        });
        assert!(provider.is_ok());
    }

    #[test]
    #[cfg(not(feature = "llm-anthropic-api"))]
    fn build_provider_returns_feature_not_compiled_for_anthropic_api() {
        let result = build_provider(ProviderConfig::AnthropicApi {
            api_key: None,
            timeout_secs: None,
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
        let result = build_provider(ProviderConfig::Openai {
            api_key: None,
            timeout_secs: None,
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
        let result = build_provider(ProviderConfig::OllamaLocal {});
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

    // ---------- serde round-trip tests (spec §1.1 acceptance criteria) ----------

    #[test]
    fn default_is_claude_cli_with_no_binary_path() {
        // Pinning the default: this is what loader.rs falls back to when
        // `llm:` is missing or its discriminator is unknown. Changing the
        // default away from `ClaudeCli { binary_path: None }` is a breaking
        // behaviour change for every existing user; force any future change
        // to update this test deliberately.
        assert_eq!(
            ProviderConfig::default(),
            ProviderConfig::ClaudeCli {
                binary_path: None,
                model: None,
                effort: None,
            }
        );
    }

    #[test]
    fn claude_cli_default_round_trips_via_serde_yaml() {
        let cfg = ProviderConfig::default();
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        let parsed: ProviderConfig = serde_yaml::from_str(&yaml).expect("round-trip");
        assert_eq!(parsed, cfg);
        // `skip_serializing_if = "Option::is_none"` keeps unset fields out of
        // the emitted YAML — only the discriminator should appear.
        assert!(
            yaml.contains("provider: claude_cli"),
            "yaml should contain discriminator: {yaml}"
        );
        assert!(
            !yaml.contains("binary_path"),
            "binary_path should be skipped when None: {yaml}"
        );
    }

    #[test]
    fn anthropic_api_round_trips_with_api_key() {
        let cfg = ProviderConfig::AnthropicApi {
            api_key: Some("sk-ant-roundtrip".into()),
            timeout_secs: Some(45),
        };
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        assert!(yaml.contains("provider: anthropic_api"));
        assert!(yaml.contains("api_key: sk-ant-roundtrip"));
        assert!(yaml.contains("timeout_secs: 45"));
        let parsed: ProviderConfig = serde_yaml::from_str(&yaml).expect("round-trip");
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn openai_round_trips() {
        // Variant is named `Openai` (not `OpenAi`) on purpose: the serde
        // attribute `rename_all = "snake_case"` would turn `OpenAi` into
        // `open_ai`, but we want the discriminator string to be the
        // industry-standard `openai`.
        let cfg = ProviderConfig::Openai {
            api_key: Some("sk-openai-roundtrip".into()),
            timeout_secs: Some(20),
        };
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        assert!(
            yaml.contains("provider: openai"),
            "expected discriminator `openai`, got: {yaml}"
        );
        assert!(
            !yaml.contains("open_ai"),
            "must NOT serialize as `open_ai`: {yaml}"
        );
        let parsed: ProviderConfig = serde_yaml::from_str(&yaml).expect("round-trip");
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn ollama_local_serializes_with_just_provider_field() {
        // Empty struct variant: the only thing on the wire should be the
        // discriminator. Acts as a regression guard against accidentally
        // adding a sentinel field that would break backwards-compatible
        // YAML.
        let cfg = ProviderConfig::OllamaLocal {};
        let yaml = serde_yaml::to_string(&cfg).expect("serialize");
        assert!(yaml.contains("provider: ollama_local"));
        let parsed: ProviderConfig = serde_yaml::from_str(&yaml).expect("round-trip");
        assert_eq!(parsed, cfg);
    }
}
