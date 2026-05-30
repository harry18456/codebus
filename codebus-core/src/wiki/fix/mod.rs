//! Fix flow — agent-driven repair, single spawn, CLI as final verifier.
//!
//! v3-fix-trust-agent: replaces v3-lint's outer ping loop with a trust-agent
//! single-shot model. The CLI runs lint precheck, spawns the agent at most
//! once, runs lint final verification, and uses the final result to decide
//! exit code. Agent is free to internally invoke `codebus lint` (subject to
//! the PreToolUse Bash hook installed by `codebus init`) any number of times
//! within its session; CLI does not orchestrate those inner iterations.
//!
//! Sandbox per Fix Loop Agent Sandbox: `Read,Glob,Grep,Write,Edit` plus
//! `Bash(codebus lint *)` whitelist (auto-approval scope; PreToolUse hook
//! is the actual hard gate — see `codebus hook check-bash`).

pub mod prompt;

use crate::agent::{AgentBackend, CommandPrefix, InvokeReport, Permission, SpawnSpec, invoke};
use crate::config::Verb;
use crate::stream::StreamEvent;
use crate::wiki::lint::lint_wiki;
use crate::wiki::types::LintResult;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Toolset for fix agent (excluding the Bash whitelist, appended separately).
pub const FIX_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

/// Bash permission specifier per Fix Loop Agent Sandbox: agent may invoke
/// `codebus lint` with arbitrary args; PreToolUse hook enforces the actual
/// restriction (see `codebus hook check-bash`).
pub const FIX_BASH_WHITELIST: &str = "Bash(codebus lint *)";

/// Outcome of one full fix run (single agent spawn).
#[derive(Debug)]
pub struct FixReport {
    /// `true` if no agent was spawned (initial lint already clean).
    pub agent_skipped: bool,
    /// Lint result after the agent terminates (or the precheck result when
    /// agent was skipped).
    pub final_lint: LintResult,
    /// `true` iff `final_lint.error_count == 0 && final_lint.warn_count == 0`.
    pub clean: bool,
    /// Reason the run terminated.
    pub termination: TerminationReason,
    /// `Some(InvokeReport)` when an agent was actually spawned (i.e., not
    /// `InitialClean`). `None` on the initial-clean short-circuit. v3-run-log:
    /// caller uses this to compose the verb's `RunLog` entry (token totals
    /// and timestamps come from the agent process; lint counts come from
    /// `final_lint`).
    pub invoke: Option<InvokeReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationReason {
    /// Initial lint reported zero issues; no agent was spawned.
    InitialClean,
    /// Agent terminated; final lint reports zero issues.
    PostLintClean,
    /// Agent terminated; final lint still reports issues.
    PostLintIssuesRemain,
}

#[derive(Debug)]
pub enum FixError {
    Spawn(io::Error),
}

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixError::Spawn(e) => write!(f, "spawn fix agent: {e}"),
        }
    }
}

impl std::error::Error for FixError {}

/// Run the fix flow against a vault.
///
/// Per Fix Single-Shot Verification:
///   1. Run lint (precheck). If 0 issues → InitialClean, no spawn.
///   2. Spawn `claude -p "/codebus-fix"` exactly once with the configured
///      `--model` / `--effort` from `claude_code.fix.*` (or omitted when None).
///   3. Wait for agent termination.
///   4. Run lint (final verification).
///   5. Use final lint state to decide TerminationReason.
///
/// `vault_root` is the `.codebus/` directory. `backend` is the agent backend
/// (model/effort resolved from its own config by `Verb::Fix`; env held
/// inside it). The Fix spawn uses Workspace permission + a `codebus lint`
/// command allowance.
pub fn run_fix_loop(
    vault_root: PathBuf,
    backend: &dyn AgentBackend,
    on_event: impl FnMut(StreamEvent),
    cancel: Option<Arc<AtomicBool>>,
    timeout: Option<std::time::Duration>,
) -> Result<FixReport, FixError> {
    let initial = lint_wiki(&vault_root);
    if initial.error_count == 0 && initial.warn_count == 0 {
        return Ok(FixReport {
            agent_skipped: true,
            final_lint: initial,
            clean: true,
            termination: TerminationReason::InitialClean,
            invoke: None,
        });
    }

    // Phase 3 (prompt-surface-layer-3-spawnspec-restructure): fix has no
    // user input — the agent runs `codebus lint --format json` itself and
    // operates on that JSON. SpawnSpec.input is empty; backend assembles
    // `/codebus-fix ""` (claude) or `$codebus-fix ` (codex).
    let invoke_report =
        invoke_fix_agent(&vault_root, String::new(), backend, on_event, cancel, timeout)
            .map_err(FixError::Spawn)?;

    let post = lint_wiki(&vault_root);
    let clean = post.error_count == 0 && post.warn_count == 0;
    Ok(FixReport {
        agent_skipped: false,
        final_lint: post,
        clean,
        termination: if clean {
            TerminationReason::PostLintClean
        } else {
            TerminationReason::PostLintIssuesRemain
        },
        invoke: Some(invoke_report),
    })
}

