//! `codebus goal "..."` — spawn `claude -p` with the codebus-goal slash
//! command, drive the agent against the vault, and commit the resulting
//! wiki snapshot. See openspec/changes/v3-goal/ for the full design.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::agent::{InvokeAgentOptions, invoke};
use codebus_core::git::auto_commit;
use codebus_core::pii::scanners::regex_basic::RegexBasicScanner;
use codebus_core::vault::layout::vault_paths;
use codebus_core::vault::manifest::{self, ManifestOutcome, SourceSignal};
use codebus_core::vault::raw_sync::{sync_with_scanner, walk_source_for_signal, SyncSummary};
use codebus_core::vault::source_signal_detect::detect_drift;

use crate::commands::init;

/// Toolset for goal: read code, write/edit wiki pages. No Bash (fix gets that),
/// no WebFetch / Task / NotebookEdit / etc. v2 iter-9 carry, spike-verified
/// 2026-05-09 to be a real hard gate.
const GOAL_TOOLSET: &[&str] = &["Read", "Glob", "Grep", "Write", "Edit"];

#[derive(Args, Debug)]
pub struct GoalArgs {
    /// What you want the wiki to capture or update.
    #[arg(value_name = "GOAL")]
    pub text: String,

    /// Force re-syncing the raw mirror even if source-signal detection
    /// reports no drift. Use when you know the source changed in a way
    /// the signal does not capture (e.g. file content changed but size
    /// stayed identical).
    #[arg(long = "force-resync")]
    pub force_resync: bool,
}

pub async fn run(
    repo: &Path,
    args: GoalArgs,
    no_obsidian_register: bool,
    debug: bool,
) -> ExitCode {
    let paths = vault_paths(repo);

    if debug {
        eprintln!(
            "[debug] goal: repo={}, vault={}, force_resync={}",
            repo.display(),
            paths.root.display(),
            args.force_resync
        );
    }

    // Step 1+2: vault precondition. v2 carry: missing vault → auto-init.
    if !paths.root.exists() {
        if debug {
            eprintln!(
                "[debug] goal: vault {} missing, running init flow",
                paths.root.display()
            );
        }
        let init_exit = init::run(repo, no_obsidian_register, debug).await;
        if !paths.root.exists() {
            // ExitCode is opaque (no PartialEq); detect failure by post-condition.
            eprintln!(
                "error: goal: auto-init did not create vault at {}",
                paths.root.display()
            );
            return init_exit;
        }
    }

    // Step 3: source-signal detection + conditional re-sync.
    let current_signal = match compute_current_signal(repo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: goal: compute source signal: {e}");
            return ExitCode::from(1);
        }
    };

    let needs_resync = args.force_resync || detect_drift(&paths.manifest_yaml, &current_signal);
    if debug {
        eprintln!(
            "[debug] goal: detect_drift={}, force_resync={}, needs_resync={}",
            !args.force_resync && needs_resync,
            args.force_resync,
            needs_resync
        );
    }

    if needs_resync {
        let scanner = match RegexBasicScanner::new(&[]) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: pii scanner init: {e}");
                return ExitCode::from(1);
            }
        };
        let summary = match sync_with_scanner(repo, &paths.raw_code, &scanner) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: raw mirror re-sync: {e}");
                return ExitCode::from(1);
            }
        };
        println!(
            "✓ raw mirror: {} files, {} bytes, {} PII matches (re-sync)",
            summary.files, summary.bytes, summary.pii_matches
        );
        let signal = manifest::compute_source_signal(repo, &summary);
        match manifest::write_or_update_manifest(
            repo,
            &paths.root,
            env!("CARGO_PKG_VERSION"),
            signal,
        ) {
            Ok(ManifestOutcome::Written) | Ok(ManifestOutcome::Updated) => {
                if debug {
                    eprintln!("[debug] goal: manifest source_signal updated after re-sync");
                }
            }
            Err(e) => {
                eprintln!("error: manifest update: {e}");
                return ExitCode::from(1);
            }
        }
    } else if debug {
        eprintln!("[debug] goal: source unchanged, skipping raw mirror re-sync");
    }

    // Step 4: spawn agent with canonical sandbox.
    let slash_command = format!("/codebus-goal \"{}\"", args.text);
    if debug {
        eprintln!(
            "[debug] goal: spawn claude with cwd={} slash={:?} toolset={:?}",
            paths.root.display(),
            slash_command,
            GOAL_TOOLSET
        );
    }
    let child_status = match invoke(InvokeAgentOptions {
        slash_command,
        vault_root: paths.root.clone(),
        toolset: GOAL_TOOLSET,
    }) {
        Ok(status) => status,
        Err(e) => {
            eprintln!("error: spawn claude: {e}");
            return ExitCode::from(1);
        }
    };
    let child_exit_code: u8 = child_status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1);
    if debug {
        eprintln!(
            "[debug] goal: agent exited code={}, success={}",
            child_status.code().unwrap_or(-1),
            child_status.success()
        );
    }

    // Step 5: auto-commit unconditionally (v2 carry behavior). Clean working
    // tree → no-op; dirty → commit "wiki: <goal>" preserving partial work
    // even when agent failed mid-flight.
    let commit_msg = format!("wiki: {}", args.text);
    match auto_commit(&paths.root, &commit_msg) {
        Ok(sha) => {
            let sha7: String = sha.chars().take(7).collect();
            println!("✓ vault git: committed {sha7} \"{commit_msg}\"");
        }
        Err(e) => {
            eprintln!("error: vault git auto-commit: {e}");
            return ExitCode::from(1);
        }
    }

    // Propagate child exit code (auto_commit success path).
    ExitCode::from(child_exit_code)
}

/// Compute the current source signal: walk source for file_count/total_bytes,
/// read git_head from `<repo>/.git/HEAD`. Reuses the manifest module's
/// `compute_source_signal` after replacing the SyncSummary's file/bytes with
/// the walk-only result (so we don't depend on having just run a real sync).
fn compute_current_signal(repo: &Path) -> std::io::Result<SourceSignal> {
    let (files, bytes) = walk_source_for_signal(repo)?;
    let stub_summary = SyncSummary {
        files,
        bytes,
        pii_matches: 0,
    };
    Ok(manifest::compute_source_signal(repo, &stub_summary))
}
