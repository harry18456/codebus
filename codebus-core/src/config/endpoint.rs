//! Endpoint profile configuration for the Claude CLI sub-process.
//!
//! `claude_code` in `~/.codebus/config.yaml` has the shape:
//!
//! ```yaml
//! claude_code:
//!   active: system   # or `azure`
//!   system:
//!     goal:  { model: opus-4-6, effort: high }
//!     query: { model: haiku-4-5,  effort: low }
//!     fix:   { model: sonnet-4-6, effort: medium }
//!   azure:
//!     base_url: https://<resource>.cognitiveservices.azure.com/anthropic
//!     keyring_service: codebus-azure
//!     goal:  { model: <deployment-name>, effort: high }
//!     query: { model: <deployment-name>, effort: low }
//!     fix:   { model: <deployment-name>, effort: medium }
//! ```
//!
//! Rules:
//! - The profile referenced by `active` MUST be fully populated.
//! - The non-active profile MAY be absent or partial (treated as cold
//!   storage; codebus does not validate it).
//! - `system` profile `model` values are free strings translated to the CLI
//!   `--model` flag via [`system_model_to_cli_flag`];
//!   invalid values reject the load.
//! - `azure` profile `model` values are arbitrary non-empty strings (the
//!   Azure deployment name) — codebus passes them verbatim to the `--model`
//!   flag, never translating system-style aliases like `opus-4-6` to brand
//!   names.
//!
//! Legacy schema detection (`claude_code.goal` / `query` / `fix` at the
//! top level, without `system` / `azure` wrappers) lives in
//! [`super::claude_code::load_claude_code_config`].

use serde::Deserialize;

use super::ConfigLoadError;

/// Which endpoint profile is currently active. Drives both model resolution
/// and env injection at spawn time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveProfile {
    System,
    Azure,
}

impl Default for ActiveProfile {
    fn default() -> Self {
        ActiveProfile::System
    }
}

/// Translate a system-profile `model` alias to the Claude CLI `--model` flag
/// value. The rule is uniform for known aliases (`opus-4-7`, …) AND any
/// future Claude model: ensure the `claude-` prefix. A value already carrying
/// the prefix (or a custom full id) passes through verbatim, so a
/// newly-released Claude model works without a codebus code change. The empty
/// string maps to an empty flag (the caller decides whether that is valid).
pub fn system_model_to_cli_flag(model: &str) -> String {
    let m = model.trim();
    if m.is_empty() || m.starts_with("claude-") {
        m.to_string()
    } else {
        format!("claude-{m}")
    }
}

/// One verb's settings under the system profile. `model` is a free string —
/// a short alias (`opus-4-7`, translated via [`system_model_to_cli_flag`]) or
/// a full `claude-…` id — so newly-released Claude models need no code change;
/// `effort` is an arbitrary string (the Claude CLI validates its values).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemVerbConfig {
    pub model: String,
    pub effort: String,
}

/// Four verb settings (goal / query / fix / verify) under the system profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemProfile {
    pub goal: SystemVerbConfig,
    pub query: SystemVerbConfig,
    pub fix: SystemVerbConfig,
    /// verify-stage-independent-model: dedicated sub-block for the
    /// content-verify spawn of quiz / goal verbs. Required in active
    /// profile (see `Endpoint Profile Schema` spec). Default value is
    /// `opus-4-6 / high` — strongest reasoning model + highest effort —
    /// reflecting the "expensive verification" design intent.
    pub verify: SystemVerbConfig,
}

impl Default for SystemProfile {
    fn default() -> Self {
        Self {
            goal: SystemVerbConfig {
                model: "opus-4-6".into(),
                effort: "high".into(),
            },
            query: SystemVerbConfig {
                model: "haiku-4-5".into(),
                effort: "low".into(),
            },
            fix: SystemVerbConfig {
                model: "sonnet-4-6".into(),
                effort: "medium".into(),
            },
            verify: SystemVerbConfig {
                model: "opus-4-6".into(),
                effort: "high".into(),
            },
        }
    }
}

