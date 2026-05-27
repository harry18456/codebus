//! `codebus chat` — interactive multi-turn read-only chat REPL.
//!
//! Unlike the thin-wrapper goal/query/fix commands, chat is a REPL state
//! machine that calls `codebus_core::verb::chat::run_chat_turn` once per
//! user turn, maintains transcript state across turns, registers a SIGINT
//! trap to cancel mid-turn (with `--resume <id>` resume on next message),
//! and — when the agent emits a promote-suggestion line marker — spawns
//! `codebus goal "<transcript>"` as a child subprocess (NOT a library call,
//! so the long-running goal flow does not block chat REPL stdin).
//!
//! See specs `chat-verb` + `cli` (Chat Subcommand Behavior, Spawn Verb
//! Library Delegation modified) + design `docs/2026-05-13-chat-verb-discussion.md`.

use std::cell::RefCell;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Args;
use codebus_core::render::RenderOptions;
use codebus_core::stream::StreamEvent;
use codebus_core::verb::chat::{ChatTurnOptions, run_chat_turn};
use codebus_core::verb::{VerbError, VerbEvent, VerbLifecycleEvent};

/// `codebus chat` accepts no positional args (the REPL collects user
/// prompts from stdin). Global flags (`--debug`, `--no-emoji`, etc.) come
/// from the binary-level clap surface.
#[derive(Args, Debug)]
pub struct ChatArgs {}

/// REPL prompt symbol displayed at each turn boundary.
const PROMPT: &str = "> ";

/// Stdout marker shown after a mid-turn cancel so the user knows the next
/// message resumes the same Claude CLI session.
const INTERRUPTED_HINT: &str = "[interrupted, send your next message to continue this session]";

/// Stdout marker for the inline promote-to-goal confirmation prompt.
const PROMOTE_CONFIRM_PROMPT: &str = "[suggest] promote to wiki? (y/n) ";

/// Pick the activity-stream tool-line prefix per render options. When
/// emoji output is enabled, map each known tool to a distinct glyph so
/// the activity stream stays scannable; fall back to a plain `→` arrow
/// otherwise. Per `Activity Stream Render` requirement (chat-verb spec)
/// + `Environment-Aware Output Styling` (cli spec).
fn tool_prefix(tool_name: &str, use_emoji: bool) -> &'static str {
    if !use_emoji {
        return "→";
    }
    match tool_name {
        "Read" => "📖",
        "Glob" => "🔍",
        "Grep" => "🔎",
        _ => "→",
    }
}