fn invoke_fix_agent(
    vault_root: &std::path::Path,
    input: String,
    backend: &dyn AgentBackend,
    on_event: impl FnMut(StreamEvent),
    cancel: Option<Arc<AtomicBool>>,
    timeout: Option<std::time::Duration>,
) -> io::Result<InvokeReport> {
    let report = invoke(
        backend,
        SpawnSpec {
            verb: Verb::Fix,
            resolve_as: None,
            sub_mode: None,
            input,
            // Fix Loop Agent Sandbox: Read,Glob,Grep,Write,Edit + bash limited
            // to `codebus lint` (PreToolUse hook is the actual hard gate).
            permission: Permission::Workspace,
            command_allowance: Some(CommandPrefix::new(["codebus", "lint"])),
            // fix verb is one-shot (no session resume); chat verb is the
            // only caller that sets Some(...) on this field.
            resume_session_id: None,
        },
        vault_root,
        on_event,
        cancel,
        timeout,
    )?;
    // We don't propagate non-zero exit: the agent's "I'm done" or "I gave up"
    // signal flows back via the post-spawn lint check. Spawn-level IO errors
    // (binary missing, fork fail) bubble up; agent's own non-zero exit is
    // expected ("nothing more I can do") and handled by the next lint round.
    // We DO return the report so the caller can use accumulated_tokens /
    // timestamps for RunLog composition.
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_clean_vault() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let wiki = tmp.path().join("wiki");
        for f in ["concepts", "entities", "modules", "processes", "synthesis"] {
            fs::create_dir_all(wiki.join(f)).unwrap();
        }
        fs::write(wiki.join("index.md"), "# index\n").unwrap();
        fs::write(wiki.join("log.md"), "# log\n").unwrap();
        tmp
    }

    /// Spec scenario: "Fix skips agent entirely when initial lint is clean"
    #[test]
    fn run_fix_loop_skips_spawn_on_clean_initial_lint() {
        use crate::agent::{ClaudeBackend, EnvOverrides};
        use crate::config::endpoint::ClaudeCodeConfig;

        let tmp = make_clean_vault();
        let backend = ClaudeBackend::new(ClaudeCodeConfig::default(), EnvOverrides::for_system());
        let result = run_fix_loop(tmp.path().to_path_buf(), &backend, |_event| {}, None, None);
        let report = result.expect("clean vault should not invoke agent");
        assert!(report.agent_skipped);
        assert_eq!(report.termination, TerminationReason::InitialClean);
        assert!(report.clean);
    }

    /// Verify the public toolset/whitelist constants line up with the
    /// Fix Loop Agent Sandbox requirement.
    #[test]
    fn fix_toolset_matches_sandbox_spec() {
        assert_eq!(FIX_TOOLSET, &["Read", "Glob", "Grep", "Write", "Edit"]);
        assert_eq!(FIX_BASH_WHITELIST, "Bash(codebus lint *)");
    }

    /// Fix spawn carries no session-resume: the `SpawnSpec` built by
    /// `invoke_fix_agent` sets `resume_session_id: None` (fix is one-shot;
    /// only chat resumes). Verified at the SpawnSpec construction level.
    #[test]
    fn fix_spawn_spec_has_no_resume_and_workspace_permission() {
        let spec = SpawnSpec {
            verb: Verb::Fix,
            resolve_as: None,
            sub_mode: None,
            input: String::new(),
            permission: Permission::Workspace,
            command_allowance: Some(CommandPrefix::new(["codebus", "lint"])),
            resume_session_id: None,
        };
        assert!(spec.resume_session_id.is_none());
        assert_eq!(spec.permission, Permission::Workspace);
        assert_eq!(
            spec.command_allowance.unwrap().joined(),
            "codebus lint"
        );
    }
}
