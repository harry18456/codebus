//! `codebus hook check-bash` — PreToolUse Bash hook for the fix sandbox.
//!
//! Internal interface, NOT a user-facing surface. Hidden from `--help`
//! via `#[command(hide = true)]` on the parent `Hook` enum variant.
//!
//! Per v3-fix-trust-agent's `Fix Bash Hook Installation` requirement:
//! - Stdin: PreToolUse hook JSON, e.g.
//!   `{"tool_name":"Bash","tool_input":{"command":"codebus lint --format json"},...}`
//! - Allow: command's first argv token resolves to the codebus binary
//!   (`codebus` or `codebus.exe` basename, case-insensitive on Windows)
//!   AND the second argv token is exactly `lint`. Exit 0, no decision JSON.
//! - Block: anything else (other binary, wrong subcommand, parse error,
//!   missing fields). Exit 0 with stdout JSON `{"decision":"block","reason":"<msg>"}`.
//! - Fail-closed default — never silently allow on parse failure.

use clap::Subcommand;
use codebus_core::config::{HooksConfig, default_config_path, load_hooks_config};
use codebus_core::vault::settings::matches_sensitive_basename;
use serde::Deserialize;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Subcommand, Debug)]
pub enum HookArgs {
    /// PreToolUse Bash hook: allow `codebus lint *` or
    /// `codebus quiz validate *`, block everything else. Reads JSON from
    /// stdin, prints decision JSON to stdout, always exits 0.
    CheckBash,
    /// PreToolUse Read hook: block image / binary file extensions whose
    /// contents would bypass the regex_basic PII filter. Reads JSON from
    /// stdin, prints decision JSON to stdout, always exits 0.
    CheckRead,
}

pub async fn run(args: HookArgs) -> ExitCode {
    match args {
        HookArgs::CheckBash => check_bash().await,
        HookArgs::CheckRead => check_read().await,
    }
}

async fn check_bash() -> ExitCode {
    // Read full stdin. Empty / unread stdin is a fail-closed condition.
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return emit_block("hook: failed to read stdin");
    }
    if buf.trim().is_empty() {
        return emit_block("hook: empty stdin (no PreToolUse JSON received)");
    }

    let parsed: PreToolUseInput = match serde_json::from_str(&buf) {
        Ok(p) => p,
        Err(_) => return emit_block("hook: malformed PreToolUse JSON on stdin"),
    };

    let cmd = parsed
        .tool_input
        .as_ref()
        .and_then(|t| t.command.as_deref())
        .unwrap_or("");
    if cmd.is_empty() {
        return emit_block("hook: tool_input.command absent or empty");
    }

    if is_allowed_bash_command(cmd) {
        // Allow: exit 0 with no decision JSON.
        ExitCode::from(0)
    } else if let Some(metachar) = find_shell_metacharacter(cmd) {
        // Metacharacter rejection — surface the specific byte so the
        // user sees which symbol tripped the gate (per spec block reason
        // requirement).
        let display = format_metachar_for_reason(metachar);
        emit_block(&format!(
            "hook: command contains forbidden shell metacharacter {display}; received `{cmd}`"
        ))
    } else {
        emit_block(&format!(
            "hook: only `codebus lint *` or `codebus quiz validate *` is permitted by the codebus agent sandbox; received `{cmd}`"
        ))
    }
}

/// Render a rejected metacharacter for inclusion in the block reason
/// string. Newline AND carriage return are rendered as their escape
/// sequences (`\n`, `\r`) instead of raw bytes so the JSON reason stays
/// single-line AND the user can recognise them in tool output.
fn format_metachar_for_reason(c: char) -> String {
    match c {
        '\n' => "`\\n`".to_string(),
        '\r' => "`\\r`".to_string(),
        other => format!("`{other}`"),
    }
}

/// PreToolUse hook input — minimal shape; unknown fields silently dropped.
#[derive(Deserialize)]
struct PreToolUseInput {
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_input: Option<ToolInput>,
    /// check-read-vault-containment: the agent working directory Claude
    /// supplies in the PreToolUse payload; codebus sets it to the vault
    /// root. Primary source for the containment boundary (fallback: the
    /// hook subprocess cwd).
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct ToolInput {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    file_path: Option<serde_json::Value>,
    /// check-read-vault-containment: the `path` argument of `Glob` / `Grep`
    /// (their search root), distinct from `Read`'s `file_path`.
    #[serde(default)]
    path: Option<serde_json::Value>,
}

/// Per `Fix Bash Hook Installation` allow rule:
///   - argv[0] basename (without directory) is exactly `codebus` (Unix
///     case-sensitive) or `codebus.exe` / `codebus.EXE` / etc. (Windows
///     case-insensitive)
///   - argv[1] is exactly `lint`
fn is_codebus_lint_command(cmd: &str) -> bool {
    let mut parts = cmd.split_whitespace();
    let Some(binary) = parts.next() else {
        return false;
    };
    if !is_codebus_binary(binary) {
        return false;
    }
    matches!(parts.next(), Some("lint"))
}

/// `codebus quiz validate ...` — the codebus-quiz generate agent's
/// self-validation form (spec `lint-feedback-loop` / Fix Bash Hook
/// Installation, allow rule (b)). Same argv strictness as the lint
/// form: binary basename must be `codebus`, then exactly `quiz` then
/// `validate`. `codebus quiz "<topic>"` (generate) does NOT match.
fn is_codebus_quiz_validate_command(cmd: &str) -> bool {
    let mut parts = cmd.split_whitespace();
    let Some(binary) = parts.next() else {
        return false;
    };
    if !is_codebus_binary(binary) {
        return false;
    }
    matches!(parts.next(), Some("quiz")) && matches!(parts.next(), Some("validate"))
}

/// Combined PreToolUse allow predicate: the codebus-fix agent's
/// `codebus lint ...` OR the codebus-quiz generate agent's
/// `codebus quiz validate ...`. Everything else is blocked.
///
/// Metacharacter rejection runs BEFORE argv tokenization — any command
/// string containing a byte from `SHELL_METACHARACTERS` returns false,
/// even when the leading argv tokens would otherwise satisfy the allow
/// form. This prevents shell-chaining bypass (e.g.
/// `codebus lint --foo && rm -rf ~`, `codebus lint $(whoami)`) under
/// any shell that evaluates the raw command string (bash, Git Bash,
/// PowerShell). Predicate is byte-level on the raw string — no shell
/// quote parsing is done, so a metachar inside quotes is also rejected
/// (per spec lint-feedback-loop §Fix Bash Hook Installation).
fn is_allowed_bash_command(cmd: &str) -> bool {
    // quiz-heredoc-selfvalidate-unblock: the single-quoted `codebus quiz
    // validate` here-document is the ONE allowed shape that legitimately
    // contains the `<` and LF metacharacters (the codebus-quiz Mode B agent
    // self-validates by piping its draft in via a heredoc). Evaluate it BEFORE
    // the metacharacter rejection. Every other command is still screened by the
    // unchanged char-level scan below, so non-heredoc `<`, shell chaining, and
    // command substitution stay blocked — no F4 regression.
    if is_quiz_validate_heredoc(cmd) {
        return true;
    }
    if find_shell_metacharacter(cmd).is_some() {
        return false;
    }
    is_codebus_lint_command(cmd) || is_codebus_quiz_validate_command(cmd)
}

/// `quiz-heredoc-selfvalidate-unblock` — recognise the codebus-quiz Mode B
/// self-validation here-document so it can be allowed despite containing the
/// `<` and LF metacharacters. This is a STRUCTURAL recognizer, not a relaxation
/// of [`find_shell_metacharacter`] (whose semantics and
/// [`SHELL_METACHARACTERS`] set are unchanged). Per spec `lint-feedback-loop`
/// §Fix Bash Hook Installation, `Allow (quiz-validate heredoc)`.
///
/// Returns true ONLY for a command of the exact shape:
///
/// ```text
/// codebus quiz validate [args] - <<'MARKER'
/// <opaque body lines>
/// MARKER
/// ```
///
/// Invariants that make this safe to allow despite the opaque body:
/// - The marker MUST be SINGLE-quoted (`<<'MARKER'`). An unquoted marker would
///   let the shell expand `$(...)` / `$VAR` inside the body — the injection
///   vector this recognizer must not reopen — so it is rejected.
/// - The first line carries no metacharacter before the `<<` operator AND no
///   trailing command after the closing quote of the marker (so
///   `<<'X'; rm -rf ~` does not qualify).
/// - The body between the first line and the closing delimiter is treated as
///   opaque stdin and is NOT scanned (a single-quoted heredoc body cannot
///   escape to shell execution).
/// - A line equal to the marker MUST close the document, and only whitespace
///   may follow it (so a command after the closing delimiter does not qualify).
///
/// Any deviation returns false, and the caller falls back to the metacharacter
/// rejection / argv-tokenization paths (which block it).
fn is_quiz_validate_heredoc(cmd: &str) -> bool {
    // Split into logical lines on LF; strip a trailing CR so a CRLF command is
    // handled identically to an LF command.
    let mut lines = cmd.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));

    let Some(first) = lines.next() else {
        return false;
    };

    // The first line must carry the heredoc operator `<<`. Split on the FIRST
    // occurrence: `prefix` is the command, `op_rest` is the marker plus
    // anything following it on the same line.
    let Some((prefix, op_rest)) = first.split_once("<<") else {
        return false;
    };

    // Marker must be single-quoted: `'MARKER'`. This rejects the unquoted
    // `<<MARKER` form (body would undergo shell expansion) AND the here-string
    // `<<<...` form (op_rest would start with `<`, not `'`).
    let Some(after_open_quote) = op_rest.strip_prefix('\'') else {
        return false;
    };
    let Some(close_idx) = after_open_quote.find('\'') else {
        return false;
    };
    let marker = &after_open_quote[..close_idx];
    let trailing = &after_open_quote[close_idx + 1..];

    // Marker must be a non-empty run of word characters, and nothing but
    // whitespace may follow the closing quote (no trailing command).
    if marker.is_empty()
        || !marker
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return false;
    }
    if !trailing.trim().is_empty() {
        return false;
    }

    // The command before `<<` must be a clean `codebus quiz validate ...`
    // invocation: no metacharacter (reuse the unchanged scan) AND the correct
    // argv head.
    if find_shell_metacharacter(prefix).is_some() {
        return false;
    }
    if !is_codebus_quiz_validate_command(prefix) {
        return false;
    }

    // Walk the remaining lines: the body is opaque until a line equal to the
    // marker closes the document; after the close, only whitespace may appear.
    let mut closed = false;
    for line in lines {
        if closed {
            if !line.trim().is_empty() {
                return false;
            }
        } else if line == marker {
            closed = true;
        }
        // else: still inside the opaque body — deliberately not scanned.
    }

    closed
}

