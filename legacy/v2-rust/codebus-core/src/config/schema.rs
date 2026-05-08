//! Global config shape — `~/.codebus/config.yaml` deserialized.
//!
//! Plugin sections (`llm`, `pii`, `render`, `log`) hold the factory-domain
//! tagged enum directly (e.g. [`crate::llm::ProviderConfig`]). The
//! discriminator field (`provider` / `scanner` / `format` / `sink`) and the
//! variant-specific sub-fields all live in the same enum value — there are
//! no intermediate flat structs.
//!
//! Forward-compat: unknown top-level keys are silently ignored. Unknown
//! sub-fields within a known section are silently ignored. Unknown
//! discriminator values (e.g. `provider: gibberish`) are reported as a
//! warning by [`super::loader::load_config`] and the section is treated as
//! unset.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmojiMode {
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintConfig {
    pub disabled_rules: Vec<String>,
    pub custom_rules_dir: Option<String>,
    #[serde(default)]
    pub auto_fix: AutoFixConfig,
}

/// Lint auto-fix policy. `enabled` flips the goal-flow auto-fix step;
/// `max_iterations` caps the lint→fix→re-lint loop. Defaults align with
/// the `lint-feedback-loop` design: agentic feel by default, hard upper
/// bound on token spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoFixConfig {
    pub enabled: bool,
    pub max_iterations: u32,
}

impl Default for AutoFixConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_iterations: 5,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub emoji: Option<EmojiMode>,
    pub llm: Option<crate::llm::ProviderConfig>,
    pub pii: Option<crate::pii::ScannerConfig>,
    pub lint: Option<LintConfig>,
    pub render: Option<crate::render::RendererConfig>,
    pub log: Option<crate::log::SinkConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_global_config_has_all_sections_unset() {
        let g = GlobalConfig::default();
        assert!(g.emoji.is_none());
        assert!(g.llm.is_none());
        assert!(g.pii.is_none());
        assert!(g.lint.is_none());
        assert!(g.render.is_none());
        assert!(g.log.is_none());
    }

    // === lint-feedback-loop: AutoFixConfig defaults ===

    #[test]
    fn auto_fix_config_default_is_enabled_with_max_iterations_five() {
        // Spec scenario: "Default config enables fix with max iterations five"
        let cfg = AutoFixConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_iterations, 5);
    }

    #[test]
    fn lint_config_default_includes_auto_fix_default() {
        // Lint config without explicit auto_fix still produces the agentic
        // default (enabled = true, max_iterations = 5).
        let cfg = LintConfig::default();
        assert!(cfg.auto_fix.enabled);
        assert_eq!(cfg.auto_fix.max_iterations, 5);
    }
}
