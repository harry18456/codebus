//! Top-level CLI entry. Mirrors TS `cli.ts` argv contract:
//! `codebus --repo <path> [--goal <text> | --query <text> | --check]`.

use clap::Parser;
use codebus_core::config::{EmojiMode, GlobalConfig, load_config};
use codebus_core::llm::{ProviderConfig, build_provider};
use codebus_core::log::{SinkConfig, build_sink};
use codebus_core::pii::{ScannerConfig, build_scanner};
use codebus_core::render::{Banner, RenderOptions, TerminalRenderer};
use codebus_core::vault::sanity_check::check_repo_is_not_vault;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod commands;

use commands::{check, fix, goal, init, query};

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

    /// Run lint feedback loop against the vault without ingest. Targets
    /// existing vaults; errors out when `<repo>/.codebus/` doesn't exist.
    #[arg(long)]
    fix: bool,

    /// Skip the auto-fix step (overrides `lint.auto_fix.enabled`).
    #[arg(long = "no-fix")]
    no_fix: bool,

    /// Override the fix-loop max iteration count for this invocation.
    #[arg(long = "fix-max-iter")]
    fix_max_iter: Option<u32>,

    /// Verbose stream-json output (debug aid).
    #[arg(long)]
    debug: bool,

    /// emoji mode: auto | on | off
    #[arg(long)]
    emoji: Option<String>,

    /// sugar for --emoji off
    #[arg(long = "no-emoji")]
    no_emoji: bool,

    /// Skip auto-registering `.codebus/wiki/` as an Obsidian vault during init.
    /// Without this flag, codebus writes a vault entry into the user's
    /// `obsidian.json` so the wiki shows up in Obsidian's vault picker and
    /// `[[wikilink]]` OSC 8 hyperlinks resolve cleanly.
    #[arg(long = "no-obsidian-register")]
    no_obsidian_register: bool,
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
        ..RenderOptions::default()
    }
}

fn auto_detect_emoji() -> bool {
    // Cross-platform TTY detection via stdlib `IsTerminal` (Rust 1.70+).
    // Modern Windows Terminal / PowerShell 7 / VSCode integrated terminal
    // all report TTY correctly and render emoji + ANSI cleanly. The pre-
    // 2026 conservative "off on Windows" fallback was for cmd.exe + Win 7
    // era hosts; on the workspace MSRV (1.85) and Win 10 1607+ ConHost
    // those are not the common case.
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Inject vault-derived render context into `render_opts` for runs that
/// emit thought streams (goal/query/fix). Builds the slug→path index by
/// scanning the wiki and looks up the vault's effective Obsidian id from
/// the user's `obsidian.json`. Both lookups are best-effort: any failure
/// (missing vault, Obsidian not installed, parse error) leaves the
/// corresponding field as `None`, disabling hyperlink emission gracefully.
fn enrich_render_opts_for_run(repo: &Path, mut render_opts: RenderOptions) -> RenderOptions {
    use codebus_core::obsidian;
    use codebus_core::vault::layout::vault_paths;
    use codebus_core::wiki::slug_index;

    let p = vault_paths(repo);
    if let Ok(idx) = slug_index::build(&p) {
        render_opts.slug_index = Some(std::sync::Arc::new(idx));
    }
    if let Ok(Some(id)) = obsidian::lookup_vault_id(&p.wiki) {
        render_opts.vault_id = Some(id);
    }
    render_opts
}

fn is_color_capable() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    // Cross-platform TTY detection — same rationale as `auto_detect_emoji`.
    // Modern terminals (Windows Terminal / PowerShell 7 / VSCode integrated
    // / iTerm2 / GNOME Terminal) all advertise TTY and accept ANSI colors.
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
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

    let (fix_disabled, fix_max_iterations) = resolve_fix_config(cli.no_fix, cli.fix_max_iter, &cfg);
    let no_obsidian_register = cli.no_obsidian_register;

    if cli.fix {
        return run_fix_cmd(
            &repo,
            render_opts,
            &cfg,
            fix_disabled,
            fix_max_iterations,
            no_obsidian_register,
        )
        .await;
    }

    if cli.goal.is_none() && cli.query.is_none() {
        return run_init_cmd(&repo, render_opts, no_obsidian_register).await;
    }

    if let Some(g) = cli.goal {
        return run_goal_cmd(
            &repo,
            &g,
            render_opts,
            &cfg,
            fix_disabled,
            fix_max_iterations,
            no_obsidian_register,
        )
        .await;
    }
    if let Some(q) = cli.query {
        return run_query_cmd(&repo, &q, render_opts, &cfg, no_obsidian_register).await;
    }
    ExitCode::from(0)
}