pub async fn run(
    repo: &Path,
    _args: ChatArgs,
    debug: bool,
    render_opts: &RenderOptions,
) -> ExitCode {
    if debug {
        eprintln!("[debug] chat: repo={}", repo.display());
    }

    // Vault precondition. Chat is a wiki reader, not a producer — missing
    // vault is a user input error (per Chat Subcommand Behavior).
    let vault = repo.join(".codebus");
    if !vault.exists() {
        eprintln!(
            "error: chat: vault not found at {}; run `codebus init` first",
            vault.display()
        );
        return ExitCode::from(2);
    }

    // SIGINT trap. Handler runs on a dedicated OS thread; second Ctrl+C
    // calls process::exit(0) directly so a blocked stdin read at the REPL
    // prompt cannot trap the user inside a "I want to leave now" intent.
    // First Ctrl+C just flips the flag — run_chat_turn polls this between
    // stream lines per `Cancellation Signal Polling` (verb-library).
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_handler = cancel.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        if cancel_for_handler.swap(true, Ordering::Relaxed) {
            // Flag was already true → second Ctrl+C in succession.
            // Exit immediately, bypassing the blocked stdin read.
            std::process::exit(0);
        }
    }) {
        eprintln!("warning: chat: could not install SIGINT handler ({e}); Ctrl+C will terminate without graceful cancel");
    }

    // REPL state.
    let mut session_id: Option<String> = None;
    let mut transcript: Vec<(String, String)> = Vec::new();

    let codebus_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("codebus"));

    let stdin = io::stdin();
    let mut buffer = String::new();
    print_prompt();

    loop {
        buffer.clear();
        let n = match stdin.lock().read_line(&mut buffer) {
            Ok(n) => n,
            Err(_) => return ExitCode::SUCCESS,
        };
        if n == 0 {
            // EOF (Ctrl+D Unix / Ctrl+Z Windows console).
            return ExitCode::SUCCESS;
        }

        // After a mid-turn cancel the flag is still true at the next
        // prompt. Pressing Enter on empty input is treated as confirmation
        // of the cancel → exit (matches the "Double Ctrl+C exits REPL"
        // scenario without requiring async stdin).
        let line = buffer.trim();
        let prior_cancel = cancel.swap(false, Ordering::Relaxed);
        if prior_cancel && line.is_empty() {
            return ExitCode::SUCCESS;
        }

        // Exit aliases.
        if matches!(line, "exit" | ":q") {
            return ExitCode::SUCCESS;
        }

        // Empty input redisplays prompt without spawning a turn.
        if line.is_empty() {
            print_prompt();
            continue;
        }

        let user_prompt = line.to_string();

        // Per-turn buffers populated by the event closure.
        let assistant_chunks: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let promote_reason: RefCell<Option<String>> = RefCell::new(None);
        let use_emoji = render_opts.use_emoji;

        let on_event = |event: VerbEvent| match &event {
            VerbEvent::Lifecycle(VerbLifecycleEvent::PromoteSuggestion { reason }) => {
                // Latest suggestion wins (spec allows at most one per
                // message; this preserves that even on hypothetical
                // future emit patterns).
                *promote_reason.borrow_mut() = Some(reason.clone());
            }
            VerbEvent::Stream(StreamEvent::ToolUse { name, input, .. }) => {
                let prefix = tool_prefix(name, use_emoji);
                println!("{prefix} {} {}", name, abbreviate_tool_input(input));
                let _ = io::stdout().flush();
            }
            VerbEvent::Stream(StreamEvent::Thought { text }) => {
                // Buffer assistant text. Per `Activity Stream Render`:
                // do NOT print per-chunk; print once at turn completion.
                assistant_chunks.borrow_mut().push(text.clone());
            }
            // Other Stream / Lifecycle events: ignored at the CLI layer
            // (events sink still persists them via run_chat_turn).
            _ => {}
        };

        let options = ChatTurnOptions {
            text: user_prompt.clone(),
            session_id: session_id.clone(),
        };
        let result = run_chat_turn(repo, options, on_event, Some(cancel.clone()));

        // Cancel-during-turn path: don't propagate the error code; redisplay
        // prompt and retain session_id for next message's `--resume <id>`.
        if cancel.load(Ordering::Relaxed) {
            println!("{INTERRUPTED_HINT}");
            // run_chat_turn may have populated session_id via partial Ok
            // path (not in current impl — Cancelled is Err) but defend
            // anyway by leaving session_id untouched if no new id observed.
            if let Ok(ref report) = result {
                session_id = Some(report.session_id.clone());
            }
            print_prompt();
            continue;
        }

        match result {
            Ok(report) => {
                session_id = Some(report.session_id.clone());
                let full_text = assistant_chunks.borrow().join("");
                if !full_text.is_empty() {
                    println!("{full_text}");
                    let _ = io::stdout().flush();
                }
                transcript.push((user_prompt, full_text));

                // Promote confirmation. Deferred to AFTER the turn so the
                // (y/n) prompt does not interleave with streaming output.
                let reason_opt = promote_reason.borrow().clone();
                if let Some(reason) = reason_opt {
                    print!("{PROMOTE_CONFIRM_PROMPT}");
                    let _ = io::stdout().flush();
                    let mut confirm = String::new();
                    let read_res = stdin.lock().read_line(&mut confirm);
                    let trimmed = confirm.trim().to_ascii_lowercase();
                    if read_res.is_ok() && trimmed == "y" {
                        spawn_promote_to_goal(
                            &codebus_bin,
                            repo,
                            &transcript,
                            &reason,
                            render_opts,
                            debug,
                        );
                    }
                }
            }
            Err(e) => return translate_error(&e),
        }

        print_prompt();
    }
}

fn print_prompt() {
    print!("{PROMPT}");
    let _ = io::stdout().flush();
}

