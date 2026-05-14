//! Shared `codebus init` orchestration used by both the CLI verb and the
//! Tauri app's `add_vault` flow.
//!
//! The function intentionally emits no terminal output — callers receive
//! [`InitEvent`] notifications via a closure and decide whether to render
//! banners, debug lines, or stay silent. This lets the CLI keep its
//! interleaved banner / debug print order intact while the Tauri backend
//! runs the same flow with zero stdout/stderr noise.
//!
//! `run_init` performs all the steps that `codebus init` used to inline:
//! sanity check, vault layout, source `.gitignore` mutation, PII-aware raw
//! mirror sync, internal `.gitignore`, nested git repo init, schema md,
//! manifest, skill bundles, vault settings.json, optional Obsidian register,
//! commit, optional global `~/.codebus/config.yaml` starter write.
//!
//! Behavioral contract:
//! - Sanity-check fail surfaces as [`InitError::Refused`].
//! - PII config load / regex compile failures are non-fatal: emitted as
//!   `InitEvent::PiiConfigLoadWarn` / `PiiPatternsExtraWarn` (callers MAY
//!   surface them to stderr) and the orchestration proceeds with safe
//!   defaults.
//! - Obsidian register failures are non-fatal (same shape as the legacy
//!   CLI; the caller's closure decides whether to print a hint).
//! - Global config starter failure is non-fatal (matches the legacy CLI
//!   "warning: global config write at … failed (non-fatal)" behavior).
//! - Every other step (layout, raw_sync, internal gitignore, nested repo,
//!   schema, manifest, skill bundles, settings, commit) is fatal and short
//!   circuits with a typed [`InitError`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config::{
    PiiConfig, PiiScannerKind, StarterOutcome, default_config_path, load_pii_config,
    write_starter_config_if_missing,
};
use crate::git::{auto_commit, init_nested_repo};
use crate::pii::PiiScanner;
use crate::pii::provider::OnHit;
use crate::pii::scanners::null_scanner::NullScanner;
use crate::pii::scanners::regex_basic::RegexBasicScanner;
use crate::schema::NEUTRAL_RULES;
use crate::skill_bundle::{self, BundleOutcome};
use crate::vault::layout::{VaultPaths, create_vault_layout};
use crate::vault::manifest::{self, ManifestOutcome, SourceSignal};
use crate::vault::nav_stubs;
use crate::vault::obsidian_register::{self, RegisterOutcome};
use crate::vault::raw_sync::{SyncSummary, sync_with_scanner};
use crate::vault::sanity_check::{VaultRefusal, check_repo_is_not_vault};
use crate::vault::settings::{self, SettingsOutcome};
use crate::vault::source_gitignore::{self, GitignoreOutcome};

/// Lines that MUST appear in `.codebus/.gitignore`. Moved here from the
/// legacy CLI `commands/init.rs`; the rationale comment is preserved
/// verbatim because the choices encode prior incidents.
const INTERNAL_GITIGNORE_LINES: &[&str] = &[
    // `.lock` is per-process file lock state.
    ".lock",
    // `raw/code/` is already tracked via source repo's git so duplicate-
    // tracking it here would noise every commit.
    "raw/code/",
    // Editor-local config that the user shouldn't see in vault diffs.
    "**/.obsidian/",
    // v3-run-log: align with `vault::layout` which creates `log/` (singular),
    // not the `logs/` plural the prior line had — that line never matched
    // anything on disk and let runs-*.jsonl files dirty the working tree.
    "log/",
    // Personal Claude Code overrides (Claude Code convention).
    ".claude/settings.local.json",
];

/// Caller-tunable options.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// When true, skip `obsidian_register::register_vault`. CLI surfaces
    /// this via the `--no-obsidian-register` flag.
    pub no_obsidian_register: bool,
    /// When true, run `write_starter_config_if_missing` at the very end so
    /// new users get a documented `~/.codebus/config.yaml`. CLI and app
    /// both want this; tests may want to skip.
    pub write_starter_config: bool,
    /// When true, ALSO materialize skill bundles at the repo-root location
    /// (`<repo>/.claude/skills/codebus-*/`) AND add the corresponding
    /// `.gitignore` patterns. The vault-internal location is always
    /// written; this flag only toggles the secondary copy.
    ///
    /// Defaults to `false` because the GUI and CLI default spawn paths
    /// both run agents with cwd = `.codebus/`, so the repo-root copy is
    /// only useful when a user opens a raw Claude Code session at the
    /// source repository root and invokes `/codebus-<verb>` interactively
    /// — a power-user workflow distinct from the default flows.
    pub with_repo_root_skills: bool,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            no_obsidian_register: false,
            write_starter_config: true,
            with_repo_root_skills: false,
        }
    }
}

