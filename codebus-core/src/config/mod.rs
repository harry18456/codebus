//! Global config plugin domain. Loads `~/.codebus/config.yaml` and turns
//! it into a [`schema::GlobalConfig`] that the CLI merges into render
//! options + each plugin factory's input.

pub mod loader;
pub mod schema;

pub use loader::{config_path, load_config, load_config_from_path};
pub use schema::{
    AutoFixConfig, EmojiMode, GlobalConfig, LintConfig, LlmConfig, LogConfig, PiiConfig,
    RenderConfig,
};