/// Render the input payload of a `tool_use` event as a one-line summary.
/// Tries the common shapes (`file_path`, `pattern`, `pattern`+`path`) and
/// falls back to a truncated JSON dump. Output width is bounded to ~70
/// columns to keep the activity stream legible.
fn abbreviate_tool_input(input: &serde_json::Value) -> String {
    const WIDTH: usize = 70;
    if let Some(p) = input.get("file_path").and_then(|v| v.as_str()) {
        return truncate(p, WIDTH);
    }
    if let Some(pat) = input.get("pattern").and_then(|v| v.as_str()) {
        let suffix = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| format!(" in {}", truncate(p, WIDTH / 2)))
            .unwrap_or_default();
        return truncate(&format!("{pat:?}{suffix}"), WIDTH);
    }
    truncate(&serde_json::to_string(input).unwrap_or_default(), WIDTH)
}

fn truncate(s: &str, width: usize) -> String {
    if s.chars().count() <= width {
        s.to_string()
    } else {
        let take: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{take}…")
    }
}

/// Compose the transcript dump that the `codebus goal` subprocess
/// receives as its positional argument. See `Transcript Dump Format For
/// Goal Subprocess` requirement (chat-verb capability).
pub(crate) fn format_transcript_dump(transcript: &[(String, String)], reason: &str) -> String {
    let mut out = String::from("Based on this conversation:\n\n");
    for (user, assistant) in transcript {
        out.push_str("<user>: ");
        out.push_str(user);
        out.push('\n');
        out.push_str("<assistant>: ");
        out.push_str(assistant);
        out.push('\n');
    }
    out.push_str("\nWrite: ");
    out.push_str(reason);
    out.push('\n');
    out
}

fn spawn_promote_to_goal(
    codebus_bin: &Path,
    repo: &Path,
    transcript: &[(String, String)],
    reason: &str,
    _render_opts: &RenderOptions,
    debug: bool,
) {
    let formatted = format_transcript_dump(transcript, reason);
    if debug {
        eprintln!(
            "[debug] chat: spawning `{} goal` subprocess with {} transcript chars",
            codebus_bin.display(),
            formatted.len()
        );
    }
    // Spawn `codebus goal "<transcript>"`. Inherit stdio so the goal
    // verb's stream-json render appears in the same terminal. We use
    // .status() not .output() so output streams in real time.
    let mut cmd = Command::new(codebus_bin);
    cmd.arg("goal").arg(&formatted).current_dir(repo);
    match cmd.status() {
        Ok(_status) => {
            // Goal subprocess wrote its own RunLog row (mode=goal, no
            // session_id field). chat REPL continues regardless of goal
            // exit code — failed promote shouldn't kill the chat session.
        }
        Err(e) => {
            eprintln!("error: chat: failed to spawn `codebus goal` subprocess: {e}");
        }
    }
}

