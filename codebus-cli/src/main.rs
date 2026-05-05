//! Top-level CLI entry. Mirrors TS `cli.ts` argv contract:
//! `codebus --repo <path> [--goal <text> | --query <text> | --check]`.

use clap::Parser;
use codebus_core::llm::claude_cli::ClaudeCliProvider;
use codebus_core::stream::StreamEvent;
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use std::path::PathBuf;
use std::process::ExitCode;

mod commands;
mod ui;

use commands::{check, goal, init, query};
use ui::{Banner, RenderOptions, print_lint_report, render_banner, render_event};

#[derive(Parser, Debug)]
#[command(
    name = "codebus",
    version,
    about = "Build an LLM wiki for any codebase via claude -p"
)]
struct Cli {
    /// Repo path (default: current directory).
    #[arg(long = "repo")]
    repo: Option<PathBuf>,

    /// Build wiki for this goal.
    #[arg(long)]
    goal: Option<String>,

    /// Ask the wiki a question.
    #[arg(long)]
    query: Option<String>,

    /// Lint vault wiki/ for Obsidian compatibility (read-only).
    #[arg(long)]
    check: bool,

    /// Verbose stream-json output (debug aid).
    #[arg(long)]
    debug: bool,

    /// emoji mode: auto | on | off
    #[arg(long)]
    emoji: Option<String>,

    /// sugar for --emoji off
    #[arg(long = "no-emoji")]
    no_emoji: bool,
}

fn main() -> ExitCode {
    // Iter-8 lesson Rust equivalent: read `repo` BEFORE installing any
    // signal handler that depends on it. Rust signal handling via Tokio
    // is owned by the runtime, not unscoped like Node's `process.on`.
    let cli = Cli::parse();
    let repo = cli
        .repo
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));

    let sanity = check_repo_is_not_vault(&repo);
    if !sanity.ok {
        if let Some(reason) = sanity.reason {
            eprintln!("error: {reason}");
        }
        if let Some(hint) = sanity.hint {
            eprintln!("hint: {hint}");
        }
        return ExitCode::from(2);
    }

    let render_opts = resolve_render_opts(&cli);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: failed to start tokio runtime: {e}");
            return ExitCode::from(1);
        }
    };

    runtime.block_on(async move { dispatch(cli, repo, render_opts).await })
}

fn resolve_render_opts(cli: &Cli) -> RenderOptions {
    // Settings priority: --no-emoji wins over --emoji unset; --emoji off
    // wins over auto. Auto: TTY + non-Windows-cmd → emoji on. For phase 1
    // we keep it simple: explicit on/off, fall back to auto = on.
    let use_emoji = if cli.no_emoji {
        false
    } else {
        match cli.emoji.as_deref() {
            Some("off") => false,
            Some("on") => true,
            _ => is_term_emoji_capable(),
        }
    };
    let use_color = is_color_capable();
    RenderOptions {
        use_emoji,
        use_color,
    }
}

fn is_term_emoji_capable() -> bool {
    if std::env::var_os("NO_EMOJI").is_some() {
        return false;
    }
    // Conservative auto: on for non-Windows TTYs, off for Windows cmd.
    #[cfg(windows)]
    {
        false
    }
    #[cfg(not(windows))]
    {
        atty_like_check()
    }
}

#[cfg(not(windows))]
fn atty_like_check() -> bool {
    // Use std isatty check via libc on unix; absent from std, settle for
    // env heuristic: TERM != "" and not "dumb".
    std::env::var("TERM")
        .map(|t| !t.is_empty() && t != "dumb")
        .unwrap_or(false)
}

fn is_color_capable() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    // Same heuristic as emoji — color is fine when TERM is set.
    std::env::var("TERM")
        .map(|t| !t.is_empty() && t != "dumb")
        .unwrap_or(false)
}

async fn dispatch(cli: Cli, repo: PathBuf, render_opts: RenderOptions) -> ExitCode {
    if cli.check {
        return run_check_cmd(&repo, render_opts).await;
    }

    if cli.goal.is_none() && cli.query.is_none() {
        return run_init_cmd(&repo, render_opts).await;
    }

    if let Some(g) = cli.goal {
        return run_goal_cmd(&repo, &g, render_opts).await;
    }
    if let Some(q) = cli.query {
        return run_query_cmd(&repo, &q, render_opts).await;
    }
    ExitCode::from(0)
}

async fn run_check_cmd(repo: &PathBuf, render_opts: RenderOptions) -> ExitCode {
    let result = match check::run_check(repo) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    print!("{}", print_lint_report(&result, render_opts));
    if result.error_count > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

async fn run_init_cmd(repo: &PathBuf, render_opts: RenderOptions) -> ExitCode {
    println!(
        "{}",
        render_banner(
            Banner::Start {
                path: &repo.to_string_lossy()
            },
            render_opts
        )
    );
    if let Err(e) = init::run_init(repo) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    let ok = if render_opts.use_emoji { "✨" } else { "✓" };
    let tip = if render_opts.use_emoji { "💡" } else { "i" };
    let vault_path = format!("{}/.codebus", repo.display()).replace('\\', "/");
    println!("{ok} Vault 已初始化於 {vault_path}");
    println!("{tip} 下一步：codebus --goal \"<你的探索目標>\"");
    ExitCode::from(0)
}

async fn run_goal_cmd(repo: &PathBuf, goal_text: &str, render_opts: RenderOptions) -> ExitCode {
    println!(
        "{}",
        render_banner(
            Banner::Start {
                path: &repo.to_string_lossy()
            },
            render_opts
        )
    );
    println!(
        "{}",
        render_banner(Banner::Goal { goal: goal_text }, render_opts)
    );

    let provider = ClaudeCliProvider::new();
    let on_event = |e: &StreamEvent| {
        let line = render_event(e, render_opts);
        if !line.is_empty() {
            println!("{line}");
        }
    };
    let result = match goal::run_goal(
        goal::RunGoalOptions {
            repo_root: repo,
            goal: goal_text,
            provider: &provider,
        },
        on_event,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    if result.wiki_changed {
        let wiki_path = format!("{}/.codebus/wiki", repo.display());
        println!(
            "{}",
            render_banner(
                Banner::Done {
                    wiki_path: &wiki_path
                },
                render_opts
            )
        );
        if let Some(lint) = &result.lint {
            let summary = ui::format_lint_summary(lint, render_opts);
            if !summary.is_empty() {
                println!("{summary}");
            }
        }
        println!(
            "{}",
            render_banner(Banner::Hint { path: &wiki_path }, render_opts)
        );
    } else {
        let shrug = if render_opts.use_emoji { "🤷" } else { "~" };
        println!("{shrug} Agent 跑完但沒動 wiki — 可能此 goal 不適合（agent 自我判斷拒絕）");
        println!("   raw 已 sync、goals.jsonl 已記錄；wiki 內容無變化");
    }
    ExitCode::from(0)
}

async fn run_query_cmd(repo: &PathBuf, query_text: &str, render_opts: RenderOptions) -> ExitCode {
    println!(
        "{}",
        render_banner(
            Banner::Start {
                path: &repo.to_string_lossy()
            },
            render_opts
        )
    );
    let provider = ClaudeCliProvider::new();
    let on_event = |e: &StreamEvent| {
        let line = render_event(e, render_opts);
        if !line.is_empty() {
            println!("{line}");
        }
    };
    if let Err(e) = query::run_query(
        query::RunQueryOptions {
            repo_root: repo,
            query: query_text,
            provider: &provider,
        },
        on_event,
    )
    .await
    {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}
