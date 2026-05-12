//! `claude_code.*` config loader.
//!
//! As of `claude-code-endpoint-profiles`, this module is a thin wrapper
//! around [`super::endpoint`]: it owns the file-reading + legacy-schema
//! detection, while the parse and validation logic lives in `endpoint.rs`.
//!
//! Legacy schema = a `claude_code` block containing top-level `goal` /
//! `query` / `fix` keys without `system` / `azure` profile wrappers. When
//! detected, codebus prints a stderr migration warning AND returns the
//! built-in default — the user's yaml file is NEVER modified by the
//! loader.

use std::fs;
use std::path::Path;

use super::ConfigLoadError;
use crate::config::endpoint::{
    ActiveProfile, ClaudeCodeConfig, ParseOutcome, parse_claude_code_yaml,
};
use super::keyring::{KeyringError, read_azure_key};
use crate::agent::EnvOverrides;
#[cfg(test)]
use crate::config::endpoint::SystemModel;

/// Which verb's settings to resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Goal,
    Query,
    Fix,
}

/// A verb's resolved settings: `model` is the value to pass to `claude
/// --model <X>` (already translated through [`SystemModel::to_cli_flag`]
/// when the system profile is active; verbatim deployment name when the
/// azure profile is active). `effort` is forwarded to `--effort <Y>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVerb {
    pub model: Option<String>,
    pub effort: Option<String>,
}

impl ClaudeCodeConfig {
    /// Resolve a verb's CLI flag values under the active profile.
    pub fn resolve(&self, verb: Verb) -> ResolvedVerb {
        match self.active {
            ActiveProfile::System => {
                let v = match verb {
                    Verb::Goal => &self.system.goal,
                    Verb::Query => &self.system.query,
                    Verb::Fix => &self.system.fix,
                };
                ResolvedVerb {
                    model: Some(v.model.to_cli_flag().to_string()),
                    effort: Some(v.effort.clone()),
                }
            }
            ActiveProfile::Azure => {
                let az = self
                    .azure
                    .as_ref()
                    .expect("azure profile populated when active=azure (validated on load)");
                let v = match verb {
                    Verb::Goal => &az.goal,
                    Verb::Query => &az.query,
                    Verb::Fix => &az.fix,
                };
                ResolvedVerb {
                    model: Some(v.model.clone()),
                    effort: Some(v.effort.clone()),
                }
            }
        }
    }
}

/// Build `EnvOverrides` from the active profile of `cfg`. System profile
/// returns an empty map (no env injection). Azure profile reads the API
/// key via the [`super::keyring::read_azure_key`] fallback chain
/// (keyring → `CODEBUS_AZURE_KEY` env → error) and returns the
/// three-key map produced by [`EnvOverrides::for_azure`]. Returns
/// [`KeyringError::EndpointKeyMissing`] when the azure profile is
/// active AND neither source has a key — the caller SHALL surface this
/// before spawning the agent child process.
pub fn build_env_overrides(cfg: &ClaudeCodeConfig) -> Result<EnvOverrides, KeyringError> {
    match cfg.active {
        ActiveProfile::System => Ok(EnvOverrides::for_system()),
        ActiveProfile::Azure => {
            let az = cfg
                .azure
                .as_ref()
                .expect("azure profile populated when active=azure (validated on load)");
            let key = read_azure_key(&az.keyring_service)?;
            Ok(EnvOverrides::for_azure(&az.base_url, &key))
        }
    }
}

/// Stderr migration warning text emitted when a legacy schema is detected.
/// Public for test cross-checking — production callers SHALL go through
/// [`load_claude_code_config`].
pub const LEGACY_MIGRATION_WARNING: &str = "\
warning: ~/.codebus/config.yaml uses the legacy `claude_code` schema (top-level \
`goal` / `query` / `fix` keys). Migrate to the profile schema:

claude_code:
  active: system
  system:
    goal:  { model: opus-4-6,   effort: high }
    query: { model: haiku-4-5,  effort: low }
    fix:   { model: sonnet-4-6, effort: medium }

codebus will continue with built-in defaults this run; your config file has \
NOT been modified.";

