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

use crate::agent::{InvokeAgentOptions, InvokeReport, invoke};
use crate::render::RenderOptions;
use crate::wiki::lint::lint_wiki;
use crate::wiki::types::LintResult;
use std::io;
use std::path::PathBuf;

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
/// `vault_root` is the `.codebus/` directory. `model` / `effort` are the
/// pass-through values from `~/.codebus/config.yaml`'s `claude_code.fix`
/// section, or `None` to omit the flags from the spawned command line.
pub fn run_fix_loop(
    vault_root: PathBuf,
    model: Option<String>,
    effort: Option<String>,
    render_opts: &RenderOptions,
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

    let invoke_report =
        invoke_fix_agent(&vault_root, prompt::initial_prompt(), model, effort, render_opts)
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
    slash_command: String,
    model: Option<String>,
    effort: Option<String>,
    render_opts: &RenderOptions,
) -> io::Result<InvokeReport> {
    let report = invoke(
        InvokeAgentOptions {
            slash_command,
            vault_root: vault_root.to_path_buf(),
            toolset: FIX_TOOLSET,
            bash_whitelist: Some(FIX_BASH_WHITELIST),
            model,
            effort,
        },
        render_opts,
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
        let tmp = make_clean_vault();
        unsafe {
            std::env::set_var("CODEBUS_CLAUDE_BIN", "/no/such/claude/bin/test-clean-skip");
        }
        let result = run_fix_loop(
            tmp.path().to_path_buf(),
            None,
            None,
            &RenderOptions::no_styling(),
        );
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
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

    /// Spec scenario: "Fix spawn arguments contain no session continuity flags"
    /// — verified at compile time by the absence of session_id/resume options
    /// in `InvokeAgentOptions`. This test asserts the struct shape.
    #[test]
    fn invoke_options_has_no_session_field() {
        let opts = InvokeAgentOptions {
            slash_command: "/codebus-fix".into(),
            vault_root: PathBuf::from("/tmp"),
            toolset: FIX_TOOLSET,
            bash_whitelist: Some(FIX_BASH_WHITELIST),
            model: None,
            effort: None,
        };
        // Destructuring asserts the struct has exactly these fields and no
        // session-related fields. Compile fails if a session field is added.
        let InvokeAgentOptions {
            slash_command: _,
            vault_root: _,
            toolset: _,
            bash_whitelist: _,
            model: _,
            effort: _,
        } = opts;
    }
}
