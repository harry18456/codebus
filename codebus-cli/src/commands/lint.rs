//! `codebus lint` subcommand — validate vault wiki/ structure.
//!
//! v3-lint Lint Subcommand Behavior:
//! - `--repo <PATH>` (optional, overrides cwd-based vault auto-detection)
//! - `--format <text|json>` (default text)
//! - `--debug` global flag (inherited)
//!
//! Exit codes:
//! - 0: zero errors (warnings allowed)
//! - 1: one or more errors
//! - 2: no vault locatable

use clap::{Args, ValueEnum};
use codebus_core::render::{RenderOptions, format_lint_text};
use codebus_core::vault::layout::vault_paths;
use codebus_core::vault::obsidian_register::lookup_vault_id;
use codebus_core::wiki::lint::{
    LocateError, format_json, format_text, lint_wiki, locate_vault_root,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(Args, Debug)]
pub struct LintArgs {
    /// Output format: human-readable text (default) or machine-readable JSON.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
}

pub async fn run(
    repo_override: Option<&Path>,
    args: LintArgs,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: failed to read current directory: {e}");
            return ExitCode::from(2);
        }
    };

    let vault_root = match locate_vault_root(&cwd, repo_override) {
        Ok(p) => p,
        Err(LocateError::NoVaultFound) => {
            eprintln!(
                "error: no codebus vault found at cwd or under .codebus/ — run `codebus init` first"
            );
            return ExitCode::from(2);
        }
    };

    if debug {
        println!("[debug] lint: vault_root = {}", vault_root.display());
    }

    // For text format only: re-derive RenderOptions to enrich `vault_id`
    // from Obsidian config so the OSC 8 hyperlink emitter can build
    // `obsidian://open?vault=<id>&file=<rel>` URLs. JSON format intentionally
    // skips this — `format_json` MUST stay machine-readable.
    let wiki_root = vault_paths(vault_root.parent().unwrap_or(Path::new(""))).wiki;
    let result = lint_wiki(&vault_root);

    match args.format {
        OutputFormat::Text => {
            let lint_opts = if render_opts.use_hyperlinks {
                let vault_id = lookup_vault_id(&wiki_root).ok().flatten();
                RenderOptions::explicit(
                    render_opts.use_emoji,
                    render_opts.use_color,
                    render_opts.use_hyperlinks,
                    vault_id,
                )
            } else {
                render_opts.clone()
            };
            // Use the styled formatter directly to pick up the lint-specific
            // vault_id; the legacy `format_text` wrapper would lose it.
            let _ = format_text; // keep the import live for backwards-compat callers
            print!("{}", format_lint_text(&result, &lint_opts, &wiki_root));
        }
        OutputFormat::Json => match format_json(&result, &vault_root) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("error: failed to serialize lint result as JSON: {e}");
                return ExitCode::from(2);
            }
        },
    }

    if result.error_count > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

// Reference vault_root path consumer to avoid unused-import warnings while
// keeping the type readable in signatures.
#[allow(dead_code)]
fn _unused(_p: PathBuf) {}
