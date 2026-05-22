//! Codex provider endpoint configuration — the `agent.providers.codex`
//! block of `~/.codebus/config.yaml`.
//!
//! Mirrors the claude provider's `system` / `azure` profile shape (see
//! [`super::endpoint`]) with two codex-specific differences:
//!
//! - The `system` profile `model` field is an arbitrary non-empty string
//!   (a codex model name like `gpt-5.5`); codex model names are NOT a closed
//!   enum, so unknown strings are passed through verbatim, never rejected.
//! - The `azure` profile carries an extra `api_version` field (e.g.
//!   `2025-04-01-preview`) because codex targets the Azure OpenAI Responses
//!   API; `keyring_service` defaults to `codebus-codex-azure` (distinct from
//!   claude's `codebus-claude-azure`) so the two providers' Azure keys do not
//!   collide. A user wanting a shared key can set `keyring_service` explicitly.

use serde::Deserialize;

use super::ConfigLoadError;
use super::Verb;
use super::claude_code::ResolvedVerb;
use super::endpoint::ActiveProfile;

/// One verb's codex settings: `model` is an arbitrary non-empty string and
/// `effort` is an arbitrary string forwarded as `model_reasoning_effort`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodexVerbConfig {
    pub model: String,
    pub effort: String,
}

/// Four verb settings under the codex system profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexSystemProfile {
    pub goal: CodexVerbConfig,
    pub query: CodexVerbConfig,
    pub fix: CodexVerbConfig,
    pub verify: CodexVerbConfig,
}

/// Codex Azure endpoint profile. `base_url` (Azure resource, e.g. ending in
/// `/openai`), `api_version`, and `keyring_service` are required when active;
/// each verb is a freeform deployment-name + effort pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAzureProfile {
    pub base_url: String,
    pub api_version: String,
    pub keyring_service: String,
    pub goal: CodexVerbConfig,
    pub query: CodexVerbConfig,
    pub fix: CodexVerbConfig,
    pub verify: CodexVerbConfig,
}

/// The full `agent.providers.codex` block, parsed and validated. The active
/// profile is guaranteed populated; the non-active profile is `None` or
/// preserved verbatim but not validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfig {
    pub active: ActiveProfile,
    pub system: Option<CodexSystemProfile>,
    pub azure: Option<CodexAzureProfile>,
}

impl CodexConfig {
    /// Resolve a verb's model/effort under the active profile. Chat/Quiz
    /// reuse Query; Verify resolves to its own sub-block (mirrors the claude
    /// provider's resolution).
    pub fn resolve(&self, verb: Verb) -> ResolvedVerb {
        let v = match self.active {
            ActiveProfile::System => {
                let p = self
                    .system
                    .as_ref()
                    .expect("system profile populated when active=system (validated on load)");
                pick_verb_system(p, verb)
            }
            ActiveProfile::Azure => {
                let p = self
                    .azure
                    .as_ref()
                    .expect("azure profile populated when active=azure (validated on load)");
                pick_verb_azure(p, verb)
            }
        };
        ResolvedVerb {
            model: Some(v.model.clone()),
            effort: Some(v.effort.clone()),
        }
    }
}

fn pick_verb_system(p: &CodexSystemProfile, verb: Verb) -> &CodexVerbConfig {
    match verb {
        Verb::Goal => &p.goal,
        Verb::Fix => &p.fix,
        Verb::Verify => &p.verify,
        Verb::Query | Verb::Chat | Verb::Quiz => &p.query,
    }
}

fn pick_verb_azure(p: &CodexAzureProfile, verb: Verb) -> &CodexVerbConfig {
    match verb {
        Verb::Goal => &p.goal,
        Verb::Fix => &p.fix,
        Verb::Verify => &p.verify,
        Verb::Query | Verb::Chat | Verb::Quiz => &p.query,
    }
}