/// Shell metacharacter rejection set — the union of POSIX shell, Git
/// Bash, AND PowerShell high-risk symbols that enable command chaining,
/// command substitution, redirection, subshells, or multi-line eval.
/// Set is identical across all platforms (per spec lint-feedback-loop
/// §Fix Bash Hook Installation "Cross-platform" clause).
const SHELL_METACHARACTERS: &[char] = &[
    ';', '&', '|', '$', '`', '>', '<', '(', ')', '\n', '\r',
];

/// Return the first metacharacter from [`SHELL_METACHARACTERS`] found in
/// `cmd`, or `None` if none present. Used both by
/// [`is_allowed_bash_command`] (boolean reject) AND by [`check_bash`]
/// (so the block decision JSON's `reason` field can name the specific
/// metacharacter that fired).
fn find_shell_metacharacter(cmd: &str) -> Option<char> {
    cmd.chars().find(|c| SHELL_METACHARACTERS.contains(c))
}

fn is_codebus_binary(token: &str) -> bool {
    // Strip directory portion — handle both `/` and `\` separators so this
    // works on Unix paths AND Windows mixed paths (e.g. `D:/x/codebus.exe`).
    let basename = token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token);

    if cfg!(target_os = "windows") {
        // Case-insensitive on Windows: `codebus`, `codebus.exe`, `Codebus.EXE`.
        let lower = basename.to_ascii_lowercase();
        lower == "codebus" || lower == "codebus.exe"
    } else {
        // Case-sensitive on Unix.
        basename == "codebus"
    }
}

/// PreToolUse Read hook image / binary blocklist — file extensions whose
/// content would bypass `regex_basic` PII filtering (which only scans text).
/// Stored lowercase so callers compare against `to_ascii_lowercase()` output.
const IMAGE_BLOCKLIST: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif", "pdf", "ico", "heic", "heif",
    "avif",
];

/// Returns true when `path` ends in an extension on [`IMAGE_BLOCKLIST`].
///
/// Comparison is ASCII case-insensitive on every platform — this **deliberately
/// diverges** from [`is_codebus_binary`]'s OS-split behavior (Windows
/// case-insensitive, Unix case-sensitive). File extensions are conventionally
/// case-insensitive across all operating systems, and a POSIX case-sensitive
/// match would let `screenshot.PNG` bypass the blocklist on Linux. Do not
/// "fix" this to match `is_codebus_binary`'s pattern without first revisiting
/// the `pretooluse-image-block` change rationale.
///
/// Path separator handling: the directory portion is stripped using either
/// `/` or `\` as a separator so Unix paths, Windows native paths, and Windows
/// mixed-separator paths all yield the same extension.
fn is_image_path(path: &str) -> bool {
    let basename = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path);
    let Some(dot_pos) = basename.rfind('.') else {
        return false;
    };
    let ext = &basename[dot_pos + 1..];
    if ext.is_empty() {
        return false;
    }
    let ext_lower = ext.to_ascii_lowercase();
    IMAGE_BLOCKLIST.contains(&ext_lower.as_str())
}

/// Pure decision function for the PreToolUse Read hook — extracts the
/// block / allow decision from a stdin JSON body so the logic is unit
/// testable without subprocess stdin.
///
/// Returns `Some(reason)` when the agent's Read tool invocation MUST be
/// blocked (image extension hit OR fail-closed condition); `None` when
/// the Read invocation may proceed silently.
///
/// `hooks_cfg.read_image_block` gates the entire body (verify-stage-
/// independent-model-toggle change): when `false`, the function
/// short-circuits to `None` for ANY input — including malformed JSON,
/// empty stdin, and image extensions — because the user has explicitly
/// turned off the Read gate. When `true`, the function executes the
/// pre-toggle blocklist + fail-closed contract.
fn check_read_inner(
    stdin_body: &str,
    hooks_cfg: &HooksConfig,
    home: Option<&Path>,
) -> Option<String> {
    if !hooks_cfg.read_image_block {
        return None;
    }
    if stdin_body.trim().is_empty() {
        return Some("hook: empty stdin (no PreToolUse JSON received)".to_string());
    }
    let parsed: PreToolUseInput = match serde_json::from_str(stdin_body) {
        Ok(p) => p,
        Err(_) => return Some("hook: malformed PreToolUse JSON on stdin".to_string()),
    };
    let file_path_value = parsed.tool_input.as_ref().and_then(|t| t.file_path.as_ref());
    let path = match file_path_value {
        Some(serde_json::Value::String(s)) if !s.is_empty() => s.as_str(),
        _ => return Some("hook: tool_input.file_path absent or empty".to_string()),
    };
    if is_image_path(path) {
        return Some(format!(
            "hook: reading image / binary files is blocked to prevent PII bypass; received `{path}`"
        ));
    }
    if let Some(reason) = check_sensitive_path(path, home) {
        return Some(reason);
    }
    None
}

/// agent-hook-hardening §PII Image Read Hook Installation sensitive-path
/// blocklist. Two complementary rules:
///   (a) basename glob — `*id_rsa*`, `*.pem`, `*.key`. Independent of
///       home resolution; decides on basename alone so the rule fires
///       even when a key file lives outside any sensitive directory.
///   (b) home-relative prefix — `<home>/.ssh/`, `<home>/.aws/`,
///       `<home>/.gnupg/`, `<home>/.config/gh/`. Needs the running
///       user's home directory; fails closed (returns block) when home
///       resolution is unavailable AND the input path required home
///       comparison (`~`-prefixed or absolute).
///
/// All comparisons are ASCII case-insensitive AND path separators are
/// normalised to forward-slash before prefix matching, so
/// `C:\Users\harry\.ssh\config` AND `/home/harry/.ssh/config` trigger
/// the same rule on their respective OS.
fn check_sensitive_path(path: &str, home: Option<&Path>) -> Option<String> {
    // (a) basename-glob rule first — does not require home, so basename
    // hits are caught even on an environment where home resolution would
    // otherwise force fail-closed.
    let basename = extract_basename(path);
    if matches_sensitive_basename(basename) {
        return Some(format!(
            "hook: file path basename matches sensitive key glob (e.g. id_rsa / *.pem / *.key); received `{path}`"
        ));
    }
    // (b) home-relative prefix rule — only fires when the input path
    // could plausibly be home-rooted (`~`-prefixed or absolute).
    // Relative paths like `wiki/foo.md` are never under a sensitive home
    // prefix AND SHALL NOT trigger fail-closed when home is unresolved.
    let needs_home = path_requires_home_resolution(path);
    if !needs_home {
        return None;
    }
    let home = match home {
        Some(h) => h,
        None => {
            return Some(format!(
                "hook: home directory unresolvable, cannot evaluate sensitive-path rule; received `{path}`"
            ));
        }
    };
    if matches_sensitive_home_prefix(path, home) {
        return Some(format!(
            "hook: file path under sensitive home directory (e.g. ~/.ssh/, ~/.aws/, ~/.gnupg/, ~/.config/gh/); received `{path}`"
        ));
    }
    None
}