/// Resolve the effective fix-loop policy by combining CLI overrides with
/// the global config. Used by both the goal flow and the standalone
/// `--fix` mode so they share one policy.
///
/// Rules (spec scenarios):
/// - `--no-fix` wins over `--fix-max-iter` (the latter is moot when the
///   loop is disabled).
/// - `--fix-max-iter` overrides `lint.auto_fix.max_iterations` when supplied.
/// - Default config (no `lint.auto_fix` section) yields
///   `(fix_disabled = false, max_iterations = 5)`.
fn resolve_fix_config(
    no_fix_cli: bool,
    fix_max_iter_cli: Option<u32>,
    cfg: &GlobalConfig,
) -> (bool, u32) {
    let auto_fix = cfg.lint.as_ref().map(|l| l.auto_fix).unwrap_or_default();
    let fix_disabled = no_fix_cli || !auto_fix.enabled;
    let max_iterations = fix_max_iter_cli.unwrap_or(auto_fix.max_iterations);
    (fix_disabled, max_iterations)
}

/// Build a [`ProviderConfig`] from a [`GlobalConfig`]. `cfg.llm == None`
/// (section unset or unknown discriminator) yields the default variant
/// (`ClaudeCli` with no binary path) — preserving 0.2.0 behavior when no
/// `~/.codebus/config.yaml` exists.
///
/// Post-tagged-enum-refactor this is a one-line clone: the loader has
/// already produced the correct variant from YAML; main.rs no longer
/// needs to bridge flat fields.
fn provider_config_from(cfg: &GlobalConfig) -> ProviderConfig {
    cfg.llm.clone().unwrap_or_default()
}

/// Build a [`ScannerConfig`] from a [`GlobalConfig`]. `cfg.pii == None`
/// (section unset or unknown discriminator) yields the default variant
/// (`Null` with `OnHit::Warn`) — preserving 0.2.0 raw mirror behavior
/// when no `~/.codebus/config.yaml` exists.
fn scanner_config_from(cfg: &GlobalConfig) -> ScannerConfig {
    cfg.pii.clone().unwrap_or_default()
}

/// Build a [`SinkConfig`] from a [`GlobalConfig`]. `cfg.log == None`
/// (section unset) yields the default variant (`Null {}`) — preserving
/// 0.x behavior of "no telemetry persistence" until the user opts in.
fn sink_config_from(cfg: &GlobalConfig) -> SinkConfig {
    cfg.log.clone().unwrap_or_default()
}

/// Resolve the vault-local default for `SinkConfig::Jsonl { dir: None }`.
/// When the user wrote `log: { sink: jsonl }` without a `dir`, fall back
/// to `<repo>/.codebus/logs/`. Other variants pass through unchanged.
fn resolve_jsonl_dir(repo: &Path, cfg: SinkConfig) -> SinkConfig {
    match cfg {
        SinkConfig::Jsonl { dir: None } => SinkConfig::Jsonl {
            dir: Some(repo.join(".codebus").join("logs")),
        },
        other => other,
    }
}

/// Extract `(model, effort)` from `ProviderConfig::ClaudeCli`, returning
/// `(None, None)` for any other provider variant (those don't carry the
/// `--model` / `--effort` flag concept; their model/effort knobs live on
/// their own variant fields and will be plumbed in #2 multi-LLM).
fn claude_cli_model_effort(pc: &ProviderConfig) -> (Option<String>, Option<String>) {
    match pc {
        ProviderConfig::ClaudeCli {
            model,
            effort,
            ..
        } => (model.clone(), effort.clone()),
        _ => (None, None),
    }
}

/// Extract the `on_hit` policy from a [`ScannerConfig`] regardless of
/// variant. Used by goal flow which needs the policy alongside the built
/// scanner. Centralized here so the goal-call site doesn't carry a match
/// every time.
fn on_hit_of(cfg: &ScannerConfig) -> codebus_core::pii::OnHit {
    match cfg {
        ScannerConfig::Null { on_hit }
        | ScannerConfig::RegexBasic { on_hit, .. }
        | ScannerConfig::Presidio { on_hit }
        | ScannerConfig::Aws { on_hit } => *on_hit,
    }
}

