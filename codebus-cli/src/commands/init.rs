//! `codebus init` — thin CLI wrapper around `codebus_core::vault::init::run_init`.
//!
//! All orchestration (vault layout, raw mirror, manifest, etc.) lives in
//! core so the Tauri app can run the same flow silently. The CLI's only job
//! here is to translate `InitEvent` notifications into the banners and
//! debug lines the existing test suite expects.

use std::path::Path;
use std::process::ExitCode;

use codebus_core::config::StarterOutcome;
use codebus_core::render::{Banner, RenderOptions, print_banner};
use codebus_core::schema::NEUTRAL_RULES;
use codebus_core::skill_bundle;
use codebus_core::vault::init::{
    InitError, InitEvent, InitOptions, bytes_to_mib, on_hit_label, pii_scanner_label, run_init,
};
use codebus_core::vault::manifest::ManifestOutcome;
use codebus_core::vault::obsidian_register::RegisterOutcome;
use codebus_core::vault::settings::{self, SettingsOutcome};
use codebus_core::vault::source_gitignore::GitignoreOutcome;

pub async fn run(
    repo: &Path,
    no_obsidian_register: bool,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    if debug {
        eprintln!(
            "[debug] init: repo={}, no_obsidian_register={no_obsidian_register}",
            repo.display()
        );
    }

    let opts = InitOptions {
        no_obsidian_register,
        write_starter_config: true,
    };

    match run_init(repo, &opts, |event| {
        handle_event(&event, debug, render_opts)
    }) {
        Ok(_) => ExitCode::SUCCESS,
        Err(InitError::Refused(refusal)) => {
            eprintln!("error: {refusal}");
            ExitCode::from(2)
        }
        Err(other) => {
            eprintln!("error: {other}");
            ExitCode::from(1)
        }
    }
}

