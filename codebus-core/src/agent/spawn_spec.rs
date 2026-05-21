//! Provider-neutral spawn intent.
//!
//! `SpawnSpec` is the inward half of the `AgentBackend` contract (see
//! [`super::backend`]): the verb layer constructs it to describe ONE agent
//! invocation in provider-neutral terms, and a concrete backend translates
//! it into that provider's argv. It deliberately carries NO provider-specific
//! encoding — no slash-command string, no CLI flag glob syntax. The backend
//! owns all of that.
//!
//! Field rationale (see `agent-backend` spec `SpawnSpec Provider-Neutral
//! Intent`):
//! - `prompt` is the fully-composed codebus skill invocation (e.g.
//!   `/codebus-goal verify: goal=...`), built by the verb layer. It is
//!   provider-neutral: the SKILL bundle is double-written identically for
//!   every provider, so the same invocation string is meaningful to all of
//!   them. The Claude-specific part is only the DELIVERY (`-p <prompt>`),
//!   which lives in the backend — not the string content.
//! - `verb` reuses the existing [`crate::config::Verb`] enum. It is the
//!   config-resolution key only: the backend resolves model/effort from its
//!   own config via `resolve(verb)`. (It is NOT used to build the prompt —
//!   the prompt is richer and phase-specific, e.g. goal main vs verify vs
//!   repair all share the slash name `goal` but differ in body and resolve
//!   to `Verb::Goal` vs `Verb::Verify`.) No separate `SpawnRole` enum is
//!   introduced — that would duplicate `Verb` + `resolve`.
//! - `permission`, `command_allowance`, and `resume_session_id` are
//!   per-spawn, NOT derived from `verb`: a single verb (quiz) issues multiple
//!   spawns with differing permission (plan ReadOnly, generate
//!   ReadOnly+allowance).

use crate::config::Verb;

/// The sandbox/permission posture for one spawn, expressed neutrally.
/// A backend maps this to its own mechanism (Claude: read-only vs
/// write-capable `--tools` set; a future sandbox-based provider: a sandbox
/// level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Read-only exploration: no write/edit capability.
    ReadOnly,
    /// Workspace-write: may create/modify files.
    Workspace,
}

/// A neutral command-prefix allowance: the leading tokens of the single
/// command family the agent may run (e.g. `["codebus", "quiz", "validate"]`).
/// Each backend formats this into its own fine-grained permission syntax —
/// Claude into a `Bash(<tokens joined> *)` `--allowedTools` specifier; a
/// provider without per-command gating degrades (best-effort + warning).
/// SHALL NOT carry provider-specific glob/flag syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPrefix(pub Vec<String>);

impl CommandPrefix {
    /// Convenience constructor from string slices.
    pub fn new<I, S>(tokens: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        CommandPrefix(tokens.into_iter().map(Into::into).collect())
    }

    /// The tokens, space-joined (e.g. `"codebus quiz validate"`). Backends
    /// use this when formatting their own permission syntax.
    pub fn joined(&self) -> String {
        self.0.join(" ")
    }
}

/// Provider-neutral intent for one agent spawn. See module docs.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    /// The config-resolution key: the backend resolves model/effort from its
    /// own config via `resolve(verb)`. NOT used to build the prompt.
    pub verb: Verb,
    /// The fully-composed codebus skill invocation string (built by the verb
    /// layer; e.g. `/codebus-quiz generate: pages=[...] count=...`). Neutral:
    /// the backend decides how to deliver it (Claude embeds it via `-p`).
    pub prompt: String,
    /// Sandbox posture for this spawn.
    pub permission: Permission,
    /// Optional fine-grained command allowance (neutral token prefix).
    pub command_allowance: Option<CommandPrefix>,
    /// Session id to resume, when continuing a multi-turn conversation.
    pub resume_session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `agent-backend` spec `SpawnSpec Provider-Neutral Intent`: the type
    /// carries neutral intent and `command_allowance` holds a token sequence,
    /// NOT a Claude `--allowedTools` glob string.
    #[test]
    fn spawn_spec_carries_neutral_intent() {
        let spec = SpawnSpec {
            verb: Verb::Quiz,
            prompt: "/codebus-quiz generate: pages=[rust] count=5".into(),
            permission: Permission::ReadOnly,
            command_allowance: Some(CommandPrefix::new(["codebus", "quiz", "validate"])),
            resume_session_id: None,
        };
        assert_eq!(spec.verb, Verb::Quiz);
        assert_eq!(spec.prompt, "/codebus-quiz generate: pages=[rust] count=5");
        assert_eq!(spec.permission, Permission::ReadOnly);
        let allowance = spec.command_allowance.expect("allowance present");
        // Neutral token sequence — not a glob string.
        assert_eq!(allowance.0, vec!["codebus", "quiz", "validate"]);
        assert_eq!(allowance.joined(), "codebus quiz validate");
        assert!(!allowance.joined().contains('*'));
        assert!(!allowance.joined().contains("Bash("));
        assert!(spec.resume_session_id.is_none());
    }

    #[test]
    fn permission_variants_are_distinct() {
        assert_ne!(Permission::ReadOnly, Permission::Workspace);
    }
}
