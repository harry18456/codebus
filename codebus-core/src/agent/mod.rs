//! Agent CLI invocation surface.
//!
//! The provider-agnostic [`AgentBackend`] trait ([`backend`]) is the seam
//! between the provider-neutral invocation loop ([`invoke`](claude_cli::invoke))
//! and a concrete agent CLI. [`invoke`] drives a backend via three methods
//! (build command / parse stream line / extract session id) and owns the
//! provider-neutral spawn loop. The verb layer constructs a neutral
//! [`SpawnSpec`] ([`spawn_spec`]) describing one spawn's intent. Two backends
//! implement the trait today — [`ClaudeBackend`] ([`claude_backend`]) and
//! [`CodexBackend`] ([`codex_backend`]) — the second landed as a pure addition
//! with no change to this seam, validating the provider-neutral design.

pub mod backend;
pub mod claude_backend;
pub mod claude_cli;
pub mod codex_backend;
pub mod dispatch;
pub mod env_overrides;
pub(crate) mod process_kill;
pub mod spawn_spec;

pub use backend::AgentBackend;
pub use claude_backend::ClaudeBackend;
pub use codex_backend::{CODEX_AZURE_KEY_ENV, CODEX_VAULT_MARKER, CodexBackend};
pub use dispatch::{ProviderConfig, build_backend, load_provider_config, parse_provider_config};
pub use claude_cli::{InvokeReport, invoke};
pub use env_overrides::EnvOverrides;
pub use spawn_spec::{CommandPrefix, Permission, SpawnSpec};
