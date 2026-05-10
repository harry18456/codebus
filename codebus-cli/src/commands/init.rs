use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use codebus_core::config::{
    PiiConfig, PiiScannerKind, StarterOutcome, default_config_path, load_pii_config,
    write_starter_config_if_missing,
};
use codebus_core::git::{auto_commit, init_nested_repo};
use codebus_core::pii::PiiScanner;
use codebus_core::pii::provider::OnHit;
use codebus_core::pii::scanners::null_scanner::NullScanner;
use codebus_core::pii::scanners::regex_basic::RegexBasicScanner;
use codebus_core::render::{Banner, RenderOptions, print_banner};
use codebus_core::schema::NEUTRAL_RULES;
use codebus_core::skill_bundle::{self, BundleOutcome};
use codebus_core::vault::layout::create_vault_layout;
use codebus_core::vault::manifest::{self, ManifestOutcome};
use codebus_core::vault::obsidian_register::{self, RegisterOutcome};
use codebus_core::vault::raw_sync::sync_with_scanner;
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use codebus_core::vault::settings::{self, SettingsOutcome};
use codebus_core::vault::source_gitignore::{self, GitignoreOutcome};

/// Required lines in the vault-internal `.codebus/.gitignore`. Excluding these
/// from nested git tracking keeps each `auto_commit` snapshot focused on
/// wiki evolution: `.lock` is per-process file lock state; `raw/code/` is
/// already tracked via source repo's git so duplicate-tracking it here
/// would noise every commit; `**/.obsidian/` is editor-local config user
/// shouldn't see in vault diff; `logs/` is verb invocation log noise;
/// `.claude/settings.local.json` is user's personal Claude Code overrides
/// (per Claude Code convention) and should not be tracked.
const INTERNAL_GITIGNORE_LINES: &[&str] = &[
    ".lock",
    "raw/code/",
    "**/.obsidian/",
    // v3-run-log: align with `vault::layout` which creates `log/` (singular,
    // not the `logs/` plural the prior line had — that line never matched
    // anything on disk and let runs-*.jsonl files dirty the working tree).
    "log/",
    ".claude/settings.local.json",
];

