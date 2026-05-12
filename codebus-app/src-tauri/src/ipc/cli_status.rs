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
use crate::error::AppError;

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
    let binary = match provider.as_str() {
        "claude_code" => "claude",
        other => {
            return Err(AppError::Invalid {
                field: "provider".into(),
                message: format!(
                    "unknown agentic provider `{other}`; only `claude_code` is supported"
                ),
            });
        }
    };
    // Run blocking process spawn on a worker thread so the Tauri async
    // runtime stays responsive while `<binary> --version` initialises.
    let binary = binary.to_string();
    let status = tauri::async_runtime::spawn_blocking(move || probe_binary(&binary))
        .await
        .unwrap_or(CliStatus::NotInstalled);
    Ok(status)
}

/// Run `<binary> --version` synchronously. Any failure path returns
/// `NotInstalled` — we do NOT surface the underlying error to the user
/// because "binary missing" / "binary returned non-zero" all mean the
/// same thing to a Settings user: "you don't have it set up."
pub(crate) fn probe_binary(binary: &str) -> CliStatus {
    let output = match Command::new(binary)
        .arg("--version")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec: only `claude_code` is supported; anything else SHALL be
    /// rejected with `AppError::Invalid { field: "provider" }`.
    #[test]
    fn unknown_provider_rejected_sync_probe() {
        // We exercise the sync helper directly — async command path
        // routes through the same `provider` match arm.
        let err = match tauri::async_runtime::block_on(check_cli_installed("codex".into())) {
            Err(e) => e,
            Ok(_) => panic!("expected Err for unknown provider"),
        };
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "provider"));
    }

    /// Probing a definitely-not-there binary returns `NotInstalled`.
    #[test]
    fn probe_missing_binary_returns_not_installed() {
        let status = probe_binary("codebus-definitely-not-a-real-binary-xyz123");
        assert_eq!(status, CliStatus::NotInstalled);
    }
}
