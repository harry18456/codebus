//! Renderer factory. Mirrors [`crate::llm::factory`] / [`crate::pii::factory`]
//! shape — explicit `match` over a tagged-enum [`RendererConfig`].
//!
//! Tagged-enum config pattern (see
//! `openspec/changes/config-tagged-enum-refactor/design.md`): each variant
//! carries its own renderer-specific fields, and `#[serde(tag = "format")]`
//! makes YAML write `format: terminal` + variant fields at the same level —
//! identical UX to the prior flat `RendererConfig` struct.

use crate::render::event_renderer::EventRenderer;
use crate::render::renderers::terminal::{RenderOptions, TerminalRenderer};
use serde::{Deserialize, Serialize};

/// Tagged-enum config: discriminator key `format` selects which renderer to
/// build. `Terminal` is the day-one only implementation; `JsonLines` and
/// `Tauri` are reserved for follow-up changes (see proposal §"EventRenderer
/// trait") and currently surface as [`RendererError::NotYetImplemented`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum RendererConfig {
    Terminal {
        #[serde(default)]
        options: RenderOptions,
    },
    JsonLines {},
    Tauri {},
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self::Terminal {
            options: RenderOptions::default(),
        }
    }
}

#[derive(Debug)]
pub enum RendererError {
    NotYetImplemented(&'static str),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RendererError::NotYetImplemented(name) => {
                write!(f, "renderer `{name}` is not yet implemented")
            }
        }
    }
}

impl std::error::Error for RendererError {}

/// Build a renderer from a [`RendererConfig`].
pub fn build_renderer(cfg: RendererConfig) -> Result<Box<dyn EventRenderer>, RendererError> {
    match cfg {
        RendererConfig::Terminal { options } => Ok(Box::new(TerminalRenderer::new(options))),
        RendererConfig::JsonLines {} => Err(RendererError::NotYetImplemented("json_lines")),
        RendererConfig::Tauri {} => Err(RendererError::NotYetImplemented("tauri")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_terminal_with_default_options() {
        let cfg = RendererConfig::default();
        assert_eq!(
            cfg,
            RendererConfig::Terminal {
                options: RenderOptions::default()
            }
        );
    }

    #[test]
    fn terminal_default_round_trips_via_serde_yaml() {
        // Minimal YAML: just the discriminator. `options` defaults via
        // `#[serde(default)]` to `RenderOptions::default()`.
        let yaml = "format: terminal\n";
        let cfg: RendererConfig = serde_yaml::from_str(yaml).expect("parse default terminal");
        assert_eq!(
            cfg,
            RendererConfig::Terminal {
                options: RenderOptions::default()
            }
        );
    }

    #[test]
    fn terminal_round_trips_with_explicit_options() {
        let yaml = "format: terminal\noptions:\n  use_emoji: true\n  use_color: false\n";
        let cfg: RendererConfig = serde_yaml::from_str(yaml).expect("parse explicit terminal");
        assert_eq!(
            cfg,
            RendererConfig::Terminal {
                options: RenderOptions {
                    use_emoji: true,
                    use_color: false,
                }
            }
        );
        // And it round-trips back.
        let serialized = serde_yaml::to_string(&cfg).expect("serialize");
        let reparsed: RendererConfig = serde_yaml::from_str(&serialized).expect("reparse");
        assert_eq!(cfg, reparsed);
    }

    #[test]
    fn terminal_partial_options_use_field_defaults() {
        // Only one of the two flags set — the other falls back to
        // `RenderOptions::default()` thanks to per-field `#[serde(default)]`.
        let yaml = "format: terminal\noptions:\n  use_emoji: true\n";
        let cfg: RendererConfig = serde_yaml::from_str(yaml).expect("parse partial options");
        assert_eq!(
            cfg,
            RendererConfig::Terminal {
                options: RenderOptions {
                    use_emoji: true,
                    use_color: false,
                }
            }
        );
    }

    #[test]
    fn terminal_build_returns_renderer() {
        let cfg = RendererConfig::Terminal {
            options: RenderOptions::default(),
        };
        let r = build_renderer(cfg);
        assert!(r.is_ok(), "terminal should build successfully");
    }

    #[test]
    fn json_lines_returns_not_yet_implemented() {
        let cfg = RendererConfig::JsonLines {};
        // `Box<dyn EventRenderer>` is not `Debug`, so we can't use `expect_err`;
        // pattern-match on `Result` instead.
        match build_renderer(cfg) {
            Ok(_) => panic!("json_lines should not build"),
            Err(RendererError::NotYetImplemented(name)) => assert_eq!(name, "json_lines"),
        }
    }

    #[test]
    fn tauri_returns_not_yet_implemented() {
        let cfg = RendererConfig::Tauri {};
        match build_renderer(cfg) {
            Ok(_) => panic!("tauri should not build"),
            Err(RendererError::NotYetImplemented(name)) => assert_eq!(name, "tauri"),
        }
    }

    #[test]
    fn json_lines_round_trips_via_serde_yaml() {
        let yaml = "format: json_lines\n";
        let cfg: RendererConfig = serde_yaml::from_str(yaml).expect("parse json_lines");
        assert_eq!(cfg, RendererConfig::JsonLines {});
    }

    #[test]
    fn tauri_round_trips_via_serde_yaml() {
        let yaml = "format: tauri\n";
        let cfg: RendererConfig = serde_yaml::from_str(yaml).expect("parse tauri");
        assert_eq!(cfg, RendererConfig::Tauri {});
    }
}
