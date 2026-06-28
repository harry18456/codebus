//! `mcp_client_status` / `mcp_client_install` / `mcp_client_remove` IPC commands
//! — register codebus as a **user-scope** MCP server in an agent client (claude
//! / codex) by shelling out to that client's own native CLI.
//!
//! Per spec `mcp-client-install`: argv-array shell-out (never a shell string),
//! the bundled codebus CLI's ABSOLUTE path (not bare `codebus`), claude gets
//! `--scope user` (its default scope is project-local), codex gets none. The
//! app NEVER parses or rewrites the client's config files. Detection reuses the
//! existing `<bin> --version` probe ([`super::cli_status`]); the binary invoked
//! for `mcp add/remove/list` reuses the agent backend's resolution
//! (`CODEBUS_*_BIN` override, else the platform default — `codex.cmd` on
//! Windows). A non-zero client exit surfaces as `AppError::Io` with the stderr
//! tail; a missing client is never an error.

use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::IpcResult;
use super::cli_status::{CliStatus, binary_for_provider, probe_binary};
use super::global_md;
use crate::error::AppError;

/// Registration status of codebus in a given client. Mirrors the `CliStatus`
/// discriminated-union pattern so the frontend pattern-matches on `kind`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpClientStatus {
    /// Client present AND a `codebus` MCP entry is registered.
    Installed,
    /// Client present but codebus is not registered.
    NotRegistered,
    /// Client CLI not detected (or an unknown provider).
    ClientMissing,
}

/// Supported MCP clients. Parsing the `provider` literal up front keeps the
/// rest of the module off stringly-typed control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Client {
    Claude,
    Codex,
}

fn parse_client(provider: &str) -> Option<Client> {
    match provider {
        "claude_code" => Some(Client::Claude),
        "codex" => Some(Client::Codex),
        _ => None,
    }
}

/// The client CLI binary to invoke for `mcp add/remove/list`, resolved by the
/// SAME rule the agent backend uses (so detection and invocation agree on the
/// binary — notably `codex.cmd` on Windows).
fn client_bin(client: Client) -> String {
    match client {
        Client::Claude => codebus_core::agent::claude_backend::claude_bin(),
        Client::Codex => codebus_core::agent::codex_backend::codex_bin(),
    }
}

/// argv (after the client binary) for `mcp add`. claude pins `--scope user`
/// (its default is project-local); codex has no scope. The codebus CLI path is
/// passed after `--` as a SINGLE argv element (spaces safe, no shell parsing).
fn install_args(client: Client, codebus: &str) -> Vec<String> {
    match client {
        Client::Claude => vec![
            "mcp".into(),
            "add".into(),
            "--scope".into(),
            "user".into(),
            "codebus".into(),
            "--".into(),
            codebus.into(),
            "mcp".into(),
        ],
        Client::Codex => vec![
            "mcp".into(),
            "add".into(),
            "codebus".into(),
            "--".into(),
            codebus.into(),
            "mcp".into(),
        ],
    }
}

/// argv (after the client binary) for `mcp remove`.
fn remove_args(client: Client) -> Vec<String> {
    match client {
        Client::Claude => vec![
            "mcp".into(),
            "remove".into(),
            "--scope".into(),
            "user".into(),
            "codebus".into(),
        ],
        Client::Codex => vec!["mcp".into(), "remove".into(), "codebus".into()],
    }
}

/// Map a finished client command to an IPC result: success → `Ok`, non-zero
/// exit → `AppError::Io` carrying the trimmed stderr tail.
fn map_exit(success: bool, stderr: &[u8]) -> IpcResult<()> {
    if success {
        return Ok(());
    }
    let tail = String::from_utf8_lossy(stderr);
    let tail = tail.trim();
    Err(AppError::io(if tail.is_empty() {
        "client command exited non-zero"
    } else {
        tail
    }))
}

/// Whether a `mcp list` listing names a `codebus` server entry. Token-matched
/// (word boundaries) so an unrelated substring cannot false-positive.
fn listing_has_codebus(listing: &str) -> bool {
    listing.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).any(|tok| tok == "codebus")
}

/// Build a `<bin> <args...>` command with stdio captured and (on Windows) the
/// console window suppressed.
fn client_command(bin: &str, args: &[String]) -> Command {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    codebus_core::win_console::hide_console(&mut cmd);
    cmd
}