/// One verb's settings under the azure profile. `model` is an arbitrary
/// non-empty string (the Azure deployment name) — codebus does NOT
/// validate, translate, or rewrite it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AzureVerbConfig {
    pub model: String,
    pub effort: String,
}

/// Azure endpoint profile. `base_url` and `keyring_service` are required;
/// each verb is a freeform deployment-name + effort pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureProfile {
    pub base_url: String,
    pub keyring_service: String,
    pub goal: AzureVerbConfig,
    pub query: AzureVerbConfig,
    pub fix: AzureVerbConfig,
    /// verify-stage-independent-model: dedicated sub-block for the
    /// content-verify spawn (mirrors `SystemProfile::verify`).
    pub verify: AzureVerbConfig,
}

/// The full `claude_code` config block, parsed and validated. The active
/// profile is guaranteed populated; the non-active profile is `None` (cold
/// storage that did not appear in the file) or `Some(...)` (preserved
/// verbatim but not validated by codebus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCodeConfig {
    pub active: ActiveProfile,
    pub system: SystemProfile,
    pub azure: Option<AzureProfile>,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            active: ActiveProfile::System,
            system: SystemProfile::default(),
            azure: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Raw deserialization scaffolding
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawConfigFile {
    #[serde(default)]
    pub agent: Option<RawAgent>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawAgent {
    #[serde(default)]
    pub active_provider: Option<String>,
    #[serde(default)]
    pub providers: Option<RawProviders>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawProviders {
    #[serde(default)]
    pub claude: Option<RawClaudeProvider>,
    #[serde(default)]
    pub codex: Option<super::codex::RawCodexProvider>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct RawClaudeProvider {
    #[serde(default)]
    pub active: Option<ActiveProfile>,
    #[serde(default)]
    pub system: Option<RawSystemProfile>,
    #[serde(default)]
    pub azure: Option<RawAzureProfile>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawSystemProfile {
    #[serde(default)]
    pub goal: Option<SystemVerbConfig>,
    #[serde(default)]
    pub query: Option<SystemVerbConfig>,
    #[serde(default)]
    pub fix: Option<SystemVerbConfig>,
    #[serde(default)]
    pub verify: Option<SystemVerbConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RawAzureProfile {
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub keyring_service: Option<String>,
    #[serde(default)]
    pub goal: Option<AzureVerbConfig>,
    #[serde(default)]
    pub query: Option<AzureVerbConfig>,
    #[serde(default)]
    pub fix: Option<AzureVerbConfig>,
    #[serde(default)]
    pub verify: Option<AzureVerbConfig>,
}

// ---------------------------------------------------------------------------
// Parsing + validation
// ---------------------------------------------------------------------------

/// Outcome of `parse_claude_code_yaml` — either a normalised config or a
/// signal that the file uses the legacy schema (top-level verb keys under
/// `claude_code` without profile wrappers). The legacy variant carries
/// nothing; the caller is responsible for warning + falling back to
/// `ClaudeCodeConfig::default()`.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    New(ClaudeCodeConfig),
    Missing,
}

/// Parse the yaml text of `~/.codebus/config.yaml`. Returns `Missing` when
/// the `agent` block (or its `providers.claude` sub-block) is absent, and
/// `New(...)` otherwise. Validation enforces that the active endpoint profile
/// of the claude provider is fully populated; the non-active profile is
/// preserved verbatim but not validated.
///
/// `agent.active_provider` is accepted only as `claude` (or absent → claude)
/// within this capability's scope; any other value is rejected.
pub fn parse_claude_code_yaml(yaml: &str) -> Result<ParseOutcome, ConfigLoadError> {
    let raw: RawConfigFile = serde_yaml::from_str(yaml).map_err(ConfigLoadError::YamlParse)?;
    let Some(agent) = raw.agent else {
        return Ok(ParseOutcome::Missing);
    };

    if let Some(provider) = agent.active_provider.as_deref()
        && provider != "claude"
        && provider != "codex"
    {
        return Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(format!(
            "agent.active_provider: only `claude` or `codex` is supported (got `{provider}`)"
        ))));
    }

    let Some(claude) = agent.providers.and_then(|p| p.claude) else {
        return Ok(ParseOutcome::Missing);
    };

    let active = claude.active.unwrap_or(ActiveProfile::System);
    let system = validate_system_profile(active, claude.system)?;
    let azure = validate_azure_profile(active, claude.azure)?;
    Ok(ParseOutcome::New(ClaudeCodeConfig {
        active,
        system,
        azure,
    }))
}

/// Parse the `agent.providers.codex` block of `~/.codebus/config.yaml`.
/// Returns `Ok(Some(config))` when the codex provider block is present and
/// valid, `Ok(None)` when absent, and `Err` on a structurally invalid or
/// under-populated active profile. (Provider routing — which provider is
/// active — is the dispatch layer's concern; this just parses codex's block.)
/// Read `agent.active_provider` from the config yaml, defaulting to `claude`
/// when the `agent` block or the key is absent. Used by the dispatch layer to
/// pick which provider config to load. Does NOT validate the value — the
/// per-provider parse path rejects unsupported names.
pub fn read_active_provider(yaml: &str) -> Result<String, ConfigLoadError> {
    let raw: RawConfigFile = serde_yaml::from_str(yaml).map_err(ConfigLoadError::YamlParse)?;
    Ok(raw
        .agent
        .and_then(|a| a.active_provider)
        .unwrap_or_else(|| "claude".to_string()))
}

pub fn parse_codex_yaml(
    yaml: &str,
) -> Result<Option<super::codex::CodexConfig>, ConfigLoadError> {
    let raw: RawConfigFile = serde_yaml::from_str(yaml).map_err(ConfigLoadError::YamlParse)?;
    let Some(agent) = raw.agent else {
        return Ok(None);
    };
    let Some(codex) = agent.providers.and_then(|p| p.codex) else {
        return Ok(None);
    };
    Ok(Some(super::codex::validate_codex_provider(codex)?))
}

fn validate_system_profile(
    active: ActiveProfile,
    raw: Option<RawSystemProfile>,
) -> Result<SystemProfile, ConfigLoadError> {
    match (active, raw) {
        (ActiveProfile::System, None) => {
            Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(
                "claude_code.active: system but `claude_code.system` block is missing",
            )))
        }
        (ActiveProfile::System, Some(raw)) => {
            let goal = raw.goal.ok_or_else(|| {
                ConfigLoadError::YamlParse(serde_yaml::Error::custom(
                    "claude_code.system.goal: required when active=system",
                ))
            })?;
            let query = raw.query.ok_or_else(|| {
                ConfigLoadError::YamlParse(serde_yaml::Error::custom(
                    "claude_code.system.query: required when active=system",
                ))
            })?;
            let fix = raw.fix.ok_or_else(|| {
                ConfigLoadError::YamlParse(serde_yaml::Error::custom(
                    "claude_code.system.fix: required when active=system",
                ))
            })?;
            let verify = raw.verify.ok_or_else(|| {
                ConfigLoadError::YamlParse(serde_yaml::Error::custom(
                    "claude_code.system.verify: required when active=system",
                ))
            })?;
            Ok(SystemProfile {
                goal,
                query,
                fix,
                verify,
            })
        }
        (ActiveProfile::Azure, None) => Ok(SystemProfile::default()),
        (ActiveProfile::Azure, Some(raw)) => Ok(SystemProfile {
            goal: raw.goal.unwrap_or(SystemProfile::default().goal),
            query: raw.query.unwrap_or(SystemProfile::default().query),
            fix: raw.fix.unwrap_or(SystemProfile::default().fix),
            verify: raw.verify.unwrap_or(SystemProfile::default().verify),
        }),
    }
}

