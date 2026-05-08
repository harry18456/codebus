use std::fs;
use std::path::Path;
use std::process::ExitCode;

use codebus_core::schema::NEUTRAL_RULES;
use codebus_core::skill_bundle::{self, BundleOutcome};
use codebus_core::vault::layout::create_vault_layout;
use codebus_core::vault::manifest::{self, ManifestOutcome};
use codebus_core::vault::obsidian_register::{self, RegisterOutcome};
use codebus_core::vault::raw_sync::sync_with_null_scanner;
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use codebus_core::vault::source_gitignore::{self, GitignoreOutcome};

pub async fn run(repo: &Path, no_obsidian_register: bool, debug: bool) -> ExitCode {
    if debug {
        eprintln!("[debug] init: repo={}, no_obsidian_register={no_obsidian_register}", repo.display());
    }

    if let Err(refusal) = check_repo_is_not_vault(repo) {
        eprintln!("error: {refusal}");
        return ExitCode::from(2);
    }
    if debug {
        eprintln!("[debug] sanity_check: target is not a vault root → ok");
    }

    let paths = match create_vault_layout(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: create vault layout: {e}");
            return ExitCode::from(1);
        }
    };
    if debug {
        eprintln!("[debug] layout: created 7 dirs under {}", paths.root.display());
    }
    println!("✓ vault layout: {}", paths.root.display());

    let summary = match sync_with_null_scanner(repo, &paths.raw_code) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: raw mirror: {e}");
            return ExitCode::from(1);
        }
    };
    if debug {
        eprintln!(
            "[debug] raw_sync: walked {} → {}, mirrored {} files / {} bytes",
            repo.display(),
            paths.raw_code.display(),
            summary.files,
            summary.bytes
        );
    }
    println!(
        "✓ raw mirror: {} files, {} bytes",
        summary.files, summary.bytes
    );

    match source_gitignore::ensure_codebus_in_gitignore(repo) {
        Ok(GitignoreOutcome::Created) => println!("✓ source .gitignore: created with .codebus/"),
        Ok(GitignoreOutcome::Appended) => println!("✓ source .gitignore: appended .codebus/"),
        Ok(GitignoreOutcome::AlreadyPresent) => {
            println!("✓ source .gitignore: already contains .codebus/")
        }
        Ok(GitignoreOutcome::NotAGitRepo) => {
            if debug {
                eprintln!("[debug] source .gitignore: skipped (not a git repo)");
            }
        }
        Err(e) => {
            eprintln!("error: source .gitignore: {e}");
            return ExitCode::from(1);
        }
    }

    match write_schema_if_missing(&paths.schema_md) {
        Ok(true) => {
            if debug {
                eprintln!("[debug] schema: wrote {} bytes", NEUTRAL_RULES.len());
            }
            println!("✓ schema file: wrote .codebus/CLAUDE.md")
        }
        Ok(false) => println!("✓ schema file: .codebus/CLAUDE.md already present"),
        Err(e) => {
            eprintln!("error: schema file: {e}");
            return ExitCode::from(1);
        }
    }

    let signal = manifest::compute_source_signal(repo, &summary);
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
    match manifest::write_or_update_manifest(repo, &paths.root, env!("CARGO_PKG_VERSION"), signal) {
        Ok(ManifestOutcome::Written) => println!("✓ manifest: wrote .codebus/manifest.yaml"),
        Ok(ManifestOutcome::Updated) => println!("✓ manifest: updated sync state in .codebus/manifest.yaml"),
        Err(e) => {
            eprintln!("error: manifest: {e}");
            return ExitCode::from(1);
        }
    }

    if let Err(e) = write_skill_bundles(&paths.root, debug) {
        eprintln!("error: skill bundles: {e}");
        return ExitCode::from(1);
    }

    if !no_obsidian_register {
        match obsidian_register::register_vault(&paths.wiki) {
            RegisterOutcome::Registered { vault_id, was_new } => {
                if debug {
                    eprintln!(
                        "[debug] obsidian: vault entry {} (id={vault_id}) for path {}",
                        if was_new { "inserted" } else { "refreshed" },
                        paths.wiki.display()
                    );
                }
                println!(
                    "✓ obsidian: vault {} (id={vault_id})",
                    if was_new { "registered" } else { "refreshed" }
                );
            }
            RegisterOutcome::ObsidianNotInstalled => {
                eprintln!("hint: Obsidian config dir not found; skipping vault registration");
            }
            RegisterOutcome::IoError { reason } => {
                eprintln!("warning: obsidian register failed (non-fatal): {reason}");
            }
        }
    } else if debug {
        eprintln!("[debug] obsidian: skipped (--no-obsidian-register)");
    }

    println!("✓ codebus init complete");
    ExitCode::SUCCESS
}

fn write_schema_if_missing(schema_md: &Path) -> std::io::Result<bool> {
    if schema_md.exists() {
        return Ok(false);
    }
    if let Some(parent) = schema_md.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(schema_md, NEUTRAL_RULES)?;
    Ok(true)
}

fn write_skill_bundles(vault_root: &Path, debug: bool) -> std::io::Result<()> {
    let outcomes = skill_bundle::write_bundles_if_missing(vault_root)?;
    let written = outcomes.iter().filter(|o| **o == BundleOutcome::Written).count();
    let preserved = outcomes
        .iter()
        .filter(|o| **o == BundleOutcome::AlreadyPresent)
        .count();
    if debug {
        for verb in skill_bundle::VERBS {
            let p = skill_bundle::skill_bundle_path(vault_root, verb);
            eprintln!("[debug] skill bundle target: {}", p.display());
        }
    }
    println!(
        "✓ skill bundles: {} written, {} already present",
        written, preserved
    );
    Ok(())
}
