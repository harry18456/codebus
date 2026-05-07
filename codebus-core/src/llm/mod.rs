pub mod factory;
pub mod provider;
pub mod providers;

pub use factory::{ProviderConfig, build_provider};
pub use provider::{EventStream, InvokeOptions, LlmMode, LlmProvider, ProviderError};
pub use providers::claude_cli::{
    ClaudeCliProvider, ExitVerdict, FORBIDDEN_TOOLS, build_argv, classify_exit,
};
