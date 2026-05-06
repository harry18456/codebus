//! Top-level CLI entry. Mirrors TS `cli.ts` argv contract:
//! `codebus --repo <path> [--goal <text> | --query <text> | --check]`.

use clap::Parser;
use codebus_core::config::{EmojiMode, GlobalConfig, load_config};
use codebus_core::llm::{ProviderConfig, build_provider};
use codebus_core::log::sinks::null_sink::NullSink;
use codebus_core::render::{Banner, RenderOptions, TerminalRenderer};
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use std::path::PathBuf;
use std::process::ExitCode;

mod commands;

use commands::{check, goal, init, query};

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

    let cfg = load_config();
    let render_opts = resolve_render_opts(&cli, &cfg);

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

    runtime.block_on(async move { dispatch(cli, repo, render_opts, cfg).await })
}

fn resolve_render_opts(cli: &Cli, cfg: &GlobalConfig) -> RenderOptions {
    // 5-level emoji priority (highest → lowest):
    //   1. --emoji on|off  (explicit CLI flag)
    //   2. --no-emoji      (sugar for --emoji off)
    //   3. NO_EMOJI env    (community standard)
    //   4. config.yaml `emoji:`
    //   5. auto-detect (TTY heuristic)
    let use_emoji = match cli.emoji.as_deref() {
        Some("on") => true,
        Some("off") => false,
        _ if cli.no_emoji => false,
        _ if std::env::var_os("NO_EMOJI").is_some() => false,
        _ => match cfg.emoji {
            Some(EmojiMode::On) => true,
            Some(EmojiMode::Off) => false,
            Some(EmojiMode::Auto) | None => auto_detect_emoji(),
        },
    };
    let use_color = is_color_capable();
    RenderOptions {
        use_emoji,
        use_color,
    }
}

fn auto_detect_emoji() -> bool {
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

async fn dispatch(
    cli: Cli,
    repo: PathBuf,
    render_opts: RenderOptions,
    cfg: GlobalConfig,
) -> ExitCode {
    if cli.check {
        return run_check_cmd(&repo, render_opts).await;
    }

    if cli.goal.is_none() && cli.query.is_none() {
        return run_init_cmd(&repo, render_opts).await;
    }

    if let Some(g) = cli.goal {
        return run_goal_cmd(&repo, &g, render_opts, &cfg).await;
    }
    if let Some(q) = cli.query {
        return run_query_cmd(&repo, &q, render_opts, &cfg).await;
    }
    ExitCode::from(0)
}

/// Build a [`ProviderConfig`] from a [`GlobalConfig`]. `cfg.llm == None`
/// (section unset or unknown discriminator) yields the default config,
/// which the LLM factory maps to `claude_cli` — preserving 0.2.0 behavior
/// when no `~/.codebus/config.yaml` exists.
fn provider_config_from(cfg: &GlobalConfig) -> ProviderConfig {
    let mut pc = ProviderConfig::default();
    if let Some(llm) = &cfg.llm {
        if let Some(kind) = llm.provider {
            pc.kind = kind;
        }
        pc.binary_path = llm.binary_path.clone();
        pc.timeout_secs = llm.timeout_secs;
        pc.api_key = llm.api_key.clone();
    }
    pc
}

async fn run_check_cmd(repo: &PathBuf, render_opts: RenderOptions) -> ExitCode {
    let result = match check::run_check(repo) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_lint_report(&result);
    if result.error_count > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

async fn run_init_cmd(repo: &PathBuf, render_opts: RenderOptions) -> ExitCode {
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
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

async fn run_goal_cmd(
    repo: &PathBuf,
    goal_text: &str,
    render_opts: RenderOptions,
    cfg: &GlobalConfig,
) -> ExitCode {
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
    renderer.render_banner(&Banner::Goal { goal: goal_text });

    let provider = match build_provider(provider_config_from(cfg)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut log_sink = NullSink::new();
    let result = match goal::run_goal(
        goal::RunGoalOptions {
            repo_root: repo,
            goal: goal_text,
            provider: provider.as_ref(),
        },
        &mut renderer,
        &mut log_sink,
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
        renderer.render_banner(&Banner::Done {
            wiki_path: &wiki_path,
        });
        if let Some(lint) = &result.lint {
            renderer.render_lint_summary(lint);
        }
        renderer.render_banner(&Banner::Hint { path: &wiki_path });
    } else {
        let shrug = if render_opts.use_emoji { "🤷" } else { "~" };
        println!("{shrug} Agent 跑完但沒動 wiki — 可能此 goal 不適合（agent 自我判斷拒絕）");
        println!("   raw 已 sync、goals.jsonl 已記錄；wiki 內容無變化");
    }
    ExitCode::from(0)
}

async fn run_query_cmd(
    repo: &PathBuf,
    query_text: &str,
    render_opts: RenderOptions,
    cfg: &GlobalConfig,
) -> ExitCode {
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
    let provider = match build_provider(provider_config_from(cfg)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut log_sink = NullSink::new();
    if let Err(e) = query::run_query(
        query::RunQueryOptions {
            repo_root: repo,
            query: query_text,
            provider: provider.as_ref(),
        },
        &mut renderer,
        &mut log_sink,
    )
    .await
    {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}