pub async fn run(
    repo: &Path,
    no_obsidian_register: bool,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
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

    // Banner: 駛入 — emitted before any per-step orchestration so the user
    // sees the codebus brand identity (the bus / boarding metaphor) at the
    // top of every run.
    print_banner(Banner::Start { repo_path: repo }, render_opts);

    let paths = match create_vault_layout(repo) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: create vault layout: {e}");
            return ExitCode::from(1);
        }
    };
    if debug {
        eprintln!("[debug] layout: created 7 dirs under {}", paths.root.display());
        println!("✓ vault layout: {}", paths.root.display());
    }

    // v3-bug-fixes: source `.gitignore` mutation must precede raw_sync so the
    // raw_sync summary's byte count reflects the post-mutation source state.
    // Otherwise the manifest writes a pre-mutation byte count and subsequent
    // verb invocations (goal/query) computing a fresh signal see drift even
    // though the user changed nothing.
    match source_gitignore::ensure_codebus_in_gitignore(repo) {
        Ok(GitignoreOutcome::Created) => {
            if debug {
                println!("✓ source .gitignore: created with .codebus/");
            }
        }
        Ok(GitignoreOutcome::Appended) => {
            if debug {
                println!("✓ source .gitignore: appended .codebus/");
            }
        }
        Ok(GitignoreOutcome::AlreadyPresent) => {
            if debug {
                println!("✓ source .gitignore: already contains .codebus/");
            }
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

    let pii_cfg = load_pii_config_with_warning();
    let scanner = build_pii_scanner(&pii_cfg);
    let sync_started = Instant::now();
    let summary = match sync_with_scanner(repo, &paths.raw_code, scanner.as_ref(), pii_cfg.on_hit) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: raw mirror: {e}");
            return ExitCode::from(1);
        }
    };
    let sync_elapsed_ms = sync_started.elapsed().as_millis();
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
            elapsed_ms: sync_elapsed_ms,
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

    if let Err(e) = merge_internal_gitignore(&paths.root) {
        eprintln!("error: vault internal .gitignore: {e}");
        return ExitCode::from(1);
    }
    if debug {
        eprintln!(
            "[debug] vault internal .gitignore: ensured {} required lines at {}",
            INTERNAL_GITIGNORE_LINES.len(),
            paths.root.join(".gitignore").display()
        );
        println!("✓ vault internal .gitignore: ensured");
    }

    let already_initialized = paths.root.join(".git").exists();
    if let Err(e) = init_nested_repo(&paths.root) {
        eprintln!("error: vault git init: {e}");
        return ExitCode::from(1);
    }
    if debug {
        eprintln!(
            "[debug] vault git: nested repo at {} ({})",
            paths.root.join(".git").display(),
            if already_initialized { "preserved" } else { "initialized" }
        );
        println!(
            "✓ vault git: {}",
            if already_initialized {
                "already initialized"
            } else {
                "nested repo initialized"
            }
        );
    }

    match write_schema_if_missing(&paths.schema_md) {
        Ok(true) => {
            if debug {
                eprintln!("[debug] schema: wrote {} bytes", NEUTRAL_RULES.len());
                println!("✓ schema file: wrote .codebus/CLAUDE.md");
            }
        }
        Ok(false) => {
            if debug {
                println!("✓ schema file: .codebus/CLAUDE.md already present");
            }
        }
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
        Ok(ManifestOutcome::Written) => {
            if debug {
                println!("✓ manifest: wrote .codebus/manifest.yaml");
            }
        }
        Ok(ManifestOutcome::Updated) => {
            if debug {
                println!("✓ manifest: updated sync state in .codebus/manifest.yaml");
            }
        }
        Err(e) => {
            eprintln!("error: manifest: {e}");
            return ExitCode::from(1);
        }
    }

    if let Err(e) = write_skill_bundles(&paths.root, repo, debug) {
        eprintln!("error: skill bundles: {e}");
        return ExitCode::from(1);
    }

    // v3-fix-trust-agent: write the vault-internal Claude Code settings.json
    // with the PreToolUse Bash hook for the fix sandbox. write-if-missing
    // preserves user customizations across re-init.
    match settings::write_settings_if_missing(&paths.root) {
        Ok(SettingsOutcome::Written) => {
            if debug {
                eprintln!(
                    "[debug] settings.json: wrote {}",
                    settings::settings_json_path(&paths.root).display()
                );
                println!("✓ vault settings: wrote .codebus/.claude/settings.json");
            }
        }
        Ok(SettingsOutcome::AlreadyPresent) => {
            if debug {
                eprintln!(
                    "[debug] settings.json: preserved existing {}",
                    settings::settings_json_path(&paths.root).display()
                );
                println!("✓ vault settings: .codebus/.claude/settings.json already present");
            }
        }
        Err(e) => {
            eprintln!("error: vault settings.json: {e}");
            return ExitCode::from(1);
        }
    }

    let mut obsidian_registered = false;
    if !no_obsidian_register {
        match obsidian_register::register_vault(&paths.wiki) {
            RegisterOutcome::Registered { vault_id, was_new } => {
                if debug {
                    eprintln!(
                        "[debug] obsidian: vault entry {} (id={vault_id}) for path {}",
                        if was_new { "inserted" } else { "refreshed" },
                        paths.wiki.display()
                    );
                    println!(
                        "✓ obsidian: vault {} (id={vault_id})",
                        if was_new { "registered" } else { "refreshed" }
                    );
                }
                obsidian_registered = true;
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

    let head_sha = match auto_commit(&paths.root, "init: codebus vault") {
        Ok(sha) => sha,
        Err(e) => {
            eprintln!("error: vault git auto-commit: {e}");
            return ExitCode::from(1);
        }
    };
    let sha7: String = head_sha.chars().take(7).collect();
    if debug {
        eprintln!("[debug] vault git: HEAD now {head_sha}");
        println!("✓ vault git: committed {sha7} \"init: codebus vault\"");
    }
    print_banner(Banner::CommitDone { sha7: &sha7 }, render_opts);

    // v3-config: write starter `~/.codebus/config.yaml` if missing. Failure
    // is non-fatal — the rest of init succeeded and the per-vault state is
    // usable; the user just won't have a discoverable global config file.
    write_global_config_starter(debug);

    print_banner(Banner::Done { wiki_path: &paths.wiki }, render_opts);
    if obsidian_registered {
        print_banner(Banner::Hint { wiki_path: &paths.wiki }, render_opts);
    }
    ExitCode::SUCCESS
}

/// Convert raw byte count to MiB (1024-based) for banner display.
fn bytes_to_mib(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0)
}

/// Map [`PiiScannerKind`] to its human-readable label for the PiiSummary banner.
fn pii_scanner_label(kind: &PiiScannerKind) -> &'static str {
    match kind {
        PiiScannerKind::RegexBasic => "regex_basic",
        PiiScannerKind::Null => "none",
    }
}