async fn run_fix_cmd(
    repo: &PathBuf,
    render_opts: RenderOptions,
    cfg: &GlobalConfig,
    fix_disabled: bool,
    fix_max_iterations: u32,
    _no_obsidian_register: bool,
) -> ExitCode {
    let render_opts = enrich_render_opts_for_run(repo, render_opts);
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });

    if fix_disabled {
        // Spec: "--no-fix wins when both flags are present" and the
        // user-facing semantics for `--fix --no-fix` should be: short-
        // circuit politely rather than running the loop and committing
        // an empty fix-loop run.
        eprintln!("info: --no-fix supplied; fix loop skipped");
        return ExitCode::from(0);
    }

    let provider = match build_provider(provider_config_from(cfg)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let mut log_sink = match build_sink(resolve_jsonl_dir(repo, sink_config_from(cfg))) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to build log sink: {e}");
            return ExitCode::from(1);
        }
    };
    let result = match fix::run_fix(
        fix::RunFixOptions {
            repo_root: repo,
            provider: provider.as_ref(),
            fix_max_iterations,
        },
        &mut renderer,
        log_sink.as_mut(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let wiki_path = format!("{}/.codebus/wiki", repo.display()).replace('\\', "/");
    renderer.render_banner(&Banner::Done {
        wiki_path: &wiki_path,
    });
    let pre = result.pre_lint.issues.len();
    let post = result.post_lint.issues.len();
    if pre != post {
        eprintln!("info: lint fix loop reduced issues {pre} -> {post}");
    }
    renderer.render_lint_summary(&result.post_lint);
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
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_lint_report(&result);
    if result.error_count > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

async fn run_init_cmd(
    repo: &PathBuf,
    render_opts: RenderOptions,
    no_obsidian_register: bool,
) -> ExitCode {
    let use_emoji = render_opts.use_emoji;
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
    if let Err(e) = init::run_init(repo, no_obsidian_register) {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    let ok = if use_emoji { "✨" } else { "✓" };
    let tip = if use_emoji { "💡" } else { "i" };
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
    fix_disabled: bool,
    fix_max_iterations: u32,
    no_obsidian_register: bool,
) -> ExitCode {
    let render_opts = enrich_render_opts_for_run(repo, render_opts);
    let use_emoji = render_opts.use_emoji;
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
    renderer.render_banner(&Banner::Goal { goal: goal_text });

    let provider_cfg = provider_config_from(cfg);
    let (model, effort) = claude_cli_model_effort(&provider_cfg);
    let provider = match build_provider(provider_cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let scanner_cfg = scanner_config_from(cfg);
    let pii_on_hit = on_hit_of(&scanner_cfg);
    // Fail-fast BEFORE invoking the LLM agent — a malformed `patterns_extra`
    // entry should not silently degrade to NullScanner (per design open
    // question resolution).
    let pii_scanner = match build_scanner(scanner_cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to build PII scanner: {e}");
            return ExitCode::from(1);
        }
    };
    let mut log_sink = match build_sink(resolve_jsonl_dir(repo, sink_config_from(cfg))) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to build log sink: {e}");
            return ExitCode::from(1);
        }
    };
    let result = match goal::run_goal(
        goal::RunGoalOptions {
            repo_root: repo,
            goal: goal_text,
            provider: provider.as_ref(),
            pii_scanner: pii_scanner.as_ref(),
            pii_on_hit,
            fix_disabled,
            fix_max_iterations,
            model: model.as_deref(),
            effort: effort.as_deref(),
            no_obsidian_register,
        },
        &mut renderer,
        log_sink.as_mut(),
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
        let shrug = if use_emoji { "🤷" } else { "~" };
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
    _no_obsidian_register: bool,
) -> ExitCode {
    let render_opts = enrich_render_opts_for_run(repo, render_opts);
    let mut renderer = TerminalRenderer::new(render_opts);
    use codebus_core::render::EventRenderer;
    renderer.render_banner(&Banner::Start {
        path: &repo.to_string_lossy(),
    });
    let provider_cfg = provider_config_from(cfg);
    let (model, effort) = claude_cli_model_effort(&provider_cfg);
    let provider = match build_provider(provider_cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };
    let mut log_sink = match build_sink(resolve_jsonl_dir(repo, sink_config_from(cfg))) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to build log sink: {e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = query::run_query(
        query::RunQueryOptions {
            repo_root: repo,
            query: query_text,
            provider: provider.as_ref(),
            model: model.as_deref(),
            effort: effort.as_deref(),
        },
        &mut renderer,
        log_sink.as_mut(),
    )
    .await
    {
        eprintln!("error: {e}");
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codebus_core::config::{AutoFixConfig, LintConfig};
    use codebus_core::pii::OnHit;

    #[test]
    fn scanner_config_from_default_when_pii_section_absent() {
        // Default config preserves 0.2.0 behavior in goal flow:
        // no pii section → Null variant with OnHit::Warn (defaults).
        let cfg = GlobalConfig::default();
        let sc = scanner_config_from(&cfg);
        match sc {
            ScannerConfig::Null { on_hit } => assert_eq!(on_hit, OnHit::Warn),
            other => panic!("expected Null default, got {other:?}"),
        }
    }

    #[test]
    fn scanner_config_from_propagates_variant_on_hit_and_extras() {
        // Goal command propagates PII config from global config to raw_sync.
        let cfg = GlobalConfig {
            pii: Some(ScannerConfig::RegexBasic {
                on_hit: OnHit::Skip,
                patterns_extra: vec![r"INTERNAL-\d{6}".to_string()],
            }),
            ..GlobalConfig::default()
        };
        let sc = scanner_config_from(&cfg);
        match sc {
            ScannerConfig::RegexBasic {
                on_hit,
                patterns_extra,
            } => {
                assert_eq!(on_hit, OnHit::Skip);
                assert_eq!(patterns_extra, vec![r"INTERNAL-\d{6}".to_string()]);
            }
            other => panic!("expected RegexBasic, got {other:?}"),
        }
    }

    #[test]
    fn on_hit_of_extracts_per_variant() {
        assert_eq!(
            on_hit_of(&ScannerConfig::Null { on_hit: OnHit::Skip }),
            OnHit::Skip
        );
        assert_eq!(
            on_hit_of(&ScannerConfig::RegexBasic {
                on_hit: OnHit::Mask,
                patterns_extra: vec![],
            }),
            OnHit::Mask
        );
    }

    #[test]
    fn malformed_patterns_extra_fails_build_scanner_so_main_can_abort() {
        // Sharp edge: when patterns_extra contains bad regex, build_scanner
        // returns Err so main can fail-fast before invoking the LLM agent.
        let cfg = GlobalConfig {
            pii: Some(ScannerConfig::RegexBasic {
                on_hit: OnHit::Warn,
                patterns_extra: vec!["[unterminated".to_string()],
            }),
            ..GlobalConfig::default()
        };
        let sc = scanner_config_from(&cfg);
        let result = build_scanner(sc);
        assert!(
            result.is_err(),
            "malformed patterns_extra must propagate Err"
        );
    }

    // === lint-feedback-loop: resolve_fix_config ===

    fn cfg_with_auto_fix(enabled: bool, max_iterations: u32) -> GlobalConfig {
        GlobalConfig {
            lint: Some(LintConfig {
                disabled_rules: Vec::new(),
                custom_rules_dir: None,
                auto_fix: AutoFixConfig {
                    enabled,
                    max_iterations,
                },
            }),
            ..GlobalConfig::default()
        }
    }

    #[test]
    fn resolve_fix_config_default_when_no_lint_section() {
        // Spec: "Default config enables fix with max iterations five"
        let cfg = GlobalConfig::default();
        let (disabled, max_iter) = resolve_fix_config(false, None, &cfg);
        assert!(!disabled);
        assert_eq!(max_iter, 5);
    }

    #[test]
    fn resolve_fix_config_no_fix_flag_disables_even_when_config_enables() {
        // Spec: "--no-fix flag disables fix even when config enables it"
        let cfg = cfg_with_auto_fix(true, 5);
        let (disabled, max_iter) = resolve_fix_config(true, None, &cfg);
        assert!(disabled);
        assert_eq!(max_iter, 5);
    }

    #[test]
    fn resolve_fix_config_max_iter_flag_overrides_config_max() {
        // Spec: "--fix-max-iter overrides config max_iterations"
        let cfg = cfg_with_auto_fix(true, 5);
        let (disabled, max_iter) = resolve_fix_config(false, Some(10), &cfg);
        assert!(!disabled);
        assert_eq!(max_iter, 10);
    }

    #[test]
    fn resolve_fix_config_no_fix_wins_over_max_iter() {
        // Spec: "--no-fix wins when both flags are present"
        let cfg = cfg_with_auto_fix(true, 5);
        let (disabled, _max_iter) = resolve_fix_config(true, Some(10), &cfg);
        assert!(disabled);
    }

    #[test]
    fn resolve_fix_config_disabled_in_config_propagates() {
        // Spec: "Disabled config skips the fix loop in goal flow"
        let cfg = cfg_with_auto_fix(false, 7);
        let (disabled, max_iter) = resolve_fix_config(false, None, &cfg);
        assert!(disabled);
        assert_eq!(max_iter, 7);
    }

    // === obsidian-clickable-wikilinks: --no-obsidian-register flag ===

    #[test]
    fn no_obsidian_register_defaults_to_false_when_absent() {
        let cli = Cli::try_parse_from(["codebus", "--repo", "."]).expect("parse");
        assert!(!cli.no_obsidian_register);
    }

    #[test]
    fn no_obsidian_register_flag_sets_true() {
        let cli =
            Cli::try_parse_from(["codebus", "--repo", ".", "--no-obsidian-register"]).expect("parse");
        assert!(cli.no_obsidian_register);
    }
}