/// Result of a successful init run.
#[derive(Debug, Clone)]
pub struct InitOutcome {
    pub paths: VaultPaths,
    /// Commit SHA produced by `auto_commit` at the end of init.
    pub head_sha: String,
    /// True iff Obsidian's known-vaults list now contains this vault.
    pub obsidian_registered: bool,
}

/// Typed init failure. Each variant maps 1:1 to a step where the legacy
/// CLI used to `eprintln!("error: …"); return ExitCode::from(N);`.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("path appears to be (or sit inside) a codebus vault: {0}")]
    Refused(VaultRefusal),

    #[error("create vault layout: {0}")]
    Layout(#[source] io::Error),

    #[error("source .gitignore: {0}")]
    SourceGitignore(#[source] io::Error),

    #[error("raw mirror: {0}")]
    RawSync(#[source] io::Error),

    #[error("vault internal .gitignore: {0}")]
    InternalGitignore(#[source] io::Error),

    #[error("vault git init: {0}")]
    NestedRepo(#[source] io::Error),

    #[error("schema file: {0}")]
    Schema(#[source] io::Error),

    #[error("manifest: {0}")]
    Manifest(#[source] io::Error),

    #[error("skill bundles: {0}")]
    SkillBundles(#[source] io::Error),

    #[error("nav stubs: {0}")]
    NavStubs(#[source] io::Error),

    #[error("vault settings.json: {0}")]
    Settings(#[source] io::Error),

    #[error("vault git auto-commit: {0}")]
    Commit(#[source] io::Error),
}

/// Progress events emitted at the same sites where the legacy CLI used to
/// `println!`, `eprintln!`, or `print_banner!`. Callers receive these
/// through the `on_event` closure passed to [`run_init`].
#[derive(Debug)]
pub enum InitEvent<'a> {
    Start {
        repo: &'a Path,
    },
    LayoutCreated {
        paths: &'a VaultPaths,
    },
    SourceGitignore {
        outcome: GitignoreOutcome,
    },
    PiiConfigLoadWarn {
        message: String,
    },
    PiiPatternsExtraWarn {
        message: String,
    },
    RawSyncDone {
        paths: &'a VaultPaths,
        repo: &'a Path,
        summary: &'a SyncSummary,
        elapsed_ms: u128,
        pii_cfg: &'a PiiConfig,
    },
    InternalGitignoreDone {
        path: PathBuf,
        required_count: usize,
    },
    NestedRepoDone {
        vault_root: &'a Path,
        already_initialized: bool,
    },
    SchemaDone {
        path: &'a Path,
        wrote_new: bool,
    },
    ManifestSignal {
        repo: &'a Path,
        signal: &'a SourceSignal,
    },
    ManifestDone {
        outcome: ManifestOutcome,
    },
    SkillBundlesDone {
        vault_root: &'a Path,
        repo: &'a Path,
        written: usize,
        preserved: usize,
    },
    NavStubsDone {
        vault_root: &'a Path,
        written: usize,
        preserved: usize,
    },
    SettingsDone {
        vault_root: &'a Path,
        outcome: SettingsOutcome,
    },
    ObsidianResult {
        wiki_path: &'a Path,
        outcome: RegisterOutcome,
    },
    ObsidianSkipped,
    CommitDone {
        head_sha: String,
        sha7: String,
    },
    StarterConfigUnavailable,
    StarterConfigDone {
        path: PathBuf,
        outcome: StarterOutcome,
    },
    StarterConfigError {
        path: PathBuf,
        message: String,
    },
    Finished {
        paths: &'a VaultPaths,
        obsidian_registered: bool,
    },
}

/// Orchestrate the full init flow. `on_event` is invoked at every step
/// boundary where the legacy CLI would have emitted a banner or progress
/// line; pass `|_| {}` for a silent run.
///
/// Returns the per-step typed error on the first fatal step; non-fatal
/// failures (PII config load, Obsidian register, starter config write) are
/// reported via [`InitEvent`] and the orchestration continues.
pub fn run_init(
    repo: &Path,
    opts: &InitOptions,
    mut on_event: impl FnMut(InitEvent<'_>),
) -> Result<InitOutcome, InitError> {
    check_repo_is_not_vault(repo).map_err(InitError::Refused)?;

    on_event(InitEvent::Start { repo });

    let paths = create_vault_layout(repo).map_err(InitError::Layout)?;
    on_event(InitEvent::LayoutCreated { paths: &paths });

    let gitignore_outcome = source_gitignore::ensure_codebus_in_gitignore(
        repo,
        opts.with_repo_root_skills,
    )
    .map_err(InitError::SourceGitignore)?;
    on_event(InitEvent::SourceGitignore {
        outcome: gitignore_outcome,
    });

    let pii_cfg = load_pii_with_warn(&mut on_event);
    let scanner = build_scanner(&pii_cfg, &mut on_event);

    let sync_started = Instant::now();
    let summary = sync_with_scanner(repo, &paths.raw_code, scanner.as_ref(), pii_cfg.on_hit)
        .map_err(InitError::RawSync)?;
    let elapsed_ms = sync_started.elapsed().as_millis();
    on_event(InitEvent::RawSyncDone {
        paths: &paths,
        repo,
        summary: &summary,
        elapsed_ms,
        pii_cfg: &pii_cfg,
    });

    merge_internal_gitignore(&paths.root).map_err(InitError::InternalGitignore)?;
    on_event(InitEvent::InternalGitignoreDone {
        path: paths.root.join(".gitignore"),
        required_count: INTERNAL_GITIGNORE_LINES.len(),
    });

    let already_initialized = paths.root.join(".git").exists();
    init_nested_repo(&paths.root).map_err(InitError::NestedRepo)?;
    on_event(InitEvent::NestedRepoDone {
        vault_root: &paths.root,
        already_initialized,
    });

    let wrote_schema = write_schema_if_missing(&paths.schema_md).map_err(InitError::Schema)?;
    on_event(InitEvent::SchemaDone {
        path: &paths.schema_md,
        wrote_new: wrote_schema,
    });

    let signal = manifest::compute_source_signal(repo, &summary);
    on_event(InitEvent::ManifestSignal {
        repo,
        signal: &signal,
    });
    let manifest_outcome =
        manifest::write_or_update_manifest(repo, &paths.root, env!("CARGO_PKG_VERSION"), signal)
            .map_err(InitError::Manifest)?;
    on_event(InitEvent::ManifestDone {
        outcome: manifest_outcome,
    });

    let (written, preserved) = write_skill_bundles(
        &paths.root,
        repo,
        opts.with_repo_root_skills,
    )
    .map_err(InitError::SkillBundles)?;
    let today_utc = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (nav_written, nav_preserved) =
        nav_stubs::write_nav_stubs_if_missing(&paths.root, &today_utc)
            .map_err(InitError::NavStubs)?;
    on_event(InitEvent::NavStubsDone {
        vault_root: &paths.root,
        written: nav_written,
        preserved: nav_preserved,
    });
    on_event(InitEvent::SkillBundlesDone {
        vault_root: &paths.root,
        repo,
        written,
        preserved,
    });

    let settings_outcome =
        settings::write_settings_if_missing(&paths.root).map_err(InitError::Settings)?;
    on_event(InitEvent::SettingsDone {
        vault_root: &paths.root,
        outcome: settings_outcome,
    });

    let obsidian_registered;
    if opts.no_obsidian_register {
        obsidian_registered = false;
        on_event(InitEvent::ObsidianSkipped);
    } else {
        let outcome = obsidian_register::register_vault(&paths.wiki);
        obsidian_registered = matches!(outcome, RegisterOutcome::Registered { .. });
        on_event(InitEvent::ObsidianResult {
            wiki_path: &paths.wiki,
            outcome,
        });
    }

    let head_sha = auto_commit(&paths.root, "init: codebus vault").map_err(InitError::Commit)?;
    let sha7: String = head_sha.chars().take(7).collect();
    on_event(InitEvent::CommitDone {
        head_sha: head_sha.clone(),
        sha7,
    });

    if opts.write_starter_config {
        match default_config_path() {
            None => on_event(InitEvent::StarterConfigUnavailable),
            Some(path) => match write_starter_config_if_missing(&path) {
                Ok(outcome) => on_event(InitEvent::StarterConfigDone {
                    path: path.clone(),
                    outcome,
                }),
                Err(e) => on_event(InitEvent::StarterConfigError {
                    path: path.clone(),
                    message: e.to_string(),
                }),
            },
        }
    }

    on_event(InitEvent::Finished {
        paths: &paths,
        obsidian_registered,
    });

    Ok(InitOutcome {
        paths,
        head_sha,
        obsidian_registered,
    })
}

fn load_pii_with_warn(on_event: &mut impl FnMut(InitEvent<'_>)) -> PiiConfig {
    let path = match default_config_path() {
        Some(p) => p,
        None => return PiiConfig::default(),
    };
    match load_pii_config(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            on_event(InitEvent::PiiConfigLoadWarn {
                message: e.to_string(),
            });
            PiiConfig::default()
        }
    }
}

fn build_scanner(cfg: &PiiConfig, on_event: &mut impl FnMut(InitEvent<'_>)) -> Box<dyn PiiScanner> {
    match cfg.scanner {
        PiiScannerKind::Null => Box::new(NullScanner::new()),
        PiiScannerKind::RegexBasic => match RegexBasicScanner::new(&cfg.patterns_extra) {
            Ok(s) => Box::new(s),
            Err(e) => {
                on_event(InitEvent::PiiPatternsExtraWarn {
                    message: e.to_string(),
                });
                Box::new(RegexBasicScanner::new(&[]).expect("built-in patterns must compile"))
            }
        },
    }
}

/// Ensure `.codebus/.gitignore` contains every entry in
/// [`INTERNAL_GITIGNORE_LINES`], preserving any existing content
/// (including user-added lines).
fn merge_internal_gitignore(vault_root: &Path) -> io::Result<()> {
    let path = vault_root.join(".gitignore");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };

    let present: std::collections::HashSet<&str> = existing.lines().map(str::trim).collect();
    let missing: Vec<&&str> = INTERNAL_GITIGNORE_LINES
        .iter()
        .filter(|l| !present.contains(*l as &str))
        .collect();

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

fn write_schema_if_missing(schema_md: &Path) -> io::Result<bool> {
    if schema_md.exists() {
        return Ok(false);
    }
    if let Some(parent) = schema_md.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(schema_md, NEUTRAL_RULES)?;
    Ok(true)
}

fn write_skill_bundles(
    vault_root: &Path,
    repo_root: &Path,
    write_repo_root: bool,
) -> io::Result<(usize, usize)> {
    let outcomes =
        skill_bundle::write_bundles_if_missing(vault_root, repo_root, write_repo_root)?;
    let written = outcomes
        .iter()
        .filter(|o| **o == BundleOutcome::Written)
        .count();
    let preserved = outcomes
        .iter()
        .filter(|o| **o == BundleOutcome::AlreadyPresent)
        .count();
    Ok((written, preserved))
}

/// Convert raw byte count to MiB (1024-based). Re-exposed for the CLI's
/// SyncDone banner formatting since the banner uses MiB units.
pub fn bytes_to_mib(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0)
}

/// Human-readable scanner label for the PiiSummary banner.
pub fn pii_scanner_label(kind: &PiiScannerKind) -> &'static str {
    match kind {
        PiiScannerKind::RegexBasic => "regex_basic",
        PiiScannerKind::Null => "none",
    }
}

/// Human-readable per-severity action label for the PiiSummary banner.
pub fn on_hit_label(action: OnHit) -> &'static str {
    match action {
        OnHit::Warn => "critical=mask, warn=warn",
        OnHit::Skip => "critical=mask, warn=skip",
        OnHit::Mask => "critical=mask, warn=mask",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Spec scenario (skill-bundles "Init default creates only
    /// vault-internal skill bundles" — InitOptions side): `Default`
    /// MUST disable repo-root skill bundles so the GUI / CLI default
    /// spawn paths get vault-only by construction.
    #[test]
    fn init_options_default_disables_repo_root_skills() {
        let opts = InitOptions::default();
        assert!(
            !opts.with_repo_root_skills,
            "InitOptions::default().with_repo_root_skills must be false"
        );
    }

    /// Smoke: silent mode produces a usable vault layout and commit.
    /// Cross-platform git availability is assumed (matches existing
    /// codebus-core tests).
    #[test]
    fn silent_run_creates_vault_artifacts() {
        let tmp = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();

        unsafe { std::env::set_var("CODEBUS_HOME", home.path()) };
        let opts = InitOptions {
            no_obsidian_register: true,
            write_starter_config: false,
            with_repo_root_skills: false,
        };
        let outcome =
            run_init(tmp.path(), &opts, |_event| {}).expect("init should succeed in tempdir");

        // Layout is present.
        assert!(outcome.paths.root.is_dir());
        assert!(outcome.paths.wiki.is_dir());
        assert!(outcome.paths.raw_code.is_dir());
        // Nested git initialized + committed.
        assert!(outcome.paths.root.join(".git").exists());
        assert!(!outcome.head_sha.is_empty());
        // Obsidian skipped per opts.
        assert!(!outcome.obsidian_registered);

        unsafe { std::env::remove_var("CODEBUS_HOME") };
    }

    /// Refusal on a vault root returns `InitError::Refused`, not a generic
    /// IO error.
    #[test]
    fn refuses_to_init_inside_existing_vault() {
        let tmp = TempDir::new().unwrap();
        // Plant the markers `check_repo_is_not_vault` looks for.
        std::fs::create_dir_all(tmp.path().join("wiki")).unwrap();
        std::fs::write(tmp.path().join("manifest.yaml"), "").unwrap();

        let opts = InitOptions {
            no_obsidian_register: true,
            write_starter_config: false,
            with_repo_root_skills: false,
        };
        let err = run_init(tmp.path(), &opts, |_| {}).expect_err("expected refusal");
        assert!(matches!(err, InitError::Refused(_)));
    }

    #[test]
    fn events_are_emitted_in_declared_order() {
        let tmp = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        unsafe { std::env::set_var("CODEBUS_HOME", home.path()) };

        let mut order: Vec<&'static str> = Vec::new();
        let opts = InitOptions {
            no_obsidian_register: true,
            write_starter_config: false,
            with_repo_root_skills: false,
        };
        run_init(tmp.path(), &opts, |event| {
            order.push(event_label(&event));
        })
        .unwrap();

        // The first event MUST be Start and the last MUST be Finished.
        assert_eq!(order.first().copied(), Some("Start"));
        assert_eq!(order.last().copied(), Some("Finished"));
        // Layout must come before raw-sync.
        let layout_idx = order.iter().position(|e| *e == "LayoutCreated").unwrap();
        let sync_idx = order.iter().position(|e| *e == "RawSyncDone").unwrap();
        assert!(layout_idx < sync_idx);

        unsafe { std::env::remove_var("CODEBUS_HOME") };
    }

    fn event_label(event: &InitEvent<'_>) -> &'static str {
        match event {
            InitEvent::Start { .. } => "Start",
            InitEvent::LayoutCreated { .. } => "LayoutCreated",
            InitEvent::SourceGitignore { .. } => "SourceGitignore",
            InitEvent::PiiConfigLoadWarn { .. } => "PiiConfigLoadWarn",
            InitEvent::PiiPatternsExtraWarn { .. } => "PiiPatternsExtraWarn",
            InitEvent::RawSyncDone { .. } => "RawSyncDone",
            InitEvent::InternalGitignoreDone { .. } => "InternalGitignoreDone",
            InitEvent::NestedRepoDone { .. } => "NestedRepoDone",
            InitEvent::SchemaDone { .. } => "SchemaDone",
            InitEvent::ManifestSignal { .. } => "ManifestSignal",
            InitEvent::ManifestDone { .. } => "ManifestDone",
            InitEvent::SkillBundlesDone { .. } => "SkillBundlesDone",
            InitEvent::NavStubsDone { .. } => "NavStubsDone",
            InitEvent::SettingsDone { .. } => "SettingsDone",
            InitEvent::ObsidianResult { .. } => "ObsidianResult",
            InitEvent::ObsidianSkipped => "ObsidianSkipped",
            InitEvent::CommitDone { .. } => "CommitDone",
            InitEvent::StarterConfigUnavailable => "StarterConfigUnavailable",
            InitEvent::StarterConfigDone { .. } => "StarterConfigDone",
            InitEvent::StarterConfigError { .. } => "StarterConfigError",
            InitEvent::Finished { .. } => "Finished",
        }
    }
}
