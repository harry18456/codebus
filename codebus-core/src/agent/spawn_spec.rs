//! Provider-neutral spawn intent.
//!
//! `SpawnSpec` is the inward half of the `AgentBackend` contract (see
//! [`super::backend`]): the verb layer constructs it to describe ONE agent
//! invocation in **provider-neutral, structured** terms, and a concrete
//! backend translates it into that provider's argv. It deliberately carries
//! NO provider-specific encoding — no slash-command string, no dollar-prefix
//! string, no CLI flag glob syntax. The backend owns all of that.
//!
//! Per `agent-backend` spec `SpawnSpec Provider-Neutral Intent`
//! (prompt-surface-layer-3-spawnspec-restructure):
//!
//! - `verb` is the SKILL bundle name — one of `Verb::Goal` / `Query` / `Fix`
//!   / `Chat` / `Quiz` (the five bundles materialized under both
//!   `.claude/skills/codebus-<verb>/` and `.codex/skills/codebus-<verb>/`).
//!   `Verb::Verify` SHALL NOT appear in this field — Verify is a
//!   model-resolution key, not a bundle name. Cross-flow verify spawns
//!   (goal verify, quiz content-verify) carry `verb: Goal` / `verb: Quiz`
//!   (their actual bundle) and use `resolve_as` for the Verify config.
//! - `resolve_as` is an optional model-resolution override. When `None`,
//!   the backend uses `resolve(verb)` to pick model/effort. When
//!   `Some(other)`, the backend uses `resolve(other)` instead. This exists
//!   for the verify-stage-independent-model pattern: a goal verify spawn
//!   sets `verb: Goal, resolve_as: Some(Verify)` so the SKILL bundle is
//!   `/codebus-goal verify: ...` while model/effort come from the dedicated
//!   `verify` config sub-block (cheap generation + expensive verification).
//! - `sub_mode` names a verb sub-mode (`verify`, `repair`, `plan`,
//!   `generate`) when present, OR is `None` for free-text invocations
//!   (chat / goal ingest / query). The backend uses sub_mode to choose the
//!   assembly form: with-mode-prefix (`/codebus-<verb> <mode>: <input>`)
//!   vs free-text (`/codebus-<verb> "<input>"` on claude, no quotes on
//!   codex).
//! - `input` is the raw user text (free-text spawns) or the structured
//!   body (sub-mode spawns, e.g. `goal=...\n\nCHANGED PAGES:\n...`).
//!   Backend SHALL NOT see a pre-composed `/codebus-` or `$codebus-`
//!   prefix here — assembly is backend's responsibility.
//! - `permission`, `command_allowance`, `sub_mode`, `resolve_as`, and
//!   `resume_session_id` are all per-spawn (NOT derived from `verb`)
//!   because a single verb can issue multiple spawns with differing
//!   permission / sub-mode / config-override (quiz issues plan,
//!   generate, and verify spawns — three different specs).
//!
//! Backend assembly forms (per spec scenarios):
//! - **Claude**: `Some(mode)` → `/codebus-<verb> <mode>: <input>`;
//!   `None` → `/codebus-<verb> "<input>"` (quote-wrapped free-text).
//!   Carried via the `-p` CLI flag.
//! - **Codex**: `Some(mode)` → `$codebus-<verb> <mode>: <input>`;
//!   `None` → `$codebus-<verb> <input>` (no quotes — F95 retraction
//!   verified modern LLM tolerance). Carried as the first positional
//!   argument. The `$`-prefix invokes codex's native skill
//!   explicit-invocation mechanism (24.8% input-token saving vs the
//!   `/`-prefix description-match path; see inventory doc §16 F26).

use crate::config::Verb;

/// The sandbox/permission posture for one spawn, expressed neutrally.
/// A backend maps this to its own mechanism (Claude: read-only vs
/// write-capable `--tools` set; codex: sandbox `-s` level).
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

/// Provider-neutral structured intent for one agent spawn. See module docs.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    /// SKILL bundle name (one of `Goal` / `Query` / `Fix` / `Chat` / `Quiz`).
    /// NOT a model-resolution key — see `resolve_as` for the Verify override.
    pub verb: Verb,
    /// Optional model-resolution override. `None` = use `verb`; `Some(other)`
    /// = use `other` for `resolve()` (verify-stage-independent-model pattern).
    pub resolve_as: Option<Verb>,
    /// Optional verb sub-mode (`verify`, `repair`, `plan`, `generate`).
    /// `None` for free-text invocations.
    pub sub_mode: Option<String>,
    /// Raw user text (free-text) or structured body (sub-mode spawns).
    /// MUST NOT contain a pre-composed `/codebus-` or `$codebus-` prefix —
    /// backend assembly adds those.
    pub input: String,
    /// Sandbox posture for this spawn.
    pub permission: Permission,
    /// Optional fine-grained command allowance (neutral token prefix).
    pub command_allowance: Option<CommandPrefix>,
    /// Session id to resume, when continuing a multi-turn conversation.
    pub resume_session_id: Option<String>,
}