fn validate_azure_profile(
    active: ActiveProfile,
    raw: Option<RawAzureProfile>,
) -> Result<Option<AzureProfile>, ConfigLoadError> {
    match (active, raw) {
        (ActiveProfile::Azure, None) => Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(
            "claude_code.active: azure but `claude_code.azure` block is missing",
        ))),
        (ActiveProfile::Azure, Some(raw)) => {
            let base_url = require_non_empty(raw.base_url, "claude_code.azure.base_url")?;
            let keyring_service =
                require_non_empty(raw.keyring_service, "claude_code.azure.keyring_service")?;
            let goal = require_azure_verb(raw.goal, "claude_code.azure.goal")?;
            let query = require_azure_verb(raw.query, "claude_code.azure.query")?;
            let fix = require_azure_verb(raw.fix, "claude_code.azure.fix")?;
            let verify = require_azure_verb(raw.verify, "claude_code.azure.verify")?;
            Ok(Some(AzureProfile {
                base_url,
                keyring_service,
                goal,
                query,
                fix,
                verify,
            }))
        }
        (ActiveProfile::System, None) => Ok(None),
        // Non-active azure block: keep as cold storage. base_url /
        // keyring_service may be missing — we deliberately do NOT
        // construct an `AzureProfile` (which would require them) and
        // instead return None so the caller can ignore it. All four
        // verb sub-blocks (incl. verify) must be present to qualify
        // for cold-storage preservation; partial verb blocks are
        // dropped silently.
        (ActiveProfile::System, Some(raw)) => {
            match (
                raw.base_url,
                raw.keyring_service,
                raw.goal,
                raw.query,
                raw.fix,
                raw.verify,
            ) {
                (
                    Some(base_url),
                    Some(keyring_service),
                    Some(goal),
                    Some(query),
                    Some(fix),
                    Some(verify),
                ) if !base_url.is_empty() && !keyring_service.is_empty() => {
                    Ok(Some(AzureProfile {
                        base_url,
                        keyring_service,
                        goal,
                        query,
                        fix,
                        verify,
                    }))
                }
                _ => Ok(None),
            }
        }
    }
}