/// Load `claude_code.*` from `path`. Migration warnings (legacy schema)
/// are sent to stderr. See [`load_claude_code_config_into`] for the
/// testable variant that lets callers capture the warning text.
///
/// Contract:
///
/// - File missing → returns [`ClaudeCodeConfig::default()`].
/// - File exists, `claude_code` section absent → returns
///   [`ClaudeCodeConfig::default()`].
/// - File exists with the new profile schema → parsed + validated config.
/// - File exists with the legacy schema → stderr migration warning, then
///   returns [`ClaudeCodeConfig::default()`]; the on-disk file is NOT
///   modified.
/// - File exists but yaml is structurally invalid OR the new schema has
///   an invalid required field → returns [`ConfigLoadError::YamlParse`].
pub fn load_claude_code_config(
    path: &Path,
) -> Result<ClaudeCodeConfig, ConfigLoadError> {
    load_claude_code_config_into(path, &mut std::io::stderr())
}

/// Like [`load_claude_code_config`] but routes the legacy-schema migration
/// warning to `warn_sink` instead of stderr. Production callers use the
/// no-sink wrapper; tests use this variant with a `Vec<u8>` to assert
/// warning content. Sink write failures are silent — losing the warning
/// is preferable to surfacing a write error that masks the real signal.
pub fn load_claude_code_config_into(
    path: &Path,
    warn_sink: &mut dyn std::io::Write,
) -> Result<ClaudeCodeConfig, ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ClaudeCodeConfig::default());
        }
        Err(err) => return Err(ConfigLoadError::Io(err)),
    };
    match parse_claude_code_yaml(&body)? {
        ParseOutcome::New(cfg) => Ok(cfg),
        ParseOutcome::Missing => Ok(ClaudeCodeConfig::default()),
        ParseOutcome::Legacy => {
            let _ = writeln!(warn_sink, "{LEGACY_MIGRATION_WARNING}");
            Ok(ClaudeCodeConfig::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// File missing → defaults.
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_claude_code_config(&tmp.path().join("nope.yaml")).unwrap();
        assert_eq!(cfg, ClaudeCodeConfig::default());
    }

    /// `claude_code` section absent → defaults.
    #[test]
    fn default_when_section_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "lint:\n  fix:\n    enabled: true\n");
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg, ClaudeCodeConfig::default());
    }

    /// System profile loads + resolves to translated model.
    #[test]
    fn system_profile_resolves_translated_model() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  active: system\n  system:\n    goal:  { model: opus-4-6, effort: high }\n    query: { model: haiku-4-5,    effort: low }\n    fix:   { model: sonnet-4-6,   effort: medium }\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        let resolved = cfg.resolve(Verb::Goal);
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(resolved.effort.as_deref(), Some("high"));
        let q = cfg.resolve(Verb::Query);
        assert_eq!(q.model.as_deref(), Some("claude-haiku-4-5"));
        let f = cfg.resolve(Verb::Fix);
        assert_eq!(f.model.as_deref(), Some("claude-sonnet-4-6"));
    }

    /// Azure profile loads + resolves to verbatim deployment name.
    #[test]
    fn azure_profile_resolves_verbatim_deployment_name() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  active: azure\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: codebus-azure\n    goal:  { model: claude-opus-4-6-2026V2, effort: high }\n    query: { model: dep-haiku, effort: low }\n    fix:   { model: dep-sonnet, effort: medium }\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        let resolved = cfg.resolve(Verb::Goal);
        assert_eq!(resolved.model.as_deref(), Some("claude-opus-4-6-2026V2"));
    }

    /// Legacy schema → returns defaults, file unchanged.
    #[test]
    fn legacy_schema_returns_defaults_without_rewrite() {
        let tmp = TempDir::new().unwrap();
        let body = "claude_code:\n  goal:\n    model: opus\n    effort: high\n  query:\n    model: haiku\n    effort: low\n";
        let p = write_yaml(tmp.path(), body);
        let before = fs::read(&p).unwrap();

        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg, ClaudeCodeConfig::default());

        let after = fs::read(&p).unwrap();
        assert_eq!(before, after, "legacy detection must not rewrite the user file");
    }

    /// Invalid yaml → propagate the parse error so the caller can warn +
    /// fall back to default.
    #[test]
    fn invalid_yaml_returns_parse_error() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  active: bogus\n  system:\n    goal: { model: opus-4-6, effort: high }\n    query: { model: haiku-4-5, effort: low }\n    fix: { model: sonnet-4-6, effort: medium }\n",
        );
        let err = load_claude_code_config(&p).expect_err("bogus active rejected");
        let msg = format!("{err}");
        assert!(msg.contains("bogus") || msg.contains("variant"), "got: {msg}");
    }

    // === build_env_overrides ===

    /// Spec: system profile → empty env (no injection).
    #[test]
    fn build_env_overrides_for_system_returns_empty() {
        let cfg = ClaudeCodeConfig::default();
        let env = build_env_overrides(&cfg).expect("system path always succeeds");
        assert!(env.is_empty());
    }

    /// Spec: azure profile + env fallback set → three-key env. Uses the
    /// `CODEBUS_AZURE_KEY` env fallback so the test does not need to
    /// touch the OS keyring. Serialises with the keyring module's tests
    /// via `TEST_ENV_LOCK` so concurrent runs do not race on the env var.
    #[test]
    fn build_env_overrides_for_azure_with_env_fallback_returns_three_keys() {
        let _g = super::super::keyring::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var("CODEBUS_AZURE_KEY").ok();
        unsafe {
            std::env::set_var("CODEBUS_AZURE_KEY", "sk-from-fallback");
        }

        let cfg = ClaudeCodeConfig {
            active: ActiveProfile::Azure,
            system: crate::config::endpoint::SystemProfile::default(),
            azure: Some(crate::config::endpoint::AzureProfile {
                base_url: "https://x.example.com/anthropic".into(),
                keyring_service: "codebus-test-build-env".into(),
                goal: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-opus".into(),
                    effort: "high".into(),
                },
                query: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-haiku".into(),
                    effort: "low".into(),
                },
                fix: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-sonnet".into(),
                    effort: "medium".into(),
                },
            }),
        };
        let env = build_env_overrides(&cfg).expect("env fallback satisfies key");
        assert_eq!(env.len(), 3);
        assert_eq!(
            env.get("ANTHROPIC_BASE_URL"),
            Some("https://x.example.com/anthropic")
        );
        assert_eq!(env.get("ANTHROPIC_API_KEY"), Some("sk-from-fallback"));
        assert_eq!(env.get("CLAUDE_CODE_DISABLE_ADVISOR_TOOL"), Some("1"));

        unsafe {
            match prev {
                Some(v) => std::env::set_var("CODEBUS_AZURE_KEY", v),
                None => std::env::remove_var("CODEBUS_AZURE_KEY"),
            }
        }
    }

    /// Spec: azure profile + neither keyring nor env → `EndpointKeyMissing`.
    #[test]
    fn build_env_overrides_for_azure_without_key_returns_endpoint_key_missing() {
        let _g = super::super::keyring::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let prev = std::env::var("CODEBUS_AZURE_KEY").ok();
        unsafe {
            std::env::remove_var("CODEBUS_AZURE_KEY");
        }
        // Use a service name guaranteed not to exist in the test keyring.
        let cfg = ClaudeCodeConfig {
            active: ActiveProfile::Azure,
            system: crate::config::endpoint::SystemProfile::default(),
            azure: Some(crate::config::endpoint::AzureProfile {
                base_url: "https://x.example.com/anthropic".into(),
                keyring_service: "codebus-test-DEFINITELY-MISSING-Hbn21Z".into(),
                goal: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-opus".into(),
                    effort: "high".into(),
                },
                query: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-haiku".into(),
                    effort: "low".into(),
                },
                fix: crate::config::endpoint::AzureVerbConfig {
                    model: "dep-sonnet".into(),
                    effort: "medium".into(),
                },
            }),
        };
        let err = build_env_overrides(&cfg).expect_err("must fail with EndpointKeyMissing");
        assert!(
            matches!(err, KeyringError::EndpointKeyMissing { .. }),
            "expected EndpointKeyMissing, got {err:?}"
        );

        unsafe {
            match prev {
                Some(v) => std::env::set_var("CODEBUS_AZURE_KEY", v),
                None => std::env::remove_var("CODEBUS_AZURE_KEY"),
            }
        }
    }

    /// `SystemModel` enum string round-trips through full file load.
    #[test]
    fn arbitrary_system_model_alias_accepted() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "claude_code:\n  active: system\n  system:\n    goal:  { model: opus-4-7, effort: high }\n    query: { model: haiku-4-5,    effort: low }\n    fix:   { model: sonnet-4-6,   effort: medium }\n",
        );
        let cfg = load_claude_code_config(&p).unwrap();
        assert_eq!(cfg.system.goal.model, SystemModel::Opus4_7);
    }
}