fn handle_event(event: &InitEvent<'_>, debug: bool, render_opts: &RenderOptions) {
    match event {
        InitEvent::Start { repo } => {
            print_banner(Banner::Start { repo_path: repo }, render_opts);
        }
        InitEvent::LayoutCreated { paths } => {
            if debug {
                eprintln!(
                    "[debug] layout: created 7 dirs under {}",
                    paths.root.display()
                );
                println!("✓ vault layout: {}", paths.root.display());
            }
        }
        InitEvent::SourceGitignore { outcome } => match outcome {
            GitignoreOutcome::Created => {
                if debug {
                    println!("✓ source .gitignore: created with .codebus/");
                }
            }
            GitignoreOutcome::Appended => {
                if debug {
                    println!("✓ source .gitignore: appended .codebus/");
                }
            }
            GitignoreOutcome::AlreadyPresent => {
                if debug {
                    println!("✓ source .gitignore: already contains .codebus/");
                }
            }
            GitignoreOutcome::NotAGitRepo => {
                if debug {
                    eprintln!("[debug] source .gitignore: skipped (not a git repo)");
                }
            }
        },
        InitEvent::PiiConfigLoadWarn { message } => {
            eprintln!("warning: pii config load failed (using defaults): {message}");
        }
        InitEvent::PiiPatternsExtraWarn { message } => {
            eprintln!(
                "warning: pii config patterns_extra failed to compile (using built-in patterns only): {message}"
            );
        }
        InitEvent::RawSyncDone {
            paths,
            repo,
            summary,
            elapsed_ms,
            pii_cfg,
        } => {
            if debug {
                eprintln!(
                    "[debug] raw_sync: walked {} → {}, mirrored {} files / {} bytes / {} PII matches",
                    repo.display(),
                    paths.raw_code.display(),
                    summary.files,
                    summary.bytes,
                    summary.pii_matches
                );
                println!(
                    "✓ raw mirror: {} files, {} bytes, {} PII matches",
                    summary.files, summary.bytes, summary.pii_matches
                );
            }
            print_banner(
                Banner::SyncDone {
                    files: summary.files,
                    mib: bytes_to_mib(summary.bytes),
                    elapsed_ms: *elapsed_ms as u64,
                },
                render_opts,
            );
            print_banner(
                Banner::PiiSummary {
                    scanner: pii_scanner_label(&pii_cfg.scanner),
                    scanned: summary.files,
                    hits: summary.pii_matches,
                    action: on_hit_label(pii_cfg.on_hit),
                },
                render_opts,
            );
        }
        InitEvent::InternalGitignoreDone {
            path,
            required_count,
        } => {
            if debug {
                eprintln!(
                    "[debug] vault internal .gitignore: ensured {} required lines at {}",
                    required_count,
                    path.display()
                );
                println!("✓ vault internal .gitignore: ensured");
            }
        }
        InitEvent::NestedRepoDone {
            vault_root,
            already_initialized,
        } => {
            if debug {
                eprintln!(
                    "[debug] vault git: nested repo at {} ({})",
                    vault_root.join(".git").display(),
                    if *already_initialized {
                        "preserved"
                    } else {
                        "initialized"
                    }
                );
                println!(
                    "✓ vault git: {}",
                    if *already_initialized {
                        "already initialized"
                    } else {
                        "nested repo initialized"
                    }
                );
            }
        }
        InitEvent::SchemaDone { path: _, wrote_new } => {
            if debug {
                if *wrote_new {
                    eprintln!("[debug] schema: wrote {} bytes", NEUTRAL_RULES.len());
                    println!("✓ schema file: wrote .codebus/CLAUDE.md");
                } else {
                    println!("✓ schema file: .codebus/CLAUDE.md already present");
                }
            }
        }
        InitEvent::ManifestSignal { signal, .. } => {
            if debug {
                let head = signal
                    .git_head
                    .as_deref()
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "(non-git)".into());
                eprintln!(
                    "[debug] manifest source_signal: git_head={head:?}, file_count={}, total_bytes={}",
                    signal.file_count, signal.total_bytes
                );
            }
        }
        InitEvent::ManifestDone { outcome } => {
            if debug {
                match outcome {
                    ManifestOutcome::Written => {
                        println!("✓ manifest: wrote .codebus/manifest.yaml")
                    }
                    ManifestOutcome::Updated => {
                        println!("✓ manifest: updated sync state in .codebus/manifest.yaml")
                    }
                }
            }
        }
        InitEvent::SkillBundlesDone {
            vault_root,
            repo,
            written,
            preserved,
        } => {
            if debug {
                for verb in skill_bundle::VERBS {
                    let vp = skill_bundle::skill_bundle_path(vault_root, verb);
                    let rp = skill_bundle::skill_bundle_path(repo, verb);
                    eprintln!("[debug] skill bundle vault target: {}", vp.display());
                    eprintln!("[debug] skill bundle repo  target: {}", rp.display());
                }
                println!(
                    "✓ skill bundles: {} written, {} already present (across vault and repo locations)",
                    written, preserved
                );
            }
        }
        InitEvent::SettingsDone {
            vault_root,
            outcome,
        } => {
            let settings_path = settings::settings_json_path(vault_root);
            match outcome {
                SettingsOutcome::Written => {
                    if debug {
                        eprintln!("[debug] settings.json: wrote {}", settings_path.display());
                        println!("✓ vault settings: wrote .codebus/.claude/settings.json");
                    }
                }
                SettingsOutcome::AlreadyPresent => {
                    if debug {
                        eprintln!(
                            "[debug] settings.json: preserved existing {}",
                            settings_path.display()
                        );
                        println!(
                            "✓ vault settings: .codebus/.claude/settings.json already present"
                        );
                    }
                }
            }
        }
        InitEvent::ObsidianResult { wiki_path, outcome } => match outcome {
            RegisterOutcome::Registered { vault_id, was_new } => {
                if debug {
                    eprintln!(
                        "[debug] obsidian: vault entry {} (id={vault_id}) for path {}",
                        if *was_new { "inserted" } else { "refreshed" },
                        wiki_path.display()
                    );
                    println!(
                        "✓ obsidian: vault {} (id={vault_id})",
                        if *was_new { "registered" } else { "refreshed" }
                    );
                }
            }
            RegisterOutcome::ObsidianNotInstalled => {
                eprintln!("hint: Obsidian config dir not found; skipping vault registration");
            }
            RegisterOutcome::IoError { reason } => {
                eprintln!("warning: obsidian register failed (non-fatal): {reason}");
            }
        },
        InitEvent::ObsidianSkipped => {
            if debug {
                eprintln!("[debug] obsidian: skipped (--no-obsidian-register)");
            }
        }
        InitEvent::CommitDone { head_sha, sha7 } => {
            if debug {
                eprintln!("[debug] vault git: HEAD now {head_sha}");
                println!("✓ vault git: committed {sha7} \"init: codebus vault\"");
            }
            print_banner(Banner::CommitDone { sha7 }, render_opts);
        }
        InitEvent::StarterConfigUnavailable => {
            if debug {
                eprintln!("[debug] global config: home dir unavailable, skipping starter write");
            }
        }
        InitEvent::StarterConfigDone { path, outcome } => {
            if debug {
                match outcome {
                    StarterOutcome::Written => {
                        eprintln!("[debug] global config: wrote starter at {}", path.display());
                        println!("✓ global config: wrote {}", path.display());
                    }
                    StarterOutcome::AlreadyPresent => {
                        eprintln!(
                            "[debug] global config: preserved existing {}",
                            path.display()
                        );
                        println!("✓ global config: {} already present", path.display());
                    }
                }
            }
        }
        InitEvent::StarterConfigError { path, message } => {
            eprintln!(
                "warning: global config write at {} failed (non-fatal): {message}",
                path.display()
            );
        }
        InitEvent::Finished {
            paths,
            obsidian_registered,
        } => {
            print_banner(
                Banner::Done {
                    wiki_path: &paths.wiki,
                },
                render_opts,
            );
            if *obsidian_registered {
                print_banner(
                    Banner::Hint {
                        wiki_path: &paths.wiki,
                    },
                    render_opts,
                );
            }
        }
    }
}