fn require_non_empty(value: Option<String>, field: &str) -> Result<String, ConfigLoadError> {
    match value {
        Some(s) if !s.is_empty() => Ok(s),
        _ => Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(
            format!("{field}: required and non-empty"),
        ))),
    }
}

fn require_azure_verb(
    value: Option<AzureVerbConfig>,
    field: &str,
) -> Result<AzureVerbConfig, ConfigLoadError> {
    match value {
        Some(v) if !v.model.is_empty() => Ok(v),
        Some(_) => Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(
            format!("{field}.model: required and non-empty"),
        ))),
        None => Err(ConfigLoadError::YamlParse(serde_yaml::Error::custom(
            format!("{field}: required when active=azure"),
        ))),
    }
}

// `serde_yaml::Error::custom` needs the `Error` trait in scope.
use serde::de::Error as _;

#[cfg(test)]
mod tests {
    use super::*;

    /// Wrap a legacy-style `claude_code:\n  <body>` test string into the
    /// unified `agent.providers.claude` envelope, re-nesting `<body>` one
    /// level deeper. Lets the existing inner-schema test strings stay
    /// verbatim while exercising the new top-level shape.
    fn agent_yaml(claude_code_yaml: &str) -> String {
        let body = claude_code_yaml
            .strip_prefix("claude_code:\n")
            .expect("test yaml must start with `claude_code:\\n`");
        let reindented: String = body
            .lines()
            .map(|l| {
                if l.trim().is_empty() {
                    String::new()
                } else {
                    format!("    {l}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("agent:\n  active_provider: claude\n  providers:\n    claude:\n{reindented}\n")
    }

    // === System Profile Model (free string + alias translation) ===

    /// Known aliases map to `claude-<alias>` for the CLI `--model` flag.
    #[test]
    fn system_model_to_cli_flag_known_aliases() {
        assert_eq!(system_model_to_cli_flag("opus-4-7"), "claude-opus-4-7");
        assert_eq!(system_model_to_cli_flag("opus-4-6"), "claude-opus-4-6");
        assert_eq!(system_model_to_cli_flag("haiku-4-5"), "claude-haiku-4-5");
        assert_eq!(system_model_to_cli_flag("sonnet-4-6"), "claude-sonnet-4-6");
    }

    /// A future Claude model needs no code change: a new alias gets the
    /// `claude-` prefix; a full `claude-…` id (or empty) passes through.
    #[test]
    fn system_model_to_cli_flag_future_and_passthrough() {
        assert_eq!(system_model_to_cli_flag("opus-4-8"), "claude-opus-4-8");
        assert_eq!(system_model_to_cli_flag("claude-opus-4-7"), "claude-opus-4-7");
        assert_eq!(system_model_to_cli_flag("  haiku-4-5  "), "claude-haiku-4-5");
        assert_eq!(system_model_to_cli_flag(""), "");
    }

    /// System `model` is now a free string: an arbitrary / newly-released
    /// alias loads without rejection (the closed-enum gate is gone) and is
    /// preserved verbatim in the config.
    #[test]
    fn system_model_accepts_arbitrary_string() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-8,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).expect("free-string model loads");
        match outcome {
            ParseOutcome::New(cfg) => assert_eq!(cfg.system.goal.model, "opus-4-8"),
            other => panic!("expected New, got {other:?}"),
        }
    }

    // === Active=System happy paths ===

    /// Spec: Endpoint Profile Schema — full system profile loads.
    #[test]
    fn system_profile_loads_with_all_three_verbs() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(cfg.active, ActiveProfile::System);
        assert_eq!(cfg.system.goal.model, "opus-4-6");
        assert_eq!(cfg.system.goal.effort, "high");
        assert_eq!(cfg.system.query.model, "haiku-4-5");
        assert_eq!(cfg.system.fix.model, "sonnet-4-6");
        assert!(cfg.azure.is_none());
    }

    /// Spec: Endpoint Profile Schema — active=system but system block missing verb rejected.
    #[test]
    fn active_system_with_missing_verb_rejected() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal: { model: opus-4-6, effort: high }\n    fix:  { model: sonnet-4-6, effort: medium }\n";
        let err = parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("missing query must be rejected");
        let msg = format!("{err}");
        assert!(msg.contains("query"), "got: {msg}");
    }

