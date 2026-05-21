//! Agent CLI invocation surface.
//!
//! The provider-agnostic [`AgentBackend`] trait ([`backend`]) is the seam
//! between the provider-neutral invocation loop ([`invoke`](claude_cli::invoke))
//! and a concrete agent CLI. [`invoke`] drives a backend via three methods
//! (build command / parse stream line / extract session id) and owns the
//! provider-neutral spawn loop. The verb layer constructs a neutral
//! [`SpawnSpec`] ([`spawn_spec`]) describing one spawn's intent. The only
//! implementation today is [`ClaudeBackend`] ([`claude_backend`]); a second
//! provider (codex) lands in a future change as a pure addition — a new
//! backend module implementing the same trait, with no change to this seam.

pub mod backend;
pub mod claude_backend;
pub mod claude_cli;
pub mod env_overrides;
pub mod spawn_spec;

pub use backend::AgentBackend;
pub use claude_backend::ClaudeBackend;
pub use claude_cli::{InvokeReport, invoke};
pub use env_overrides::EnvOverrides;
pub use spawn_spec::{CommandPrefix, Permission, SpawnSpec};
