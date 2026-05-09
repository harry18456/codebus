//! Fix loop module — agent-driven repair with CLI outer ping safety net.
//!
//! Per v3-lint design Decision §"Agent-driven self-loop":
//! - Agent self-loops in-session (one process, agent decides when done).
//! - CLI verifies post-agent by running lint again.
//! - On remaining issues, CLI uses `--resume <uuid>` to send a follow-up
//!   prompt up to `outer_ping_max` times (default 2).
//!
//! Sandbox per Fix Loop Agent Sandbox: `Read,Glob,Grep,Write,Edit` plus
//! `Bash(codebus lint *)` whitelist (no other binary, no other Bash usage).

pub mod prompt;
pub mod session;

use crate::agent::{InvokeAgentOptions, SessionAction, invoke};
use crate::wiki::lint::lint_wiki;
use crate::wiki::types::LintResult;
use std::io;
use std::path::PathBuf;

/// Toolset for fix agent (excluding the Bash whitelist, appended separately).
pub const FIX_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

/// Bash permission specifier per Fix Loop Agent Sandbox: agent may only
/// invoke `codebus lint` with arbitrary args, no other binary.
pub const FIX_BASH_WHITELIST: &str = "Bash(codebus lint *)";

/// Outcome of one full fix loop run (initial spawn + zero or more outer pings).
#[derive(Debug)]
pub struct FixLoopReport {
    /// Total agent invocations performed (1 initial + N pings).
    pub agent_invocations: u32,
    /// Lint result after the final agent run.
    pub final_lint: LintResult,
    /// `true` iff `final_lint.error_count == 0 && final_lint.warn_count == 0`.
    pub clean: bool,
    /// Reason the loop stopped.
    pub termination: TerminationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationReason {
    /// Initial lint reported zero issues; no agent was spawned.
    InitialClean,
    /// A post-agent lint reported zero issues; loop terminated successfully.
    PostLintClean,
    /// `outer_ping_max` ping attempts exhausted with issues still remaining.
    PingBudgetExhausted,
}

#[derive(Debug)]
pub enum FixLoopError {
    Spawn(io::Error),
    LintIo(io::Error),
}

impl std::fmt::Display for FixLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixLoopError::Spawn(e) => write!(f, "spawn fix agent: {e}"),
            FixLoopError::LintIo(e) => write!(f, "lint check during fix loop: {e}"),
        }
    }
}

impl std::error::Error for FixLoopError {}

/// Run the fix loop against a vault.
///
/// 1. Run lint once. If 0 issues → return InitialClean.
/// 2. Generate UUID, spawn `claude -p "/codebus-fix"` with --session-id.
/// 3. Run lint again. If 0 issues → return PostLintClean.
/// 4. While iterations < outer_ping_max: spawn with --resume + follow-up
///    prompt. Re-run lint. If 0 issues → return PostLintClean.
/// 5. If we reach iteration cap with issues remaining → return
///    PingBudgetExhausted.
///
/// `vault_root` is the `.codebus/` directory.
pub fn run_fix_loop(
    vault_root: PathBuf,
    outer_ping_max: u32,
) -> Result<FixLoopReport, FixLoopError> {
    let initial = lint_wiki(&vault_root);
    if initial.error_count == 0 && initial.warn_count == 0 {
        return Ok(FixLoopReport {
            agent_invocations: 0,
            final_lint: initial,
            clean: true,
            termination: TerminationReason::InitialClean,
        });
    }

    let uuid = session::new_uuid();
    let mut invocations = 0u32;

    // Initial spawn (Start session).
    invoke_fix_agent(
        &vault_root,
        prompt::initial_prompt(),
        SessionAction::Start(uuid.clone()),
    )
    .map_err(FixLoopError::Spawn)?;
    invocations += 1;

    // Post-agent verification + ping loop.
    for ping_idx in 0..=outer_ping_max {
        let post = lint_wiki(&vault_root);
        let clean = post.error_count == 0 && post.warn_count == 0;
        if clean {
            return Ok(FixLoopReport {
                agent_invocations: invocations,
                final_lint: post,
                clean: true,
                termination: TerminationReason::PostLintClean,
            });
        }
        if ping_idx == outer_ping_max {
            // We've reached the cap — return without spawning another ping.
            return Ok(FixLoopReport {
                agent_invocations: invocations,
                final_lint: post,
                clean: false,
                termination: TerminationReason::PingBudgetExhausted,
            });
        }
        // Spawn follow-up ping with --resume.
        invoke_fix_agent(
            &vault_root,
            prompt::followup_prompt(&post, &vault_root),
            SessionAction::Resume(uuid.clone()),
        )
        .map_err(FixLoopError::Spawn)?;
        invocations += 1;
    }

    // Unreachable — the for-loop always returns once inside the body.
    unreachable!("fix loop control flow")
}

fn invoke_fix_agent(
    vault_root: &std::path::Path,
    slash_command: String,
    session: SessionAction,
) -> io::Result<()> {
    let status = invoke(InvokeAgentOptions {
        slash_command,
        vault_root: vault_root.to_path_buf(),
        toolset: FIX_TOOLSET,
        bash_whitelist: Some(FIX_BASH_WHITELIST),
        session: Some(session),
    })?;
    // We don't propagate non-zero exit: the agent's "I'm done" or "I gave up"
    // signal flows back via post-spawn lint check. Spawn-level IO errors
    // (binary missing, fork fail) bubble up; agent's own non-zero exit is
    // expected ("nothing more I can do") and handled by the next lint round.
    let _ = status;
    Ok(())
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

    /// Spec scenario: "Fix loop skips agent entirely when initial lint
    /// reports zero issues" — the LLM provider's invoke is NOT called.
    #[test]
    fn run_fix_loop_skips_spawn_on_clean_initial_lint() {
        let tmp = make_clean_vault();
        // CODEBUS_CLAUDE_BIN points at a nonexistent binary; if loop tries
        // to spawn it, the spawn will Err. This proves no spawn happened.
        unsafe {
            std::env::set_var("CODEBUS_CLAUDE_BIN", "/no/such/claude/bin/test-clean-skip");
        }
        let result = run_fix_loop(tmp.path().to_path_buf(), 2);
        unsafe {
            std::env::remove_var("CODEBUS_CLAUDE_BIN");
        }
        let report = result.expect("clean vault should not invoke agent");
        assert_eq!(report.agent_invocations, 0);
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
}