/// Strip directory portion using `/` or `\` as separators (mirrors the
/// existing `is_image_path` basename extraction).
fn extract_basename(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
}

/// Returns true when `path` is `~`-prefixed OR is an absolute path
/// (Unix `/...` or Windows drive-letter `X:/` / `X:\`). Relative paths
/// AND bare `~` (no separator suffix) return false — relative paths
/// cannot be home-rooted by construction, AND bare `~` never matches a
/// sensitive prefix even after expansion (the prefixes all carry a
/// directory suffix).
fn path_requires_home_resolution(path: &str) -> bool {
    if path == "~" {
        return false;
    }
    if path.starts_with("~/") || path.starts_with("~\\") {
        return true;
    }
    if path.starts_with('/') {
        return true;
    }
    // Windows drive-letter absolute path detection — `X:/` or `X:\` where
    // X is an ASCII alpha character. Drive-only `X:` (no separator) is
    // ambiguous AND treated as non-absolute (rare edge, unused in practice).
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

/// Returns true when `path` (after expanding a leading `~/` or `~\` to
/// `home` AND normalising backslashes to forward-slashes) starts with
/// any of the sensitive home-relative prefixes, compared ASCII
/// case-insensitively.
fn matches_sensitive_home_prefix(path: &str, home: &Path) -> bool {
    // Expand leading `~/` or `~\` to home.
    let expanded: String = if let Some(rest) = path.strip_prefix("~/") {
        format!("{}/{rest}", home.display())
    } else if let Some(rest) = path.strip_prefix("~\\") {
        format!("{}/{rest}", home.display())
    } else {
        path.to_string()
    };
    // Normalise separators AND case for comparison.
    let normalised = expanded.replace('\\', "/").to_ascii_lowercase();
    let home_str = home.display().to_string().replace('\\', "/").to_ascii_lowercase();
    let suffixes = [".ssh", ".aws", ".gnupg", ".config/gh"];
    for suffix in suffixes {
        let full_prefix = format!("{home_str}/{suffix}/");
        if normalised.starts_with(&full_prefix) {
            return true;
        }
    }
    false
}

async fn check_read() -> ExitCode {
    let mut buf = String::new();
    if io::stdin().read_to_string(&mut buf).is_err() {
        return emit_block("hook: failed to read stdin");
    }
    // verify-stage-independent-model-toggle: resolve the runtime gate
    // BEFORE running the blocklist. Any failure to load the config
    // (file absent, malformed yaml, missing section) falls back to
    // `HooksConfig::default()` — fail-safe to block. `default_config_path`
    // returning None (no resolvable home dir) is also treated as "no
    // config → default", preserving the pre-toggle behavior.
    let hooks_cfg = match default_config_path() {
        Some(p) => load_hooks_config(&p).unwrap_or_default(),
        None => HooksConfig::default(),
    };
    // agent-hook-hardening: resolve home for sensitive-path predicate.
    // `None` triggers fail-closed inside `check_sensitive_path` when the
    // input path requires home comparison (per spec PII Image Read Hook
    // Installation §Block (unresolvable home)).
    let home = dirs::home_dir();
    // check-read-vault-containment: the vault root falls back to the hook
    // subprocess working directory when the PreToolUse payload omits `cwd`.
    let env_cwd = std::env::current_dir().ok();

    // Stage 1 — vault containment (primary read gate; covers Read/Glob/Grep).
    if let Some(reason) =
        check_containment_inner(&buf, &hooks_cfg, home.as_deref(), env_cwd.as_deref())
    {
        return emit_block(&reason);
    }
    // Stage 2 — image / sensitive denylist (in-vault defense-in-depth).
    // Read-scoped: Glob/Grep carry no image-content-read risk AND are
    // governed by containment alone, so they skip this stage (and must not
    // be failed closed for lacking a `file_path`).
    if !is_search_tool(&buf)
        && let Some(reason) = check_read_inner(&buf, &hooks_cfg, home.as_deref()) {
            return emit_block(&reason);
        }
    ExitCode::from(0)
}

fn emit_block(reason: &str) -> ExitCode {
    let payload = format!(
        "{{\"decision\":\"block\",\"reason\":{}}}",
        json_escape(reason)
    );
    let _ = writeln!(io::stdout(), "{payload}");
    ExitCode::from(0)
}

/// Minimal JSON string escape — covers the chars we'd emit in our reason
/// strings (no need for full Unicode handling; reasons are ASCII / simple).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// --- check-read-vault-containment: vault-root containment gate ---

/// Resolve the agent's read target path from a parsed PreToolUse payload,
/// keyed by `tool_name`: `Read` uses `file_path`, `Glob`/`Grep` use `path`.
/// Any other tool name falls back to whichever field is present. Returns
/// `None` when the relevant field is absent / empty / non-string.
fn target_path_str<'a>(parsed: &'a PreToolUseInput, tool_name: &str) -> Option<&'a str> {
    let ti = parsed.tool_input.as_ref()?;
    let val = match tool_name {
        "Read" => ti.file_path.as_ref(),
        "Glob" | "Grep" => ti.path.as_ref(),
        _ => ti.file_path.as_ref().or(ti.path.as_ref()),
    };
    match val {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    }
}

