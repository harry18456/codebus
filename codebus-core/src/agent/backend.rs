//! The `AgentBackend` trait — the sole contract between the
//! provider-agnostic invocation loop ([`super::claude_cli::invoke`]) and a
//! concrete agent CLI.
//!
//! See `agent-backend` spec `Agent Backend Trait Contract`. The trait has
//! exactly three methods and exposes NO tool / sandbox / MCP / model / argv
//! concepts to its caller — those are encapsulated entirely inside the
//! implementing type (e.g. [`super::claude_backend::ClaudeBackend`]). The
//! only thing a backend hands back out is the normalized cross-provider
//! [`StreamEvent`] (plus token usage carried within it).

use crate::stream::StreamEvent;
use std::process::Command;

use super::spawn_spec::SpawnSpec;

/// A concrete agent CLI backend. Implementors own argv composition, stream
/// format parsing, session-id extraction, their security posture (tool
/// gating / sandbox / MCP isolation), and their own config schema. The
/// invocation loop drives them through these three methods only.
pub trait AgentBackend: Send + Sync {
    /// Build the fully-formed child-process command for one spawn. The
    /// implementor translates the neutral [`SpawnSpec`] into its own argv
    /// (binary, flags, permission gating, model selection, env). The caller
    /// is responsible only for `current_dir` / stdio piping / spawn.
    fn build_command(&self, spec: &SpawnSpec) -> Command;

    /// Parse one raw stdout line into zero or more normalized
    /// [`StreamEvent`]s. Format-only: this maps the provider's wire format
    /// (JSONL, etc.) onto the shared event type. It SHALL NOT interpret
    /// codebus-semantic `[CODEBUS_*]` markers — those stay in the verb layer.
    fn parse_stream_line(&self, line: &str) -> Vec<StreamEvent>;

    /// Extract a session id from one raw stdout line, if present. Returns
    /// `Some(id)` for the line that carries the provider's session/thread
    /// identifier (Claude `system`/`init`; another provider's equivalent),
    /// `None` otherwise. The loop polls every line until the first hit.
    fn extract_session_id(&self, line: &str) -> Option<String>;
}