    /// Spec: Endpoint Profile Schema — `agent` block absent → Missing.
    #[test]
    fn missing_agent_section_returns_missing() {
        let outcome = parse_claude_code_yaml("pii:\n  scanner: regex_basic\n").unwrap();
        assert_eq!(outcome, ParseOutcome::Missing);
    }

    /// Legacy top-level `claude_code` schema is treated as an absent `agent`
    /// block → Missing (no warning, no special handling).
    #[test]
    fn legacy_claude_code_top_level_returns_missing() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal: { model: opus-4-6, effort: high }\n";
        let outcome = parse_claude_code_yaml(yaml).unwrap();
        assert_eq!(outcome, ParseOutcome::Missing);
    }

    /// `agent.providers.claude` absent (agent present but empty) → Missing.
    #[test]
    fn agent_without_claude_provider_returns_missing() {
        let outcome = parse_claude_code_yaml("agent:\n  active_provider: claude\n").unwrap();
        assert_eq!(outcome, ParseOutcome::Missing);
    }

    /// `agent.active_provider` other than `claude`/`codex` is rejected.
    /// Spec: Endpoint Profile Schema — Unsupported provider name rejected.
    #[test]
    fn unsupported_active_provider_rejected() {
        let yaml = "agent:\n  active_provider: gemini\n  providers:\n    claude:\n      active: system\n";
        let err = parse_claude_code_yaml(yaml).expect_err("unsupported provider must reject");
        assert!(format!("{err}").contains("active_provider"));
    }

    /// Spec: Endpoint Profile Schema — Codex active_provider is accepted (not
    /// rejected on the basis of the provider name). The claude loader returns
    /// Missing here (no claude block); the dispatch layer routes to the codex
    /// loader instead.
    #[test]
    fn codex_active_provider_not_rejected() {
        let yaml = "agent:\n  active_provider: codex\n  providers:\n    codex:\n      active: system\n";
        let outcome = parse_claude_code_yaml(yaml).expect("codex provider must not be rejected");
        assert_eq!(outcome, ParseOutcome::Missing);
    }

    // === Active=Azure happy + reject paths ===

    /// Spec: Endpoint Profile Schema — full azure profile loads.
    #[test]
    fn azure_profile_loads_with_required_fields() {
        let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://example.cognitiveservices.azure.com/anthropic\n    keyring_service: codebus-azure\n    goal:   { model: dep-opus,   effort: high   }\n    query:  { model: dep-haiku,  effort: low    }\n    fix:    { model: dep-sonnet, effort: medium }\n    verify: { model: dep-opus,   effort: high   }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(cfg.active, ActiveProfile::Azure);
        let az = cfg.azure.expect("azure populated when active=azure");
        assert_eq!(
            az.base_url,
            "https://example.cognitiveservices.azure.com/anthropic"
        );
        assert_eq!(az.keyring_service, "codebus-azure");
        assert_eq!(az.goal.model, "dep-opus");
        assert_eq!(az.fix.effort, "medium");
    }

    /// Spec: Endpoint Profile Schema — active=azure but base_url missing rejected.
    #[test]
    fn active_azure_with_missing_base_url_rejected() {
        let yaml = "claude_code:\n  active: azure\n  azure:\n    keyring_service: codebus-azure\n    goal:  { model: dep-opus, effort: high }\n    query: { model: dep-haiku, effort: low }\n    fix:   { model: dep-sonnet, effort: medium }\n";
        let err = parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("missing base_url must be rejected");
        let msg = format!("{err}");
        assert!(msg.contains("base_url"), "got: {msg}");
    }

    /// Spec: Endpoint Profile Schema — active=azure but azure block missing entirely.
    #[test]
    fn active_azure_with_no_azure_block_rejected() {
        let yaml = "claude_code:\n  active: azure\n  system:\n    goal: { model: opus-4-6, effort: high }\n    query: { model: haiku-4-5, effort: low }\n    fix:   { model: sonnet-4-6, effort: medium }\n";
        let err = parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("missing azure block must be rejected");
        let msg = format!("{err}");
        assert!(msg.contains("azure"), "got: {msg}");
    }

    // === Non-active profile may be partial ===

    /// Spec: Endpoint Profile Schema — non-active profile may be absent.
    #[test]
    fn non_active_profile_may_be_absent() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n";
        // No azure block at all → still valid.
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        assert!(matches!(outcome, ParseOutcome::New(_)));
    }

    /// Spec: Endpoint Profile Schema — non-active profile may be partial.
    #[test]
    fn non_active_azure_partial_is_silently_dropped() {
        // active=system; azure block present but missing keyring_service.
        // codebus does not validate; it just drops the partial profile to
        // None so the caller never sees half-populated state.
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n  azure:\n    base_url: https://e.example.com/anthropic\n    goal: { model: dep-opus, effort: high }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        // Non-active azure dropped to None since incomplete.
        assert!(cfg.azure.is_none());
    }

    // === verify-stage-independent-model task 1.1 (RED) ===
    //
    // `Endpoint Profile Schema` requirement now lists `verify` as a fourth
    // required verb sub-block (alongside goal / query / fix). Active-profile
    // yaml without it MUST be rejected; non-active profile yaml MAY omit it
    // (cold storage tolerance preserved).

    /// Spec: Endpoint Profile Schema — active=system but system.verify missing rejected.
    #[test]
    fn active_system_with_missing_verify_rejected() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:  { model: opus-4-6, effort: high }\n    query: { model: haiku-4-5, effort: low }\n    fix:   { model: sonnet-4-6, effort: medium }\n";
        let err =
            parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("missing system.verify must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("verify"),
            "error must mention the verify field path; got: {msg}"
        );
    }

    /// Spec: Endpoint Profile Schema — active=azure but azure.verify missing rejected.
    #[test]
    fn active_azure_with_missing_verify_rejected() {
        let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://e.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:  { model: dep-opus,   effort: high   }\n    query: { model: dep-haiku,  effort: low    }\n    fix:   { model: dep-sonnet, effort: medium }\n";
        let err = parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("missing azure.verify must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("verify"),
            "error must mention the verify field path; got: {msg}"
        );
    }

    /// Spec: Endpoint Profile Schema — non-active profile may omit verify
    /// (cold storage tolerance preserved). This is a regression guardrail:
    /// the strict verify requirement applies ONLY to the active profile.
    #[test]
    fn non_active_profile_may_omit_verify() {
        // active=system with full system block (incl. verify); azure
        // block partial (no verify) — must parse OK.
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6, effort: high }\n    query:  { model: haiku-4-5, effort: low }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6, effort: high }\n  azure:\n    base_url: https://e.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:  { model: dep-opus,   effort: high   }\n    query: { model: dep-haiku,  effort: low    }\n    fix:   { model: dep-sonnet, effort: medium }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        assert!(
            matches!(outcome, ParseOutcome::New(_)),
            "non-active azure missing verify must NOT block load"
        );
    }

    /// Spec: Endpoint Profile Schema — active=system with all four verbs populated (incl. verify).
    #[test]
    fn system_profile_loads_with_all_four_verbs_including_verify() {
        let yaml = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6, effort: high }\n    query:  { model: haiku-4-5, effort: low }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6, effort: high }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(cfg.active, ActiveProfile::System);
        assert_eq!(cfg.system.verify.model, "opus-4-6");
        assert_eq!(cfg.system.verify.effort, "high");
    }

    /// Spec: Endpoint Profile Schema — active=azure with all four verbs populated (incl. verify).
    #[test]
    fn azure_profile_loads_with_all_four_verbs_including_verify() {
        let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://e.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:   { model: dep-opus,   effort: high   }\n    query:  { model: dep-haiku,  effort: low    }\n    fix:    { model: dep-sonnet, effort: medium }\n    verify: { model: dep-opus,   effort: high   }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        let az = cfg.azure.expect("azure populated when active=azure");
        assert_eq!(az.verify.model, "dep-opus");
        assert_eq!(az.verify.effort, "high");
    }

    // === Active selector default ===

    /// Spec: Endpoint Profile Schema — `active` defaults to system when absent.
    #[test]
    fn active_defaults_to_system_when_unspecified() {
        let yaml = "claude_code:\n  system:\n    goal:   { model: opus-4-6,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n";
        let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
        let cfg = match outcome {
            ParseOutcome::New(cfg) => cfg,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(cfg.active, ActiveProfile::System);
    }

    // === Defaults ===

    /// Spec: Endpoint Profile Schema — built-in defaults.
    #[test]
    fn default_config_uses_v2_verified_models() {
        let cfg = ClaudeCodeConfig::default();
        assert_eq!(cfg.active, ActiveProfile::System);
        assert_eq!(cfg.system.goal.model, "opus-4-6");
        assert_eq!(cfg.system.goal.effort, "high");
        assert_eq!(cfg.system.query.model, "haiku-4-5");
        assert_eq!(cfg.system.query.effort, "low");
        assert_eq!(cfg.system.fix.model, "sonnet-4-6");
        assert_eq!(cfg.system.fix.effort, "medium");
        assert!(cfg.azure.is_none());
    }

    // === Conflict: legacy verbs + profile blocks ===

    // === Azure passthrough (spec: Azure Profile Model String Passthrough) ===

    mod azure_passthrough {
        use super::*;

        /// Spec: Azure mode does NOT translate the system-style alias
        /// `opus-4-6` to `claude-opus-4-6`. The literal string is preserved
        /// verbatim — if a deployment of that name doesn't exist on Azure
        /// the user gets a 404 at runtime, which is the correct signal.
        #[test]
        fn system_alias_literal_is_not_translated_in_azure() {
            let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:   { model: opus-4-6, effort: high   }\n    query:  { model: opus-4-6, effort: low    }\n    fix:    { model: opus-4-6, effort: medium }\n    verify: { model: opus-4-6, effort: high   }\n";
            let outcome = parse_claude_code_yaml(&agent_yaml(yaml)).unwrap();
            let cfg = match outcome {
                ParseOutcome::New(c) => c,
                other => panic!("expected New, got {other:?}"),
            };
            let az = cfg.azure.expect("azure populated");
            assert_eq!(az.goal.model, "opus-4-6");
            assert_eq!(az.query.model, "opus-4-6");
            assert_eq!(az.fix.model, "opus-4-6");
        }

        /// Spec: Azure deployment names (with embedded version + suffix) are
        /// preserved verbatim across the round-trip.
        #[test]
        fn long_azure_deployment_name_preserved_verbatim() {
            let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:   { model: claude-opus-4-6-2026V2,   effort: high   }\n    query:  { model: claude-haiku-4-5-2026V2,  effort: low    }\n    fix:    { model: claude-sonnet-4-6-2026V2, effort: medium }\n    verify: { model: claude-opus-4-6-2026V2,   effort: high   }\n";
            let cfg = match parse_claude_code_yaml(&agent_yaml(yaml)).unwrap() {
                ParseOutcome::New(c) => c,
                other => panic!("expected New, got {other:?}"),
            };
            let az = cfg.azure.unwrap();
            assert_eq!(az.goal.model, "claude-opus-4-6-2026V2");
            assert_eq!(az.query.model, "claude-haiku-4-5-2026V2");
            assert_eq!(az.fix.model, "claude-sonnet-4-6-2026V2");
        }

        /// Spec: Azure mode rejects an empty model string (the spec
        /// "arbitrary NON-EMPTY string" rule).
        #[test]
        fn empty_azure_model_rejected() {
            let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:  { model: '', effort: high }\n    query: { model: dep-haiku, effort: low }\n    fix:   { model: dep-sonnet, effort: medium }\n";
            let err = parse_claude_code_yaml(&agent_yaml(yaml)).expect_err("empty model rejected");
            let msg = format!("{err}");
            assert!(msg.contains("model"), "got: {msg}");
        }

        /// Spec: Azure mode preserves casing exactly (no normalisation).
        #[test]
        fn azure_model_case_preserved() {
            let yaml = "claude_code:\n  active: azure\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:   { model: MyDeployment-Opus-V2, effort: high   }\n    query:  { model: dep-haiku,            effort: low    }\n    fix:    { model: dep-sonnet,           effort: medium }\n    verify: { model: MyDeployment-Opus-V2, effort: high   }\n";
            let cfg = match parse_claude_code_yaml(&agent_yaml(yaml)).unwrap() {
                ParseOutcome::New(c) => c,
                other => panic!("expected New, got {other:?}"),
            };
            assert_eq!(cfg.azure.unwrap().goal.model, "MyDeployment-Opus-V2");
        }
    }

}
