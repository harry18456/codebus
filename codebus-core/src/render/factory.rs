//! Renderer factory. Mirrors [`crate::llm::factory`] / [`crate::pii::factory`]
//! shape — explicit `match` over a [`RendererKind`] enum.

use crate::render::event_renderer::EventRenderer;
use crate::render::renderers::terminal::{RenderOptions, TerminalRenderer};

/// Discriminator for which renderer to build. `Terminal` is the day-one
/// only implementation; `JsonLines` and `Tauri` are reserved for follow-up
/// changes (see proposal §"EventRenderer trait").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RendererKind {
    #[default]
    Terminal,
    JsonLines,
    Tauri,
}

#[derive(Debug, Clone, Default)]
pub struct RendererConfig {
    pub kind: RendererKind,
    pub terminal: RenderOptions,
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
    match cfg.kind {
        RendererKind::Terminal => Ok(Box::new(TerminalRenderer::new(cfg.terminal))),
        RendererKind::JsonLines => Err(RendererError::NotYetImplemented("json_lines")),
        RendererKind::Tauri => Err(RendererError::NotYetImplemented("tauri")),
    }
}
