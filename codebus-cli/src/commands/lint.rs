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
use codebus_core::wiki::lint::{format_json, format_text, lint_wiki, locate_vault_root, LocateError};
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

pub async fn run(repo_override: Option<&Path>, args: LintArgs, debug: bool) -> ExitCode {
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

    let result = lint_wiki(&vault_root);

    match args.format {
        OutputFormat::Text => {
            print!("{}", format_text(&result));
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
