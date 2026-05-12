//! `codebus query "..."` — CLI thin wrapper over
//! `codebus_core::verb::query::run_query`.
//!
//! After v3-goal-library, all read-only query orchestration lives in
//! `codebus_core::verb::query`. This file is just the clap argument
//! surface plus the `VerbEvent` → terminal renderer dispatch closure
//! plus the `VerbError` → exit-code / stderr translation table.

use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use codebus_core::config::ConfigLoadError;
use codebus_core::render::{RenderOptions, print_banner, print_event};
use codebus_core::verb::query::{QueryOptions, run_query};
use codebus_core::verb::{VerbError, VerbEvent, VerbLifecycleEvent};

#[derive(Args, Debug)]
pub struct QueryArgs {
    /// What you want to know about the codebase.
    #[arg(value_name = "QUERY")]
    pub text: String,
}

pub async fn run(
    repo: &Path,
    args: QueryArgs,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    if debug {
        eprintln!("[debug] query: repo={}", repo.display());
    }

    let options = QueryOptions {
        text: args.text.clone(),
    };

    let render_opts_for_closure = render_opts.clone();
    let on_event = move |event: VerbEvent| match event {
        VerbEvent::Banner(b) => print_banner(b.as_banner(), &render_opts_for_closure),
        VerbEvent::Stream(s) => print_event(&s, &render_opts_for_closure),
        VerbEvent::Lifecycle(VerbLifecycleEvent::SpawnEnd { exit_code, .. }) => {
            if debug {
                eprintln!(
                    "[debug] query: agent exited code={}",
                    exit_code.unwrap_or(-1)
                );
            }
        }
        VerbEvent::Lifecycle(_) => {}
    };

    match run_query(repo, options, on_event, None) {
        Ok(report) => {
            let exit: u8 = report
                .agent_exit_code
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(0);
            ExitCode::from(exit)
        }
        Err(e) => translate_error("query", &e, render_opts),
    }
}

fn translate_error(verb: &str, err: &VerbError, _render_opts: &RenderOptions) -> ExitCode {
    match err {
        VerbError::VaultMissing { path } => {
            eprintln!(
                "error: {verb}: vault not found at {}; run `codebus init` first",
                path.display()
            );
            ExitCode::from(2)
        }
        VerbError::ConfigParse { which, source } => {
            // Match pre-refactor stderr verbatim: section-specific
            // message with the config path.
            let path_disp = codebus_core::config::default_config_path()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            match source {
                ConfigLoadError::Io(_) | ConfigLoadError::YamlParse(_) => {
                    eprintln!("error: {which} config parse failed at {path_disp}: {source}");
                }
            }
            ExitCode::from(2)
        }
        VerbError::KeyringMissing { source } => {
            eprintln!("error: {verb}: {source}");
            ExitCode::from(3)
        }
        VerbError::Spawn { source } => {
            eprintln!("error: spawn claude: {source}");
            ExitCode::from(1)
        }
        VerbError::Cancelled => ExitCode::from(0),
        VerbError::Internal { message } => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