fn translate_error(err: &VerbError) -> ExitCode {
    match err {
        VerbError::VaultMissing { path } => {
            eprintln!(
                "error: chat: vault not found at {}; run `codebus init` first",
                path.display()
            );
            ExitCode::from(2)
        }
        VerbError::ConfigParse { which, source } => {
            eprintln!("error: chat: {which} config parse failed: {source}");
            ExitCode::from(2)
        }
        VerbError::KeyringMissing { source } => {
            eprintln!("error: chat: {source}");
            ExitCode::from(3)
        }
        VerbError::Spawn { source } => {
            eprintln!("error: chat: spawn claude: {source}");
            ExitCode::from(1)
        }
        VerbError::Cancelled => {
            // Cancel during turn is handled inline above — this branch
            // covers Err propagation when run_chat_turn returns Cancelled
            // without the main-loop cancel flag still being set (e.g.
            // race window). Treat as graceful exit.
            ExitCode::SUCCESS
        }
        VerbError::AgentFailed { exit_code } => {
            // Active arm: chat is the only verb that emits AgentFailed
            // (per spec verb-library §Verb Error Enum). Surface the
            // child's exit code in stderr so the user sees something
            // distinct from a launch failure (Spawn) or an internal panic
            // (Internal). The CLI exit code itself is collapsed to 1 via
            // cli_exit_code() — chat REPL semantics keep "1 = error"
            // simple for shell consumers.
            match exit_code {
                Some(code) => eprintln!("error: chat: agent exited with code {code}"),
                None => eprintln!("error: chat: agent exited without a recorded exit code"),
            }
            ExitCode::from(err.cli_exit_code())
        }
        VerbError::Internal { message } => {
            eprintln!("error: chat: {message}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Transcript Dump Format For Goal Subprocess` scenario:
    /// "Two-turn transcript format"
    #[test]
    fn transcript_dump_format_two_turn_example() {
        let transcript = vec![
            ("what does X do?".to_string(), "X does Y.".to_string()),
            (
                "summarize".to_string(),
                "[CODEBUS_PROMOTE_SUGGESTION] X module behavior summary\n\nIn summary, X is ..."
                    .to_string(),
            ),
        ];
        let reason = "X module behavior summary";
        let dump = format_transcript_dump(&transcript, reason);
        assert!(dump.starts_with("Based on this conversation:\n\n"));
        assert!(dump.contains("<user>: what does X do?\n"));
        assert!(dump.contains("<assistant>: X does Y.\n"));
        assert!(dump.contains("<user>: summarize\n"));
        assert!(dump.contains(
            "<assistant>: [CODEBUS_PROMOTE_SUGGESTION] X module behavior summary\n\nIn summary, X is ...\n"
        ));
        assert!(dump.ends_with("\nWrite: X module behavior summary\n"));
    }

    /// chat REPL accumulates `(user, assistant)` pairs across turns;
    /// `format_transcript_dump` is the contract surface for the buffer.
    /// Two pushes → two `<user>:` / `<assistant>:` line pairs (per
    /// `Chat Transcript Buffer Accumulates Two Turns` style scenario).
    #[test]
    fn chat_transcript_buffer_accumulates_two_turns() {
        let mut transcript: Vec<(String, String)> = Vec::new();
        transcript.push(("turn 1 q".into(), "turn 1 a".into()));
        transcript.push(("turn 2 q".into(), "turn 2 a".into()));
        assert_eq!(transcript.len(), 2);
        let dump = format_transcript_dump(&transcript, "topic");
        let user_lines: Vec<_> = dump.lines().filter(|l| l.starts_with("<user>:")).collect();
        let assistant_lines: Vec<_> = dump
            .lines()
            .filter(|l| l.starts_with("<assistant>:"))
            .collect();
        assert_eq!(user_lines.len(), 2);
        assert_eq!(assistant_lines.len(), 2);
    }

    #[test]
    fn abbreviate_tool_input_uses_file_path_when_present() {
        let input = serde_json::json!({"file_path": "wiki/modules/uv-lib.md"});
        assert_eq!(abbreviate_tool_input(&input), "wiki/modules/uv-lib.md");
    }

    #[test]
    fn abbreviate_tool_input_uses_pattern_when_no_file_path() {
        let input = serde_json::json!({"pattern": "wiki/modules/*.md"});
        let s = abbreviate_tool_input(&input);
        assert!(s.contains("wiki/modules/*.md"));
    }

    #[test]
    fn abbreviate_tool_input_truncates_long_payloads() {
        let long = "a".repeat(200);
        let input = serde_json::json!({"file_path": long});
        let s = abbreviate_tool_input(&input);
        assert!(s.chars().count() <= 70);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn tool_prefix_uses_arrow_when_emoji_disabled() {
        for tool in ["Read", "Glob", "Grep", "Unknown"] {
            assert_eq!(tool_prefix(tool, false), "→", "tool={tool}");
        }
    }

    #[test]
    fn tool_prefix_uses_distinct_emoji_per_known_tool() {
        assert_eq!(tool_prefix("Read", true), "📖");
        assert_eq!(tool_prefix("Glob", true), "🔍");
        assert_eq!(tool_prefix("Grep", true), "🔎");
        // Unknown tool falls back to arrow even with emoji enabled —
        // safer than silently emitting an empty / wrong glyph.
        assert_eq!(tool_prefix("Bash", true), "→");
    }
}