/// Resolve the ABSOLUTE path of the bundled codebus CLI. Packaged builds bundle
/// it as the `bin/codebus.exe` resource. In a dev build (no bundled resource)
/// fall back to the CLI sibling of the running app exe (workspace
/// `target/debug/`), then to the bare name as a last resort. The fallback is
/// dev-only — packaged behavior is unchanged.
fn resolve_codebus_path(app: &AppHandle) -> PathBuf {
    if let Ok(p) = app
        .path()
        .resolve(cli_resource_rel(), tauri::path::BaseDirectory::Resource)
        && p.exists()
    {
        return p;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(cli_exe_name());
        if sibling.exists() {
            return sibling;
        }
    }
    PathBuf::from(cli_exe_name())
}

fn cli_resource_rel() -> &'static str {
    if cfg!(windows) {
        "bin/codebus.exe"
    } else {
        "bin/codebus"
    }
}

fn cli_exe_name() -> &'static str {
    if cfg!(windows) {
        "codebus.exe"
    } else {
        "codebus"
    }
}

/// The client's GLOBAL instruction file (claude `CLAUDE.md`, codex `AGENTS.md`),
/// resolved by [`global_md`] honoring `CLAUDE_CONFIG_DIR` / `CODEX_HOME`.
fn global_md_path(client: Client) -> Option<PathBuf> {
    match client {
        Client::Claude => global_md::claude_md_path(),
        Client::Codex => global_md::codex_md_path(),
    }
}

/// Keep the codebus guidance block in the resolved global instruction file in
/// sync with the registration: upsert on enable, remove on disable. Best-effort
/// and SUBORDINATE to the MCP registration — any failure (including an
/// unresolvable home directory) warns to stderr and is swallowed, so it can
/// never fail the install/remove IPC. The atomic write in [`global_md`] keeps a
/// failed write from corrupting the file.
fn sync_guidance_at(path: Option<PathBuf>, enabled: bool) {
    let Some(path) = path else {
        eprintln!("warning: codebus mcp guidance: home directory unavailable, skipping");
        return;
    };
    let res = if enabled {
        global_md::upsert_block_at(&path)
    } else {
        global_md::remove_block_at(&path)
    };
    if let Err(e) = res {
        eprintln!(
            "warning: codebus mcp guidance: failed to update {}: {e}",
            path.display()
        );
    }
}

fn sync_guidance(client: Client, enabled: bool) {
    sync_guidance_at(global_md_path(client), enabled);
}

/// Compute the registration status: detect the client via the shared
/// `--version` probe, then query its own `mcp list` for a codebus entry.
fn compute_status(provider: &str) -> McpClientStatus {
    let Some(client) = parse_client(provider) else {
        return McpClientStatus::ClientMissing;
    };
    // Detection reuses the `<bin> --version` probe (the Settings CLI-status row).
    let Some(probe_bin) = binary_for_provider(provider) else {
        return McpClientStatus::ClientMissing;
    };
    if probe_binary(probe_bin) == CliStatus::NotInstalled {
        return McpClientStatus::ClientMissing;
    }
    // Registration state from the client's own `mcp list`.
    match client_command(&client_bin(client), &["mcp".into(), "list".into()]).output() {
        Ok(out) => {
            let listing = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            if listing_has_codebus(&listing) {
                McpClientStatus::Installed
            } else {
                McpClientStatus::NotRegistered
            }
        }
        Err(_) => McpClientStatus::ClientMissing,
    }
}

#[tauri::command]
pub async fn mcp_client_status(provider: String) -> IpcResult<McpClientStatus> {
    let status = tauri::async_runtime::spawn_blocking(move || compute_status(&provider))
        .await
        .unwrap_or(McpClientStatus::ClientMissing);
    Ok(status)
}

#[tauri::command]
pub async fn mcp_client_install(app: AppHandle, provider: String) -> IpcResult<()> {
    let Some(client) = parse_client(&provider) else {
        return Err(AppError::io("unknown MCP client provider"));
    };
    let codebus = resolve_codebus_path(&app);
    let args = install_args(client, &codebus.to_string_lossy());
    run_client(client_bin(client), args).await?;
    // Subordinate to the registration above: add the global-md guidance block.
    // Non-fatal — a failure warns but does not undo the registration.
    sync_guidance(client, true);
    Ok(())
}

#[tauri::command]
pub async fn mcp_client_remove(provider: String) -> IpcResult<()> {
    let Some(client) = parse_client(&provider) else {
        return Err(AppError::io("unknown MCP client provider"));
    };
    run_client(client_bin(client), remove_args(client)).await?;
    // Subordinate to the unregistration above: remove the guidance block.
    sync_guidance(client, false);
    Ok(())
}

