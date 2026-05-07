//! Output rendering plugin domain.
//!
//! Day-one wiring lands [`event_renderer::EventRenderer`] trait +
//! [`factory::build_renderer`] + [`renderers::terminal::TerminalRenderer`]
//! (default). Consumers pass `&mut dyn EventRenderer` everywhere stream
//! events / lint reports / banners are emitted, so swapping in a Tauri
//! webview emitter (Phase E) or a JSON-lines machine renderer (`--render
//! json`) is a one-line factory change.

pub mod event_renderer;
pub mod factory;
pub mod renderers;

pub use event_renderer::{Banner, EventRenderer};
pub use factory::{RendererConfig, RendererError, build_renderer};
pub use renderers::terminal::{RenderOptions, TerminalRenderer};