impl SpawnSpec {
    /// The effective model-resolution key: `resolve_as` if set, else `verb`.
    pub fn config_key(&self) -> Verb {
        self.resolve_as.unwrap_or(self.verb)
    }
}

/// Map the SKILL bundle verb to its lowercase bundle directory name
/// (`Verb::Goal` → `"goal"` — matches `<vault>/.codebus/.claude/skills/codebus-goal/`).
/// Only valid for the five SKILL bundle verbs; `Verb::Verify` panics because
/// Verify is never a bundle name (it is only a resolve_as override).
pub(crate) fn verb_bundle_name(verb: Verb) -> &'static str {
    match verb {
        Verb::Goal => "goal",
        Verb::Query => "query",
        Verb::Fix => "fix",
        Verb::Chat => "chat",
        Verb::Quiz => "quiz",
        Verb::Verify => panic!(
            "Verb::Verify is not a SKILL bundle name; cross-flow verify spawns set verb: Goal/Quiz and resolve_as: Some(Verify) instead"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `agent-backend` spec `SpawnSpec Provider-Neutral Intent`: the type
    /// carries neutral structured intent (verb + sub_mode + input + resolve_as),
    /// NOT a pre-composed prompt string, AND `command_allowance` holds a token
    /// sequence, NOT a Claude `--allowedTools` glob string.
    #[test]
    fn spawn_spec_carries_neutral_intent() {
        let spec = SpawnSpec {
            verb: Verb::Quiz,
            resolve_as: None,
            sub_mode: Some("generate".to_string()),
            input: "pages=[rust] count=5".to_string(),
            permission: Permission::ReadOnly,
            command_allowance: Some(CommandPrefix::new(["codebus", "quiz", "validate"])),
            resume_session_id: None,
        };
        assert_eq!(spec.verb, Verb::Quiz);
        assert_eq!(spec.sub_mode.as_deref(), Some("generate"));
        assert_eq!(spec.input, "pages=[rust] count=5");
        // No pre-composed prefix in input.
        assert!(!spec.input.starts_with("/codebus-"));
        assert!(!spec.input.starts_with("$codebus-"));
        assert_eq!(spec.permission, Permission::ReadOnly);
        let allowance = spec.command_allowance.expect("allowance present");
        assert_eq!(allowance.0, vec!["codebus", "quiz", "validate"]);
        assert_eq!(allowance.joined(), "codebus quiz validate");
        assert!(!allowance.joined().contains('*'));
        assert!(!allowance.joined().contains("Bash("));
        assert!(spec.resume_session_id.is_none());
    }

    #[test]
    fn config_key_defaults_to_verb() {
        let spec = SpawnSpec {
            verb: Verb::Goal,
            resolve_as: None,
            sub_mode: None,
            input: "draft".to_string(),
            permission: Permission::Workspace,
            command_allowance: None,
            resume_session_id: None,
        };
        assert_eq!(spec.config_key(), Verb::Goal);
    }

    #[test]
    fn config_key_uses_resolve_as_when_set() {
        // verify-stage-independent-model: goal verify spawn sets verb=Goal
        // (SKILL bundle) but resolves model/effort under Verb::Verify config.
        let spec = SpawnSpec {
            verb: Verb::Goal,
            resolve_as: Some(Verb::Verify),
            sub_mode: Some("verify".to_string()),
            input: "goal=X\n\nCHANGED PAGES:\n...".to_string(),
            permission: Permission::ReadOnly,
            command_allowance: None,
            resume_session_id: None,
        };
        assert_eq!(spec.config_key(), Verb::Verify);
        assert_eq!(spec.verb, Verb::Goal); // bundle name still Goal
    }

    #[test]
    fn permission_variants_are_distinct() {
        assert_ne!(Permission::ReadOnly, Permission::Workspace);
    }

    #[test]
    fn verb_bundle_name_maps_five_bundles() {
        assert_eq!(verb_bundle_name(Verb::Goal), "goal");
        assert_eq!(verb_bundle_name(Verb::Query), "query");
        assert_eq!(verb_bundle_name(Verb::Fix), "fix");
        assert_eq!(verb_bundle_name(Verb::Chat), "chat");
        assert_eq!(verb_bundle_name(Verb::Quiz), "quiz");
    }

    #[test]
    #[should_panic(expected = "not a SKILL bundle name")]
    fn verb_bundle_name_panics_on_verify() {
        let _ = verb_bundle_name(Verb::Verify);
    }
}