/// Spawn the client command on a blocking worker, then map its exit status.
async fn run_client(bin: String, args: Vec<String>) -> IpcResult<()> {
    let output = tauri::async_runtime::spawn_blocking(move || client_command(&bin, &args).output())
        .await
        .map_err(|e| AppError::internal(format!("join error: {e}")))?;
    match output {
        Ok(out) => map_exit(out.status.success(), &out.stderr),
        Err(e) => Err(AppError::io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn provider_parsing_is_closed_over_the_two_clients() {
        assert_eq!(parse_client("claude_code"), Some(Client::Claude));
        assert_eq!(parse_client("codex"), Some(Client::Codex));
        assert_eq!(parse_client("gemini_cli"), None);
    }

    #[test]
    fn claude_install_args_pin_user_scope_and_pass_path_as_one_arg() {
        let args = install_args(Client::Claude, r"C:\Program Files\codebus\bin\codebus.exe");
        // --scope user is mandatory (claude default scope is project-local).
        let scope = args.iter().position(|a| a == "--scope").expect("--scope present");
        assert_eq!(args[scope + 1], "user");
        // The path is a single argv element after `--`, spaces intact.
        let dashes = args.iter().position(|a| a == "--").expect("`--` separator");
        assert_eq!(args[dashes + 1], r"C:\Program Files\codebus\bin\codebus.exe");
        assert_eq!(args[dashes + 2], "mcp");
        assert_eq!(args.first().map(String::as_str), Some("mcp"));
        assert!(args.contains(&"add".to_string()));
        assert!(args.contains(&"codebus".to_string()));
    }

    #[test]
    fn codex_install_args_omit_scope() {
        let args = install_args(Client::Codex, "/abs/codebus");
        assert!(!args.iter().any(|a| a == "--scope"), "codex has no scope: {args:?}");
        let dashes = args.iter().position(|a| a == "--").expect("`--` separator");
        assert_eq!(args[dashes + 1], "/abs/codebus");
        assert_eq!(args[dashes + 2], "mcp");
        assert_eq!(&args[..4], &["mcp", "add", "codebus", "--"]);
    }

    #[test]
    fn remove_args_are_symmetric_to_install() {
        assert_eq!(
            remove_args(Client::Claude),
            vec!["mcp", "remove", "--scope", "user", "codebus"]
        );
        assert_eq!(remove_args(Client::Codex), vec!["mcp", "remove", "codebus"]);
    }

    #[test]
    fn non_zero_exit_maps_to_io_error_with_stderr_tail() {
        let err = map_exit(false, b"  codebus mcp add: connection refused\n").unwrap_err();
        let AppError::Io { message } = err else {
            panic!("expected Io variant");
        };
        assert!(message.contains("connection refused"), "stderr tail kept: {message}");
        // Empty stderr still yields a non-empty message.
        let err = map_exit(false, b"").unwrap_err();
        assert!(matches!(err, AppError::Io { .. }));
        // Success → Ok.
        assert!(map_exit(true, b"").is_ok());
    }

    #[test]
    fn listing_detection_is_token_based() {
        assert!(listing_has_codebus("codebus: codebus mcp"));
        assert!(listing_has_codebus("  codebus    ✓ connected"));
        assert!(!listing_has_codebus("codebus-other: something"));
        assert!(!listing_has_codebus("no entries"));
    }

    #[test]
    fn cli_names_are_platform_specific() {
        if cfg!(windows) {
            assert_eq!(cli_exe_name(), "codebus.exe");
            assert_eq!(cli_resource_rel(), "bin/codebus.exe");
        } else {
            assert_eq!(cli_exe_name(), "codebus");
            assert_eq!(cli_resource_rel(), "bin/codebus");
        }
    }

    /// mcp-usage-guidance: enable adds the guidance block to the client's global
    /// instruction file, disable removes it, hand-written content survives both.
    /// Drives the guidance path directly (no client spawn, no env mutation).
    #[test]
    fn sync_guidance_at_upserts_then_removes_block() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("CLAUDE.md");
        std::fs::write(&path, "# my rules\n").unwrap();

        sync_guidance_at(Some(path.clone()), true);
        let after_enable = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_enable.contains("codebus:mcp:start"),
            "enable must add the guidance block: {after_enable}"
        );
        assert!(after_enable.contains("# my rules"), "hand-written content kept on enable");

        sync_guidance_at(Some(path.clone()), false);
        let after_disable = std::fs::read_to_string(&path).unwrap();
        assert!(
            !after_disable.contains("codebus:mcp:start"),
            "disable must remove the guidance block: {after_disable}"
        );
        assert!(after_disable.contains("# my rules"), "hand-written content kept on disable");
    }

    /// An unresolvable home directory is best-effort: it warns and is swallowed,
    /// never panicking — so it can never fail the install/remove IPC.
    #[test]
    fn sync_guidance_at_none_is_noop_not_panic() {
        sync_guidance_at(None, true);
        sync_guidance_at(None, false);
    }
}