/// Map [`OnHit`] to the per-severity dispatch label rendered in the
/// PiiSummary banner. v3-pii-severity-dispatch: Critical-severity matches
/// are ALWAYS masked (security floor); the user-configured `OnHit` only
/// governs Warn-severity. The banner exposes both so the user can see
/// the per-severity outcome at a glance.
fn on_hit_label(action: OnHit) -> &'static str {
    match action {
        OnHit::Warn => "critical=mask, warn=warn",
        OnHit::Skip => "critical=mask, warn=skip",
        OnHit::Mask => "critical=mask, warn=mask",
    }
}

/// Load `pii.*` config from the default path, surfacing parse errors as a
/// stderr warning prefixed with `warning: pii config` and falling back to
/// `PiiConfig::default()` so init does not abort on a malformed config.
fn load_pii_config_with_warning() -> PiiConfig {
    let path = match default_config_path() {
        Some(p) => p,
        None => return PiiConfig::default(),
    };
    match load_pii_config(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("warning: pii config load failed (using defaults): {e}");
            PiiConfig::default()
        }
    }
}

/// Construct the active PII scanner per the `pii.scanner` discriminator.
/// On `RegexBasic` with bad `patterns_extra` regex, emits a stderr warning
/// and falls back to the built-in pattern set only (no NullScanner — built-in
/// 4 patterns are still useful even when user regex is malformed).
fn build_pii_scanner(cfg: &PiiConfig) -> Box<dyn PiiScanner> {
    match cfg.scanner {
        PiiScannerKind::Null => Box::new(NullScanner::new()),
        PiiScannerKind::RegexBasic => match RegexBasicScanner::new(&cfg.patterns_extra) {
            Ok(s) => Box::new(s),
            Err(e) => {
                eprintln!(
                    "warning: pii config patterns_extra failed to compile (using built-in patterns only): {e}"
                );
                Box::new(
                    RegexBasicScanner::new(&[])
                        .expect("built-in patterns must compile"),
                )
            }
        },
    }
}

/// Write the global config starter to `~/.codebus/config.yaml` if missing.
/// Emits one progress line on success (Written / AlreadyPresent) or a
/// `warning: global config` stderr message on failure. Non-fatal — caller
/// continues regardless.
fn write_global_config_starter(debug: bool) {
    let path = match default_config_path() {
        Some(p) => p,
        None => {
            if debug {
                eprintln!("[debug] global config: home dir unavailable, skipping starter write");
            }
            return;
        }
    };
    let display = path.display();
    match write_starter_config_if_missing(&path) {
        Ok(StarterOutcome::Written) => {
            if debug {
                eprintln!("[debug] global config: wrote starter at {display}");
                println!("✓ global config: wrote {display}");
            }
        }
        Ok(StarterOutcome::AlreadyPresent) => {
            if debug {
                eprintln!("[debug] global config: preserved existing {display}");
                println!("✓ global config: {display} already present");
            }
        }
        Err(e) => {
            eprintln!("warning: global config write at {display} failed (non-fatal): {e}");
        }
    }
}

/// Ensure `.codebus/.gitignore` contains every entry in [`INTERNAL_GITIGNORE_LINES`],
/// preserving any existing content (including user-added lines). Missing
/// required lines are appended in declared order.
fn merge_internal_gitignore(vault_root: &Path) -> std::io::Result<()> {
    let path = vault_root.join(".gitignore");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let present: std::collections::HashSet<&str> =
        existing.lines().map(str::trim).collect();
    let missing: Vec<&&str> = INTERNAL_GITIGNORE_LINES
        .iter()
        .filter(|l| !present.contains(*l as &str))
        .collect();

    // Fast path: file exists and already has every required line — no write.
    if missing.is_empty() && !existing.is_empty() {
        return Ok(());
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for line in missing {
        out.push_str(line);
        out.push('\n');
    }
    fs::write(&path, out)?;
    Ok(())
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

fn write_skill_bundles(vault_root: &Path, repo_root: &Path, debug: bool) -> std::io::Result<()> {
    let outcomes = skill_bundle::write_bundles_if_missing(vault_root, repo_root)?;
    let written = outcomes.iter().filter(|o| **o == BundleOutcome::Written).count();
    let preserved = outcomes
        .iter()
        .filter(|o| **o == BundleOutcome::AlreadyPresent)
        .count();
    if debug {
        for verb in skill_bundle::VERBS {
            let vp = skill_bundle::skill_bundle_path(vault_root, verb);
            let rp = skill_bundle::skill_bundle_path(repo_root, verb);
            eprintln!("[debug] skill bundle vault target: {}", vp.display());
            eprintln!("[debug] skill bundle repo  target: {}", rp.display());
        }
        println!(
            "✓ skill bundles: {} written, {} already present (across vault and repo locations)",
            written, preserved
        );
    }
    Ok(())
}