// ---------------------------------------------------------------------------
// Raw deserialization scaffolding
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawCodexProvider {
    #[serde(default)]
    pub active: Option<ActiveProfile>,
    #[serde(default)]
    pub system: Option<RawCodexSystemProfile>,
    #[serde(default)]
    pub azure: Option<RawCodexAzureProfile>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawCodexSystemProfile {
    #[serde(default)]
    pub goal: Option<CodexVerbConfig>,
    #[serde(default)]
    pub query: Option<CodexVerbConfig>,
    #[serde(default)]
    pub fix: Option<CodexVerbConfig>,
    #[serde(default)]
    pub verify: Option<CodexVerbConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawCodexAzureProfile {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub keyring_service: Option<String>,
    #[serde(default)]
    pub goal: Option<CodexVerbConfig>,
    #[serde(default)]
    pub query: Option<CodexVerbConfig>,
    #[serde(default)]
    pub fix: Option<CodexVerbConfig>,
    #[serde(default)]
    pub verify: Option<CodexVerbConfig>,
}

/// Default keyring service for the codex azure profile — shared with the
/// claude provider so a single Azure key serves both.
const DEFAULT_AZURE_KEYRING_SERVICE: &str = "codebus-codex-azure";

/// Validate a `RawCodexProvider` (already deserialized from the
/// `agent.providers.codex` block) into a [`CodexConfig`]. The active profile
/// MUST be fully populated; the non-active profile is dropped to `None`.
pub(super) fn validate_codex_provider(
    raw: RawCodexProvider,
) -> Result<CodexConfig, ConfigLoadError> {
    let active = raw.active.unwrap_or(ActiveProfile::System);
    let system = validate_codex_system(active, raw.system)?;
    let azure = validate_codex_azure(active, raw.azure)?;
    Ok(CodexConfig {
        active,
        system,
        azure,
    })
}

fn validate_codex_system(
    active: ActiveProfile,
    raw: Option<RawCodexSystemProfile>,
) -> Result<Option<CodexSystemProfile>, ConfigLoadError> {
    match (active, raw) {
        (ActiveProfile::System, None) => Err(err(
            "agent.providers.codex.active: system but `system` block is missing",
        )),
        (ActiveProfile::System, Some(raw)) => Ok(Some(CodexSystemProfile {
            goal: require_verb(raw.goal, "codex.system.goal")?,
            query: require_verb(raw.query, "codex.system.query")?,
            fix: require_verb(raw.fix, "codex.system.fix")?,
            verify: require_verb(raw.verify, "codex.system.verify")?,
        })),
        (ActiveProfile::Azure, _) => Ok(None),
    }
}

fn validate_codex_azure(
    active: ActiveProfile,
    raw: Option<RawCodexAzureProfile>,
) -> Result<Option<CodexAzureProfile>, ConfigLoadError> {
    match (active, raw) {
        (ActiveProfile::Azure, None) => Err(err(
            "agent.providers.codex.active: azure but `azure` block is missing",
        )),
        (ActiveProfile::Azure, Some(raw)) => Ok(Some(CodexAzureProfile {
            base_url: require_non_empty(raw.base_url, "codex.azure.base_url")?,
            api_version: require_non_empty(raw.api_version, "codex.azure.api_version")?,
            keyring_service: raw
                .keyring_service
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_AZURE_KEYRING_SERVICE.to_string()),
            goal: require_verb(raw.goal, "codex.azure.goal")?,
            query: require_verb(raw.query, "codex.azure.query")?,
            fix: require_verb(raw.fix, "codex.azure.fix")?,
            verify: require_verb(raw.verify, "codex.azure.verify")?,
        })),
        (ActiveProfile::System, _) => Ok(None),
    }
}

fn require_verb(value: Option<CodexVerbConfig>, field: &str) -> Result<CodexVerbConfig, ConfigLoadError> {
    match value {
        Some(v) if !v.model.is_empty() => Ok(v),
        Some(_) => Err(err(&format!("{field}.model: required and non-empty"))),
        None => Err(err(&format!("{field}: required when this profile is active"))),
    }
}

fn require_non_empty(value: Option<String>, field: &str) -> Result<String, ConfigLoadError> {
    match value {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(err(&format!("{field}: required and non-empty"))),
    }
}

fn err(msg: &str) -> ConfigLoadError {
    ConfigLoadError::YamlParse(<serde_yaml::Error as serde::de::Error>::custom(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::endpoint::parse_codex_yaml;

    fn codex_yaml(body: &str) -> String {
        format!("agent:\n  active_provider: codex\n  providers:\n    codex:\n{body}")
    }

    /// Spec: Codex Provider Config Schema — system profile accepts arbitrary
    /// (non-enum) model strings.
    #[test]
    fn codex_system_profile_accepts_arbitrary_models() {
        let yaml = codex_yaml(
            "      active: system\n      system:\n        goal:   { model: gpt-5.5, effort: high }\n        query:  { model: gpt-5.5-mini, effort: low }\n        fix:    { model: gpt-5.5, effort: medium }\n        verify: { model: gpt-5.5, effort: high }\n",
        );
        let cfg = parse_codex_yaml(&yaml).unwrap().expect("codex config present");
        assert_eq!(cfg.active, ActiveProfile::System);
        let sys = cfg.system.expect("system populated");
        assert_eq!(sys.goal.model, "gpt-5.5");
        assert_eq!(sys.query.model, "gpt-5.5-mini");
        assert_eq!(sys.query.effort, "low");
    }

    /// Spec: Codex Provider Config Schema — azure profile loads Responses API fields.
    #[test]
    fn codex_azure_profile_loads_responses_fields() {
        let yaml = codex_yaml(
            "      active: azure\n      azure:\n        base_url: https://x.cognitiveservices.azure.com/openai\n        api_version: 2025-04-01-preview\n        keyring_service: codebus-azure\n        goal:   { model: gpt-5.4, effort: high }\n        query:  { model: gpt-5.4, effort: low }\n        fix:    { model: gpt-5.4, effort: medium }\n        verify: { model: gpt-5.4, effort: high }\n",
        );
        let cfg = parse_codex_yaml(&yaml).unwrap().expect("codex config present");
        assert_eq!(cfg.active, ActiveProfile::Azure);
        let az = cfg.azure.expect("azure populated");
        assert_eq!(az.base_url, "https://x.cognitiveservices.azure.com/openai");
        assert_eq!(az.api_version, "2025-04-01-preview");
        assert_eq!(az.keyring_service, "codebus-azure");
        assert_eq!(az.goal.model, "gpt-5.4");
    }

    /// Spec: Codex Provider Config Schema — active profile missing a verb fails.
    #[test]
    fn codex_active_system_missing_verify_rejected() {
        let yaml = codex_yaml(
            "      active: system\n      system:\n        goal:  { model: gpt-5.5, effort: high }\n        query: { model: gpt-5.5, effort: low }\n        fix:   { model: gpt-5.5, effort: medium }\n",
        );
        let err = parse_codex_yaml(&yaml).expect_err("missing verify must reject");
        assert!(format!("{err}").contains("verify"), "got: {err}");
    }

    /// Spec: Codex Provider Config Schema — azure keyring_service defaults to
    /// codebus-codex-azure (distinct from claude's default so keys don't collide).
    #[test]
    fn codex_azure_keyring_service_defaults() {
        let yaml = codex_yaml(
            "      active: azure\n      azure:\n        base_url: https://x.cognitiveservices.azure.com/openai\n        api_version: 2025-04-01-preview\n        goal:   { model: gpt-5.4, effort: high }\n        query:  { model: gpt-5.4, effort: low }\n        fix:    { model: gpt-5.4, effort: medium }\n        verify: { model: gpt-5.4, effort: high }\n",
        );
        let cfg = parse_codex_yaml(&yaml).unwrap().expect("codex config present");
        assert_eq!(cfg.azure.unwrap().keyring_service, "codebus-codex-azure");
    }
}
