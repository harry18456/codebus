//! Global config shape — `~/.codebus/config.yaml` deserialized.
//!
//! All fields are `Option<…>` and `#[serde(default)]` so a partial or empty
//! config parses cleanly. Plugin sections each map to one of the five
//! plugin domain configs in [`crate::llm`], [`crate::pii`], [`crate::wiki::lint`],
//! [`crate::render`], [`crate::log`].
//!
//! Forward-compat: unknown top-level keys are silently ignored (serde
//! default). Unknown sub-fields within a known section are silently
//! ignored. Unknown discriminator values (e.g. `provider: gibberish`) are
//! reported as a warning by [`super::loader::load_config`] and the section
//! is treated as unset.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmojiMode {
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Resolved provider kind. `None` when the YAML had no `provider` key
    /// or the value was unrecognized (warned + dropped by the loader).
    #[serde(skip)]
    pub provider: Option<crate::llm::ProviderKind>,
    pub binary_path: Option<String>,
    pub timeout_secs: Option<u64>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PiiConfig {
    #[serde(skip)]
    pub scanner: Option<crate::pii::ScannerKind>,
    #[serde(skip)]
    pub on_hit: Option<crate::pii::OnHit>,
    pub patterns_extra: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintConfig {
    pub disabled_rules: Vec<String>,
    pub custom_rules_dir: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderConfig {
    #[serde(skip)]
    pub format: Option<crate::render::RendererKind>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(skip)]
    pub sink: Option<crate::log::SinkKind>,
    pub retention_days: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub emoji: Option<EmojiMode>,
    pub llm: Option<LlmConfig>,
    pub pii: Option<PiiConfig>,
    pub lint: Option<LintConfig>,
    pub render: Option<RenderConfig>,
    pub log: Option<LogConfig>,
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
}
