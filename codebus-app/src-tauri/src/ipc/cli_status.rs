//! `check_cli_installed` IPC command — probes whether an agentic CLI
//! binary is installed and reachable from the app process.
//!
//! Frontend uses this to render the Settings "CLI status" row(s) so the
//! user knows whether `claude` (or future `codex` / `gemini-cli`) is
//! installed before configuring its endpoint. Replaces the v1
//! "Authentication" pseudo-status that just rendered a static label.
//!
//! Probe mechanism: spawn `<binary> --version` blocking on a thread.
//! Success (exit 0 + non-empty stdout) → installed + version string;
//! anything else (binary missing / non-zero exit) → not installed. The
//! probe is best-effort — false negatives are acceptable and surface to
//! the user as "not installed".

use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::IpcResult;

/// Reply shape — discriminated union mirroring the `KeyStatus` pattern
/// so the frontend pattern-matches on `kind` for installed / not installed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliStatus {
    Installed { version: String },
    NotInstalled,
}

#[tauri::command]
pub async fn check_cli_installed(provider: String) -> IpcResult<CliStatus> {
    // Unknown provider → not_installed (never an error to the frontend).
    let binary = match binary_for_provider(&provider) {
        Some(b) => b.to_string(),
        None => return Ok(CliStatus::NotInstalled),
    };
    // Run blocking process spawn on a worker thread so the Tauri async
    // runtime stays responsive while `<binary> --version` initialises.
    let status = tauri::async_runtime::spawn_blocking(move || probe_binary(&binary))
        .await
        .unwrap_or(CliStatus::NotInstalled);
    Ok(status)
}

/// Map a provider id (the frontend `cliBinaryId`) to its CLI binary name.
/// `None` for an unknown provider — the caller collapses that to
/// `not_installed`. Add a future provider by extending this match.
pub(crate) fn binary_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "claude_code" => Some("claude"),
        "codex" => Some("codex"),
        _ => None,
    }
}

/// Run `<binary> --version` synchronously. Any failure path returns
/// `NotInstalled` — we do NOT surface the underlying error to the user
/// because "binary missing" / "binary returned non-zero" all mean the
/// same thing to a Settings user: "you don't have it set up."
pub(crate) fn probe_binary(binary: &str) -> CliStatus {
    let output = match version_command(binary)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(out) => out,
        Err(_) => return CliStatus::NotInstalled,
    };
    if !output.status.success() {
        return CliStatus::NotInstalled;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return CliStatus::NotInstalled;
    }
    CliStatus::Installed { version }
}

/// Build the `<binary> --version` probe command. On Windows, npm-installed
/// CLIs (e.g. `codex` → `codex.cmd`) are batch shims that `Command::new`
/// does NOT resolve via `PATHEXT`, so route through `cmd /C` which does.
/// Native binaries (`claude.exe`) resolve fine through `cmd /C` too, so the
/// Windows path is uniform.
#[cfg(windows)]
fn version_command(binary: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(binary).arg("--version");
    cmd
}

#[cfg(not(windows))]
fn version_command(binary: &str) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("--version");
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec (codex-settings-ui): provider → binary mapping is total over the
    /// legal set; unknown → None.
    #[test]
    fn binary_for_provider_maps_claude_and_codex() {
        assert_eq!(binary_for_provider("claude_code"), Some("claude"));
        assert_eq!(binary_for_provider("codex"), Some("codex"));
        assert_eq!(binary_for_provider("gemini_cli"), None);
    }

    /// `codex` is supported — the probe never returns an error (installed /
    /// not_installed only).
    #[test]
    fn codex_is_a_supported_provider() {
        let result = tauri::async_runtime::block_on(check_cli_installed("codex".into()));
        assert!(result.is_ok(), "codex provider must not error: {result:?}");
    }

    /// A provider outside the legal set collapses to not_installed.
    #[test]
    fn unknown_provider_collapses_to_not_installed() {
        let result = tauri::async_runtime::block_on(check_cli_installed("gemini_cli".into()));
        assert_eq!(result.unwrap(), CliStatus::NotInstalled);
    }

    /// Probing a definitely-not-there binary returns `NotInstalled`.
    #[test]
    fn probe_missing_binary_returns_not_installed() {
        let status = probe_binary("codebus-definitely-not-a-real-binary-xyz123");
        assert_eq!(status, CliStatus::NotInstalled);
    }
}