/// Lexically resolve `.` and `..` without touching the filesystem (the
/// fallback when a path does not exist and so cannot be canonicalized).
fn lexical_clean(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Normalize a path to a comparable string: strip a Windows `\\?\` verbatim
/// prefix, convert separators to `/`, drop a trailing slash, AND (on Windows)
/// fold ASCII case so drive-letter / case differences do not defeat the
/// prefix comparison.
fn norm_path_string(p: &Path) -> String {
    let lossy = p.to_string_lossy();
    let stripped = lossy.strip_prefix(r"\\?\").unwrap_or(&lossy);
    let mut s = stripped.replace('\\', "/");
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    if cfg!(windows) {
        s = s.to_ascii_lowercase();
    }
    s
}

/// Decide whether `target` resolves inside `vault_root`. A leading `~/` /
/// `~\` is expanded to `home`; when that expansion is required but `home`
/// is unavailable the boundary is undecidable (`None` → caller fails
/// closed). Both operands are canonicalized (or lexically cleaned when they
/// do not exist) AND string-normalized before the prefix comparison, so a
/// `cwd` in backslash form and a target in forward-slash form (as observed
/// in real PreToolUse payloads) compare correctly.
fn target_within_vault(target: &str, vault_root: &Path, home: Option<&Path>) -> Option<bool> {
    let expanded: String = if let Some(rest) = target
        .strip_prefix("~/")
        .or_else(|| target.strip_prefix("~\\"))
    {
        match home {
            Some(h) => format!("{}/{}", h.display(), rest),
            None => return None,
        }
    } else {
        target.to_string()
    };
    let vr = fs::canonicalize(vault_root).unwrap_or_else(|_| lexical_clean(vault_root));
    let t = Path::new(&expanded);
    let target_abs = if t.is_absolute() {
        fs::canonicalize(t).unwrap_or_else(|_| lexical_clean(t))
    } else {
        lexical_clean(&vr.join(t))
    };
    let vr_n = norm_path_string(&vr);
    let t_n = norm_path_string(&target_abs);
    Some(t_n == vr_n || t_n.starts_with(&format!("{vr_n}/")))
}

/// Pure decision for the containment stage of `codebus hook check-read`.
/// Returns `Some(reason)` when the Read/Glob/Grep invocation MUST be blocked
/// for resolving outside the vault root (or for an undecidable boundary);
/// `None` when containment allows the path through to the denylist stage.
///
/// Gated by `hooks_cfg.read_path_containment` (independent of
/// `read_image_block`). `env_cwd` is the hook subprocess working directory,
/// used as the vault-root fallback when the PreToolUse payload omits `cwd`.
fn check_containment_inner(
    stdin_body: &str,
    hooks_cfg: &HooksConfig,
    home: Option<&Path>,
    env_cwd: Option<&Path>,
) -> Option<String> {
    if !hooks_cfg.read_path_containment {
        return None;
    }
    if stdin_body.trim().is_empty() {
        return Some("hook: empty stdin (no PreToolUse JSON received)".to_string());
    }
    let parsed: PreToolUseInput = match serde_json::from_str(stdin_body) {
        Ok(p) => p,
        Err(_) => return Some("hook: malformed PreToolUse JSON on stdin".to_string()),
    };
    let tool_name = parsed.tool_name.as_deref().unwrap_or_default();
    // No target path: Glob/Grep omitting `path` means the implicit search root
    // is the vault cwd (in-vault) → allow; a Read with no `file_path` is failed
    // closed by the denylist stage, not here.
    let Some(path) = target_path_str(&parsed, tool_name) else {
        return None;
    };
    let vault_root = parsed
        .cwd
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| env_cwd.map(|p| p.to_path_buf()));
    let vault_root = match vault_root {
        Some(v) => v,
        None => {
            return Some(format!(
                "hook: vault-containment — vault root unresolvable (no `cwd` in PreToolUse input, no process cwd); received `{path}`"
            ));
        }
    };
    match target_within_vault(path, &vault_root, home) {
        Some(true) => None,
        Some(false) => Some(format!(
            "hook: vault-containment — read path resolves outside the vault root; received `{path}`"
        )),
        None => Some(format!(
            "hook: vault-containment — cannot resolve `~` home directory for path; received `{path}`"
        )),
    }
}

/// True when the PreToolUse payload is a `Glob` or `Grep` invocation — those
/// search tools are governed by containment only AND skip the Read-scoped
/// denylist stage (so they are not failed closed for lacking `file_path`).
fn is_search_tool(stdin_body: &str) -> bool {
    serde_json::from_str::<PreToolUseInput>(stdin_body)
        .ok()
        .and_then(|p| p.tool_name)
        .map(|t| t == "Glob" || t == "Grep")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_bare_codebus_lint() {
        assert!(is_codebus_lint_command("codebus lint"));
        assert!(is_codebus_lint_command("codebus lint --format json"));
        assert!(is_codebus_lint_command("codebus lint --repo /some/path"));
    }

    // --- quiz-validate-repair task 3.3: hook also allows
    // `codebus quiz validate ...` (spec lint-feedback-loop / Fix Bash
    // Hook Installation, new allow form). `is_allowed_bash_command` is
    // the combined predicate; `codebus lint *` OR `codebus quiz
    // validate *` is allowed, everything else blocked.

    #[test]
    fn allow_codebus_quiz_validate() {
        assert!(is_allowed_bash_command("codebus quiz validate -"));
        assert!(is_allowed_bash_command(
            "codebus quiz validate draft.md --json"
        ));
        if !cfg!(target_os = "windows") {
            assert!(is_allowed_bash_command(
                "/usr/local/bin/codebus quiz validate draft.md"
            ));
        }
    }

    #[test]
    fn block_codebus_quiz_generate_form() {
        // `codebus quiz "topic"` (generate) is NOT the validate
        // sub-action and MUST stay blocked.
        assert!(!is_allowed_bash_command("codebus quiz topic"));
        assert!(!is_allowed_bash_command("codebus quiz \"some topic\""));
        assert!(!is_allowed_bash_command("codebus quiz"));
    }

    #[test]
    fn combined_predicate_keeps_lint_allow_and_other_block() {
        assert!(is_allowed_bash_command("codebus lint --format json"));
        assert!(!is_allowed_bash_command("codebus fix --no-fix"));
        assert!(!is_allowed_bash_command("echo MARKER"));
        assert!(!is_allowed_bash_command(""));
    }

    #[test]
    fn allow_codebus_via_unix_absolute_path() {
        if !cfg!(target_os = "windows") {
            assert!(is_codebus_lint_command("/usr/local/bin/codebus lint"));
            assert!(is_codebus_lint_command(
                "/home/user/.cargo/bin/codebus lint --format json"
            ));
        }
    }

    #[test]
    fn allow_codebus_exe_via_windows_path() {
        if cfg!(target_os = "windows") {
            assert!(is_codebus_lint_command("D:/dev/codebus.exe lint"));
            assert!(is_codebus_lint_command(
                "D:\\dev\\codebus.exe lint --repo C:\\repo"
            ));
            assert!(is_codebus_lint_command("D:/dev/codebus.EXE lint"));
            assert!(is_codebus_lint_command("D:/dev/Codebus.exe lint"));
        }
    }

    #[test]
    fn block_non_codebus_binary() {
        assert!(!is_codebus_lint_command("echo MARKER"));
        assert!(!is_codebus_lint_command("rm -rf /tmp/x"));
        assert!(!is_codebus_lint_command("git status"));
        assert!(!is_codebus_lint_command("cargo lint"));
        assert!(!is_codebus_lint_command("/usr/bin/echo lint"));
    }

    #[test]
    fn block_codebus_other_subcommands() {
        assert!(!is_codebus_lint_command("codebus init"));
        assert!(!is_codebus_lint_command("codebus goal hello"));
        assert!(!is_codebus_lint_command("codebus fix"));
        assert!(!is_codebus_lint_command("codebus query something"));
        assert!(!is_codebus_lint_command("codebus hook check-bash"));
    }

    #[test]
    fn block_codebus_alone_no_subcommand() {
        // Per spec: argv[1] MUST be exactly `lint` — bare `codebus` is blocked.
        assert!(!is_codebus_lint_command("codebus"));
        assert!(!is_codebus_lint_command("codebus "));
    }

    #[test]
    fn block_lookalikes() {
        // Names that contain `codebus` but aren't exactly the basename.
        assert!(!is_codebus_lint_command("codebusx lint"));
        assert!(!is_codebus_lint_command("xcodebus lint"));
        assert!(!is_codebus_lint_command("codebus-fake lint"));
    }

    #[test]
    fn block_empty_or_whitespace_only_command() {
        assert!(!is_codebus_lint_command(""));
        assert!(!is_codebus_lint_command("   "));
        assert!(!is_codebus_lint_command("\t\n"));
    }

    // --- agent-hook-hardening: shell metacharacter rejection (Fix Bash
    // Hook Installation). `is_allowed_bash_command` rejects any command
    // whose raw string contains a shell metacharacter from the rejection
    // set, even when the leading argv tokens would otherwise satisfy the
    // `codebus lint *` or `codebus quiz validate *` allow form. The
    // metacharacter rejection runs BEFORE argv tokenization and applies
    // regardless of whether the byte is inside double quotes, single
    // quotes, or any other quoting context — the predicate is byte-level
    // on the raw command string.

    #[test]
    fn block_bash_metachar_logical_and() {
        assert!(!is_allowed_bash_command(
            "codebus lint --format json && rm -rf /tmp/evil"
        ));
        assert!(!is_allowed_bash_command(
            "codebus quiz validate draft.md && curl evil.example"
        ));
    }

    #[test]
    fn block_bash_metachar_semicolon() {
        assert!(!is_allowed_bash_command("codebus lint; curl evil.example"));
        assert!(!is_allowed_bash_command(
            "codebus quiz validate -; whoami"
        ));
    }

    #[test]
    fn block_bash_metachar_pipe() {
        assert!(!is_allowed_bash_command(
            "codebus lint | tee /tmp/leak.log"
        ));
        assert!(!is_allowed_bash_command(
            "codebus quiz validate - | grep secret"
        ));
    }

    #[test]
    fn block_bash_metachar_dollar_and_command_substitution() {
        // `$VAR` expansion, `$(cmd)` substitution, and backtick substitution.
        assert!(!is_allowed_bash_command("codebus lint $HOME"));
        assert!(!is_allowed_bash_command("codebus lint $(whoami)"));
        assert!(!is_allowed_bash_command("codebus lint `whoami`"));
    }

    #[test]
    fn block_bash_metachar_redirection() {
        assert!(!is_allowed_bash_command("codebus lint > /tmp/out"));
        assert!(!is_allowed_bash_command("codebus lint < /tmp/in"));
    }

    #[test]
    fn block_bash_metachar_parens() {
        assert!(!is_allowed_bash_command("(codebus lint)"));
        assert!(!is_allowed_bash_command("codebus lint (--bogus)"));
    }

    #[test]
    fn block_bash_metachar_newline_or_carriage_return() {
        // Embedded newline / CR can split into two commands under shell eval.
        assert!(!is_allowed_bash_command("codebus lint\nrm -rf /tmp"));
        assert!(!is_allowed_bash_command("codebus lint\rrm -rf /tmp"));
    }

    #[test]
    fn block_bash_metachar_inside_double_quotes() {
        // Quote-awareness is deliberately NOT implemented — any metachar in
        // the raw command string blocks, even inside double quotes.
        assert!(!is_allowed_bash_command(
            "codebus lint --filter \"foo;bar\""
        ));
        assert!(!is_allowed_bash_command(
            "codebus lint --filter \"foo&&bar\""
        ));
    }

    #[test]
    fn block_bash_metachar_inside_single_quotes() {
        assert!(!is_allowed_bash_command(
            "codebus lint --filter 'foo;bar'"
        ));
        assert!(!is_allowed_bash_command(
            "codebus lint --filter 'foo|bar'"
        ));
    }

    #[test]
    fn allow_bash_command_without_metachar_still_passes() {
        // Regression guard: the rejection set MUST NOT catch the canonical
        // forms used by the codebus-fix / codebus-quiz agents.
        assert!(is_allowed_bash_command("codebus lint"));
        assert!(is_allowed_bash_command("codebus lint --format json"));
        assert!(is_allowed_bash_command(
            "codebus lint --repo /some/safe/path"
        ));
        assert!(is_allowed_bash_command("codebus quiz validate -"));
        assert!(is_allowed_bash_command(
            "codebus quiz validate draft.md --json"
        ));
    }

    // --- quiz-heredoc-selfvalidate-unblock: Quiz-Validate Heredoc Exception
    // (spec lint-feedback-loop / Fix Bash Hook Installation, new
    // `Allow (quiz-validate heredoc)` clause). The claude quiz Mode B agent
    // self-validates by piping its draft into `codebus quiz validate -` via a
    // single-quoted heredoc. The whole heredoc — including the multi-line body
    // and its line feeds — is the raw command string, so the `<` and LF bytes
    // would otherwise trip the metacharacter rejection. `is_allowed_bash_command`
    // recognises the exact single-quoted heredoc shape and allows it, while the
    // body is treated as opaque stdin (NOT scanned). Any deviation (chaining on
    // the first line, a command after the closing delimiter, an unquoted marker,
    // or a non-heredoc input redirection) MUST still block — guarding against an
    // F4 shell-metacharacter bypass regression.

    #[test]
    fn allow_quiz_validate_single_quoted_heredoc() {
        let cmd = "codebus quiz validate - <<'CBQZ'\n\
                   ## Q1. What is a vault?\n\
                   A) a folder\n\
                   B) a database\n\
                   ## Answer: A\n\
                   ## Explanation: see [[vault]].\n\
                   CBQZ";
        assert!(
            is_allowed_bash_command(cmd),
            "well-formed single-quoted quiz-validate heredoc MUST be allowed"
        );
    }

    #[test]
    fn allow_quiz_validate_heredoc_with_json_flag() {
        let cmd = "codebus quiz validate --json - <<'CBQZ'\n\
                   ## Q1. stem\n\
                   ## Answer: A\n\
                   CBQZ";
        assert!(
            is_allowed_bash_command(cmd),
            "the --json variant of the quiz-validate heredoc MUST be allowed"
        );
    }

    #[test]
    fn allow_quiz_validate_heredoc_body_containing_metacharacters() {
        // The body is opaque stdin: line feeds plus `|`, `$`, `;`, `(`, `)`,
        // `>`, backtick inside the body MUST NOT cause a block. This is the
        // case a naive "scan everything but the heredoc operator" approach
        // would wrongly reject.
        let cmd = "codebus quiz validate - <<'CBQZ'\n\
                   ## Q1. In bash, the `|` operator does what, and how do $vars,\n\
                   (subshells), `;` separators, and > redirection differ?\n\
                   A) piping; B) $(cmd) substitution; C) a () group; D) end\n\
                   ## Answer: A\n\
                   CBQZ";
        assert!(
            is_allowed_bash_command(cmd),
            "heredoc body containing shell metacharacters MUST still be allowed (body is opaque stdin)"
        );
    }

    #[test]
    fn block_quiz_validate_heredoc_first_line_chaining_semicolon() {
        // A trailing command after the heredoc operator on the FIRST line.
        let cmd = "codebus quiz validate - <<'X'; rm -rf ~\n\
                   ## Q1. stem\n\
                   X";
        assert!(
            !is_allowed_bash_command(cmd),
            "heredoc with first-line chaining (semicolon) MUST stay blocked (F4 guard)"
        );
    }

    #[test]
    fn block_quiz_validate_heredoc_first_line_chaining_and() {
        let cmd = "codebus quiz validate - <<'X' && curl evil.example\n\
                   ## Q1. stem\n\
                   X";
        assert!(
            !is_allowed_bash_command(cmd),
            "heredoc with first-line chaining (&&) MUST stay blocked (F4 guard)"
        );
    }

    #[test]
    fn block_quiz_validate_heredoc_command_after_closing_delimiter() {
        // The heredoc is well-formed, but a further command follows the
        // closing delimiter line.
        let cmd = "codebus quiz validate - <<'CBQZ'\n\
                   ## Q1. stem\n\
                   CBQZ\n\
                   rm -rf ~";
        assert!(
            !is_allowed_bash_command(cmd),
            "command after the closing delimiter MUST stay blocked (F4 guard)"
        );
    }

    #[test]
    fn block_quiz_validate_heredoc_unquoted_marker() {
        // An unquoted marker permits shell expansion inside the body — the
        // exception MUST NOT apply.
        let cmd = "codebus quiz validate - <<CBQZ\n\
                   ## Q1. stem\n\
                   CBQZ";
        assert!(
            !is_allowed_bash_command(cmd),
            "unquoted heredoc marker MUST stay blocked (expansion-in-body injection guard)"
        );
    }

    #[test]
    fn block_non_heredoc_input_redirection() {
        // A single `<` input redirection is not a here-document.
        assert!(
            !is_allowed_bash_command("codebus quiz validate < ~/.ssh/id_rsa"),
            "non-heredoc input redirection into quiz validate MUST stay blocked"
        );
        assert!(
            !is_allowed_bash_command("codebus lint < /etc/passwd"),
            "non-heredoc input redirection into lint MUST stay blocked"
        );
    }

    #[test]
    fn block_here_string_not_treated_as_heredoc() {
        // `<<<` is a here-string, not a here-document; it MUST NOT qualify.
        let cmd = "codebus quiz validate - <<<'CBQZ'";
        assert!(
            !is_allowed_bash_command(cmd),
            "here-string (<<<) MUST stay blocked"
        );
    }

    #[test]
    fn block_lint_heredoc_not_exempted() {
        // The heredoc exception is scoped to `quiz validate` only; a heredoc
        // fronted by `codebus lint` does NOT qualify and stays blocked.
        let cmd = "codebus lint - <<'CBQZ'\n\
                   payload\n\
                   CBQZ";
        assert!(
            !is_allowed_bash_command(cmd),
            "a `codebus lint` heredoc MUST stay blocked (exception is quiz-validate only)"
        );
    }

    #[test]
    fn block_unterminated_quiz_validate_heredoc() {
        // No closing delimiter line equal to the marker — falls through to the
        // metacharacter rejection.
        let cmd = "codebus quiz validate - <<'CBQZ'\n\
                   ## Q1. stem\n\
                   NOTTHEMARKER";
        assert!(
            !is_allowed_bash_command(cmd),
            "unterminated heredoc (no closing marker line) MUST stay blocked"
        );
    }

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape("hi"), "\"hi\"");
        assert_eq!(json_escape("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_escape("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_escape("line1\nline2"), "\"line1\\nline2\"");
    }

    #[test]
    fn block_emits_valid_decision_json() {
        // We can't easily capture stdout in unit tests, but verify the
        // payload format that emit_block constructs.
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape("test message")
        );
        assert_eq!(
            payload,
            "{\"decision\":\"block\",\"reason\":\"test message\"}"
        );
        // Confirm the payload parses as JSON.
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["decision"], "block");
        assert_eq!(parsed["reason"], "test message");
    }

    #[test]
    fn block_reason_with_command_containing_quotes_stays_valid_json() {
        let cmd_with_quote = r#"echo "hello""#;
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape(&format!(
                "hook: only `codebus lint *` is permitted by codebus fix sandbox; received `{cmd_with_quote}`"
            ))
        );
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["decision"], "block");
        assert!(parsed["reason"].as_str().unwrap().contains("hello"));
    }

    // --- pretooluse-image-block task 1.1 (RED) — is_image_path predicate.
    // Implementation lands in task 1.2; these tests pin the behavior
    // contract from the `PII Image Read Hook Installation` requirement.

    #[test]
    fn is_image_path_blocks_lowercase_extensions() {
        for ext in IMAGE_BLOCKLIST {
            let path = format!("foo.{ext}");
            assert!(is_image_path(&path), "expected block for {path}");
        }
    }

    #[test]
    fn is_image_path_blocks_uppercase_extensions() {
        // ASCII case-insensitive on every platform — Linux too.
        let cases = ["foo.PNG", "foo.JPG", "foo.JPEG", "foo.PDF", "foo.HEIC"];
        for path in cases {
            assert!(is_image_path(path), "expected block for {path}");
        }
    }

    #[test]
    fn is_image_path_blocks_mixed_case_extensions() {
        assert!(is_image_path("photo.Jpeg"));
        assert!(is_image_path("icon.Avif"));
        assert!(is_image_path("doc.PdF"));
    }

    #[test]
    fn is_image_path_allows_text_extensions() {
        let allowed = [
            "foo.md",
            "foo.rs",
            "foo.svg",
            "foo.txt",
            "foo.json",
            "foo.yaml",
            "foo.toml",
        ];
        for path in allowed {
            assert!(!is_image_path(path), "expected allow for {path}");
        }
    }

    #[test]
    fn is_image_path_allows_no_extension() {
        assert!(!is_image_path("Makefile"));
        assert!(!is_image_path("script"));
        assert!(!is_image_path(""));
    }

    #[test]
    fn is_image_path_allows_hidden_files_with_non_image_extension() {
        // `.gitignore` etc. — the leading dot is part of the basename, the
        // extension extractor sees `gitignore` and must not match.
        assert!(!is_image_path(".gitignore"));
        assert!(!is_image_path("/repo/.gitignore"));
    }

    #[test]
    fn is_image_path_handles_unix_path_separator() {
        assert!(is_image_path("/repo/assets/img.png"));
        assert!(is_image_path("./relative/photo.JPG"));
        assert!(!is_image_path("/repo/src/main.rs"));
    }

    #[test]
    fn is_image_path_handles_windows_path_separator() {
        assert!(is_image_path("C:\\repo\\assets\\img.png"));
        assert!(is_image_path("D:\\photos\\IMG_001.HEIC"));
        assert!(!is_image_path("C:\\repo\\src\\main.rs"));
    }

    #[test]
    fn is_image_path_handles_mixed_path_separator() {
        // Windows commonly mixes `/` and `\`.
        assert!(is_image_path("C:/repo\\assets/img.png"));
        assert!(is_image_path("D:\\photos/snapshot.PNG"));
    }

    #[test]
    fn is_image_path_only_considers_basename_extension() {
        // Directory components containing dots SHALL NOT bleed into the
        // extension match; only the filename's final `.ext` counts.
        assert!(!is_image_path("a/b.png/file.md"));
        assert!(!is_image_path("C:\\repo.git\\notes\\daily.md"));
    }

    // --- pretooluse-image-block task 1.3 (RED) — check_read fail-closed
    // contract. Implementation lands in task 1.4; these tests pin the
    // fail-closed branches required by the `PII Image Read Hook
    // Installation` requirement (the subcommand SHALL NEVER silently allow
    // on parse failure or missing fields).

    #[test]
    fn check_read_fail_closed_on_empty_stdin() {
        let reason = check_read_inner("", &HooksConfig::default(), None);
        assert!(reason.is_some(), "empty stdin must block");
        assert!(
            reason.as_ref().unwrap().contains("empty"),
            "block reason must identify empty stdin, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_whitespace_only_stdin() {
        // Mirror check_bash's `buf.trim().is_empty()` semantics.
        assert!(check_read_inner("   \n\t  ", &HooksConfig::default(), None).is_some());
    }

    #[test]
    fn check_read_fail_closed_on_malformed_json() {
        let reason = check_read_inner("{not valid json", &HooksConfig::default(), None);
        assert!(reason.is_some(), "malformed JSON must block");
        assert!(
            reason.as_ref().unwrap().contains("malformed"),
            "block reason must identify malformed JSON, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_missing_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "missing file_path must block");
        assert!(
            reason.as_ref().unwrap().contains("file_path"),
            "block reason must identify file_path absence, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_non_string_file_path() {
        // Numeric file_path — type confusion / fuzzing case.
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":123}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "non-string file_path must block");
    }

    #[test]
    fn check_read_fail_closed_on_null_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":null}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "null file_path must block");
    }

    #[test]
    fn check_read_fail_closed_on_empty_string_file_path() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":""}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "empty-string file_path must block");
        assert!(
            reason.as_ref().unwrap().contains("file_path"),
            "block reason must identify empty file_path, got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fail_closed_on_missing_tool_input() {
        // Whole `tool_input` object missing.
        let body = r#"{"tool_name":"Read"}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "missing tool_input must block");
    }

    // --- pretooluse-image-block task 1.4 — check_read positive contract:
    // image extensions hit the blocklist; non-image extensions pass through.

    #[test]
    fn check_read_blocks_image_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None);
        assert!(reason.is_some(), "image extension must block");
        let reason_str = reason.unwrap();
        assert!(
            reason_str.contains("flow.png"),
            "block reason must echo the file_path, got: {reason_str}"
        );
        assert!(
            reason_str.contains("PII bypass") || reason_str.contains("image"),
            "block reason must surface the policy intent, got: {reason_str}"
        );
    }

    #[test]
    fn check_read_blocks_image_extension_uppercase() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/logo.JPG"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default(), None).is_some());
    }

    #[test]
    fn check_read_blocks_windows_path_image() {
        let body =
            r#"{"tool_name":"Read","tool_input":{"file_path":"C:\\repo\\assets\\img.png"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default(), None).is_some());
    }

    #[test]
    fn check_read_allows_text_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/modules/uv-lib.md"}}"#;
        assert!(
            check_read_inner(body, &HooksConfig::default(), None).is_none(),
            "text file must pass through"
        );
    }

    #[test]
    fn check_read_allows_source_code() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"codebus-core/src/agent/claude_cli.rs"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default(), None).is_none());
    }

    #[test]
    fn check_read_allows_svg() {
        // SVG is XML, scannable by regex_basic — deliberately NOT blocked.
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagram.svg"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default(), None).is_none());
    }

    #[test]
    fn check_read_allows_no_extension() {
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"Makefile"}}"#;
        assert!(check_read_inner(body, &HooksConfig::default(), None).is_none());
    }

    #[test]
    fn check_read_block_reason_is_valid_json_after_emit() {
        // Make sure the block reason survives JSON escaping with a path that
        // contains backslashes (Windows).
        let body =
            r#"{"tool_name":"Read","tool_input":{"file_path":"C:\\repo\\img.png"}}"#;
        let reason = check_read_inner(body, &HooksConfig::default(), None).expect("must block");
        let payload = format!(
            "{{\"decision\":\"block\",\"reason\":{}}}",
            json_escape(&reason)
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&payload).expect("decision payload must parse as JSON");
        assert_eq!(parsed["decision"], "block");
        assert!(
            parsed["reason"]
                .as_str()
                .unwrap()
                .contains("img.png")
        );
    }

    // --- pretooluse-image-block-toggle task 2.1 (RED) ---
    //
    // `check_read_inner` gains a `hooks_cfg: &HooksConfig` parameter.
    // When `hooks_cfg.read_image_block` is false, the function MUST
    // return None for any input (image / non-image / malformed JSON /
    // empty / missing fields); the blocklist + fail-closed branches
    // SHALL NOT execute. When true, behavior matches the unmodified
    // `check_read_inner` contract — exercised by existing tests.

    #[test]
    fn check_read_config_off_allows_image_extension() {
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        assert!(
            check_read_inner(body, &cfg, None).is_none(),
            "read_image_block=false must allow image extensions"
        );
    }

    #[test]
    fn check_read_config_off_allows_uppercase_image() {
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"assets/logo.JPG"}}"#;
        assert!(check_read_inner(body, &cfg, None).is_none());
    }

    #[test]
    fn check_read_config_off_allows_pdf() {
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"docs/manual.pdf"}}"#;
        assert!(check_read_inner(body, &cfg, None).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_empty_stdin() {
        // When the gate is off, even empty stdin (which would normally
        // be fail-closed → block) MUST be allowed — the entire stdin
        // processing branch is short-circuited.
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        assert!(check_read_inner("", &cfg, None).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_malformed_json() {
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        assert!(check_read_inner("{not valid json", &cfg, None).is_none());
    }

    #[test]
    fn check_read_config_off_short_circuits_missing_file_path() {
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{}}"#;
        assert!(check_read_inner(body, &cfg, None).is_none());
    }

    #[test]
    fn check_read_config_on_blocks_image_like_before() {
        // Mirror the existing blocks_image_extension test but with the
        // explicit `read_image_block: true` config; behavior must be
        // identical to the pre-toggle implementation.
        let cfg = HooksConfig {
            read_image_block: true,
            read_path_containment: true,
        };
        let body = r#"{"tool_name":"Read","tool_input":{"file_path":"wiki/diagrams/flow.png"}}"#;
        let reason = check_read_inner(body, &cfg, None);
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("flow.png"));
    }

    #[test]
    fn check_read_config_on_fails_closed_on_empty_stdin_like_before() {
        let cfg = HooksConfig {
            read_image_block: true,
            read_path_containment: true,
        };
        let reason = check_read_inner("", &cfg, None);
        assert!(reason.is_some());
        assert!(reason.as_ref().unwrap().contains("empty"));
    }

    // --- agent-hook-hardening: sensitive path blocklist (PII Image Read
    // Hook Installation). Tests inject a fake home (`/tmp/test-home` on
    // Unix, `C:/Users/poc` on Windows) so the prefix rule is deterministic
    // regardless of the running user's actual home. The basename-glob
    // rule needs no home; home-unresolvable tests pass `None` AND assert
    // fail-closed behavior.

    fn fake_home() -> std::path::PathBuf {
        if cfg!(target_os = "windows") {
            std::path::PathBuf::from("C:/Users/poc")
        } else {
            std::path::PathBuf::from("/tmp/test-home")
        }
    }

    fn body_with_path(path: &str) -> String {
        // Embed `path` into the standard PreToolUse JSON shape. Backslashes
        // in Windows paths MUST be JSON-escaped (`\` → `\\`).
        let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\"tool_name\":\"Read\",\"tool_input\":{{\"file_path\":\"{escaped}\"}}}}"
        )
    }

    #[test]
    fn check_read_blocks_ssh_home_prefix() {
        let home = fake_home();
        let ssh_path = format!("{}/.ssh/config", home.display());
        let body = body_with_path(&ssh_path);
        let reason = check_read_inner(&body, &HooksConfig::default(), Some(&home));
        assert!(
            reason.is_some(),
            "expected block for {ssh_path}; got: {reason:?}"
        );
        assert!(
            reason.as_ref().unwrap().contains("sensitive home directory"),
            "reason SHALL identify the sensitive-path rule; got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_blocks_aws_home_prefix() {
        let home = fake_home();
        let aws_path = format!("{}/.aws/credentials", home.display());
        let body = body_with_path(&aws_path);
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_some()
        );
    }

    #[test]
    fn check_read_blocks_gnupg_home_prefix() {
        let home = fake_home();
        let gnupg_path = format!("{}/.gnupg/pubring.kbx", home.display());
        let body = body_with_path(&gnupg_path);
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_some()
        );
    }

    #[test]
    fn check_read_blocks_gh_cli_config_home_prefix() {
        let home = fake_home();
        let gh_path = format!("{}/.config/gh/hosts.yml", home.display());
        let body = body_with_path(&gh_path);
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_some()
        );
    }

    #[test]
    fn check_read_blocks_tilde_prefixed_path() {
        // `~/.ssh/known_hosts` SHALL be expanded against `home` THEN
        // matched against the sensitive-prefix list.
        let home = fake_home();
        let body = body_with_path("~/.ssh/known_hosts");
        let reason = check_read_inner(&body, &HooksConfig::default(), Some(&home));
        assert!(
            reason.is_some(),
            "tilde-prefixed sensitive path SHALL block; got: {reason:?}"
        );
    }

    #[test]
    fn check_read_blocks_basename_glob_id_rsa_anywhere() {
        // basename glob `*id_rsa*` SHALL hit regardless of directory.
        let body = body_with_path("/tmp/random/extra-id_rsa-backup");
        // No home needed because basename-glob is independent.
        assert!(
            check_read_inner(&body, &HooksConfig::default(), None).is_some()
        );
    }

    #[test]
    fn check_read_blocks_basename_glob_pem_anywhere() {
        let body = body_with_path("/tmp/random/server.pem");
        assert!(
            check_read_inner(&body, &HooksConfig::default(), None).is_some()
        );
    }

    #[test]
    fn check_read_blocks_basename_glob_key_anywhere() {
        let body = body_with_path("/tmp/random/private.key");
        assert!(
            check_read_inner(&body, &HooksConfig::default(), None).is_some()
        );
    }

    #[test]
    fn check_read_allows_home_path_outside_sensitive_dirs() {
        // A path under home but NOT under any sensitive directory SHALL
        // pass through (no false positive).
        let home = fake_home();
        let safe_path = format!("{}/Documents/notes.md", home.display());
        let body = body_with_path(&safe_path);
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_none(),
            "home/Documents/* SHALL allow; got block for {safe_path}"
        );
    }

    #[test]
    fn check_read_blocks_case_insensitively() {
        // The sensitive-prefix match SHALL be case-insensitive (ASCII).
        let home = fake_home();
        let upper_path = format!("{}/.SSH/CONFIG", home.display());
        let body = body_with_path(&upper_path);
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_some()
        );
    }

    #[test]
    fn check_read_fails_closed_when_home_unresolvable_and_path_absolute() {
        // Path that *could* match a sensitive prefix under a resolvable
        // home → SHALL fail-closed when home is None.
        let body = body_with_path("/home/someone/.ssh/config");
        let reason = check_read_inner(&body, &HooksConfig::default(), None);
        assert!(
            reason.is_some(),
            "absolute path under unresolved home SHALL fail-closed; got: {reason:?}"
        );
        assert!(
            reason.as_ref().unwrap().contains("unresolvable"),
            "reason SHALL identify the unresolvable-home rule; got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_fails_closed_on_tilde_path_when_home_unresolvable() {
        let body = body_with_path("~/.aws/credentials");
        let reason = check_read_inner(&body, &HooksConfig::default(), None);
        assert!(
            reason.is_some(),
            "tilde path under unresolved home SHALL fail-closed; got: {reason:?}"
        );
        assert!(reason.as_ref().unwrap().contains("unresolvable"));
    }

    #[test]
    fn check_read_basename_glob_still_decides_when_home_unresolvable() {
        // Even when home is None, the basename-glob rule SHALL fire (the
        // rule does not require home resolution).
        let body = body_with_path("/some/random/server.pem");
        let reason = check_read_inner(&body, &HooksConfig::default(), None);
        assert!(reason.is_some());
        assert!(
            reason.as_ref().unwrap().contains("basename"),
            "basename-glob rule SHALL fire even with no home; got: {}",
            reason.unwrap()
        );
    }

    #[test]
    fn check_read_relative_path_does_not_fail_closed_on_unresolved_home() {
        // A relative path like `wiki/foo.md` does NOT require home
        // resolution (it cannot be home-rooted) AND SHALL pass through
        // even when home is None.
        let body = body_with_path("wiki/concepts/foo.md");
        assert!(
            check_read_inner(&body, &HooksConfig::default(), None).is_none(),
            "relative non-sensitive path SHALL allow even with no home"
        );
    }

    #[test]
    fn check_read_blocks_windows_backslash_ssh_path() {
        // Cross-platform path separator: Windows-style backslash path
        // under home SHALL trigger the same rule as the forward-slash
        // form (separators normalised before prefix comparison).
        let home = std::path::PathBuf::from("C:/Users/poc");
        let body = body_with_path("C:\\Users\\poc\\.ssh\\config");
        assert!(
            check_read_inner(&body, &HooksConfig::default(), Some(&home)).is_some()
        );
    }

    // --- check-read-vault-containment: containment stage tests ---
    // These target `check_containment_inner` directly (the denylist stage is
    // covered by the tests above and is unchanged). The vault root is the
    // PreToolUse `cwd` field (task 1.1, spike-confirmed) with the
    // hook-subprocess cwd as fallback.

    use tempfile::TempDir;

    /// Build a PreToolUse JSON body with an explicit `cwd` (the vault root)
    /// and a single `tool_input` field (`file_path` for Read, `path` for
    /// Glob/Grep). serde_json escapes Windows backslashes correctly.
    fn ct_body(tool: &str, key: &str, path: &str, cwd: &str) -> String {
        let mut ti = serde_json::Map::new();
        ti.insert(key.to_string(), serde_json::Value::String(path.to_string()));
        serde_json::json!({
            "tool_name": tool,
            "cwd": cwd,
            "tool_input": serde_json::Value::Object(ti),
        })
        .to_string()
    }

    fn both_on() -> HooksConfig {
        HooksConfig::default()
    }

    // ---- 3.1 core ----

    /// F1: an absolute Read path that canonicalizes outside the vault root
    /// is blocked by containment.
    #[test]
    fn containment_blocks_out_of_vault_absolute_read() {
        let vault = TempDir::new().unwrap();
        let outside = vault.path().parent().unwrap().join("outside_secret.txt");
        let body = ct_body(
            "Read",
            "file_path",
            outside.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        let r = check_containment_inner(&body, &both_on(), None, None);
        assert!(r.is_some(), "out-of-vault Read must block");
        assert!(r.unwrap().contains("vault-containment"));
    }

    /// An in-vault relative Read path is allowed (resolved against cwd).
    #[test]
    fn containment_allows_in_vault_relative_read() {
        let vault = TempDir::new().unwrap();
        let body = ct_body(
            "Read",
            "file_path",
            "raw/code/src/main.rs",
            vault.path().to_str().unwrap(),
        );
        assert!(check_containment_inner(&body, &both_on(), None, None).is_none());
    }

    /// The fix workflow reads in-vault wiki files via the ABSOLUTE paths
    /// `codebus lint` emits — containment MUST allow these (the
    /// canonicalize-then-contain rule, never ban-absolute).
    #[test]
    fn containment_allows_in_vault_absolute_read_fix_style() {
        let vault = TempDir::new().unwrap();
        let wikidir = vault.path().join("wiki").join("modules");
        std::fs::create_dir_all(&wikidir).unwrap();
        let f = wikidir.join("auth.md");
        std::fs::write(&f, "x").unwrap();
        let body = ct_body(
            "Read",
            "file_path",
            f.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        assert!(
            check_containment_inner(&body, &both_on(), None, None).is_none(),
            "fix's in-vault absolute path must be allowed"
        );
    }

    /// F2: a Grep whose `path` is outside the vault root is blocked.
    #[test]
    fn containment_blocks_out_of_vault_grep_path() {
        let vault = TempDir::new().unwrap();
        let outside = vault.path().parent().unwrap();
        let body = ct_body(
            "Grep",
            "path",
            outside.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        let r = check_containment_inner(&body, &both_on(), None, None);
        assert!(r.is_some(), "out-of-vault Grep must block");
        assert!(r.unwrap().contains("vault-containment"));
    }

    /// Glob/Grep omitting `path` means the implicit search root is the vault
    /// cwd — allowed, NOT failed closed for the absent field.
    #[test]
    fn containment_allows_glob_grep_omitting_path() {
        let vault = TempDir::new().unwrap();
        let cwd = vault.path().to_str().unwrap();
        let grep = serde_json::json!({"tool_name":"Grep","cwd":cwd,"tool_input":{"pattern":"foo"}})
            .to_string();
        assert!(check_containment_inner(&grep, &both_on(), None, None).is_none());
        let glob =
            serde_json::json!({"tool_name":"Glob","cwd":cwd,"tool_input":{"pattern":"**/*.md"}})
                .to_string();
        assert!(check_containment_inner(&glob, &both_on(), None, None).is_none());
    }

    /// The PreToolUse stdin `cwd` is the vault root, taking precedence over
    /// the hook-subprocess cwd fallback. An absolute path under the stdin
    /// cwd but outside the env_cwd MUST be allowed (proving stdin cwd wins).
    #[test]
    fn containment_prefers_stdin_cwd_over_env_cwd() {
        let vault = TempDir::new().unwrap();
        let other = TempDir::new().unwrap();
        let f = vault.path().join("wiki").join("x.md");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "x").unwrap();
        let body = ct_body(
            "Read",
            "file_path",
            f.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        // env_cwd points at `other`; stdin cwd (vault) must govern → allow.
        assert!(check_containment_inner(&body, &both_on(), None, Some(other.path())).is_none());
    }

    /// When the stdin payload omits `cwd`, the hook-subprocess cwd is the
    /// fallback vault root.
    #[test]
    fn containment_falls_back_to_env_cwd_when_cwd_absent() {
        let vault = TempDir::new().unwrap();
        let inside =
            serde_json::json!({"tool_name":"Read","tool_input":{"file_path":"raw/code/x.rs"}})
                .to_string();
        assert!(
            check_containment_inner(&inside, &both_on(), None, Some(vault.path())).is_none(),
            "relative in-vault path resolves against the env_cwd fallback"
        );
        let outside = vault.path().parent().unwrap().join("o.txt");
        let body = serde_json::json!({"tool_name":"Read","tool_input":{"file_path":outside.to_str().unwrap()}})
            .to_string();
        assert!(
            check_containment_inner(&body, &both_on(), None, Some(vault.path())).is_some(),
            "out-of-vault absolute path blocks against the env_cwd fallback"
        );
    }

    // ---- 3.2 edges ----

    /// `read_path_containment: false` disables the boundary (escape hatch).
    #[test]
    fn containment_disabled_skips_boundary() {
        let vault = TempDir::new().unwrap();
        let outside = vault.path().parent().unwrap().join("o.txt");
        let body = ct_body(
            "Read",
            "file_path",
            outside.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        let cfg = HooksConfig {
            read_image_block: true,
            read_path_containment: false,
        };
        assert!(
            check_containment_inner(&body, &cfg, None, None).is_none(),
            "containment off → no boundary block"
        );
    }

    /// The two gates are independent: with `read_image_block: false` but
    /// `read_path_containment: true`, an out-of-vault path is still blocked.
    #[test]
    fn containment_independent_of_read_image_block() {
        let vault = TempDir::new().unwrap();
        let outside = vault.path().parent().unwrap().join("o.txt");
        let body = ct_body(
            "Read",
            "file_path",
            outside.to_str().unwrap(),
            vault.path().to_str().unwrap(),
        );
        let cfg = HooksConfig {
            read_image_block: false,
            read_path_containment: true,
        };
        assert!(check_containment_inner(&body, &cfg, None, None).is_some());
    }

    /// With a target path present but no resolvable vault root (no `cwd`,
    /// no env_cwd), containment fails closed.
    #[test]
    fn containment_vault_root_unresolvable_blocks() {
        let body =
            serde_json::json!({"tool_name":"Read","tool_input":{"file_path":"/abs/x.txt"}})
                .to_string();
        let r = check_containment_inner(&body, &both_on(), None, None);
        assert!(r.is_some());
        assert!(r.unwrap().contains("unresolvable"));
    }

    /// Empty / malformed stdin fails closed while containment is on.
    #[test]
    fn containment_empty_and_malformed_stdin_block() {
        assert!(check_containment_inner("", &both_on(), None, None).is_some());
        assert!(check_containment_inner("{bad json", &both_on(), None, None).is_some());
    }

    /// Windows: an in-vault target expressed with backslash separators AND a
    /// differently-cased drive letter than the cwd is still recognized as
    /// in-vault (both operands normalize under one canonicalization).
    #[cfg(windows)]
    #[test]
    fn containment_windows_separator_and_drive_case_in_vault_allows() {
        let vault = TempDir::new().unwrap();
        let f = vault.path().join("wiki").join("x.md");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "x").unwrap();
        // target: backslash form; cwd: forward-slash form — deliberate mismatch.
        let target_bs = f.to_string_lossy().replace('/', "\\");
        let cwd_fs = vault.path().to_string_lossy().replace('\\', "/");
        let body = ct_body("Read", "file_path", &target_bs, &cwd_fs);
        assert!(
            check_containment_inner(&body, &both_on(), None, None).is_none(),
            "in-vault path must allow despite separator / drive-case variance"
        );
    }
}
