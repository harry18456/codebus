//! `codebus config` subcommand — manage the Azure API key in the OS
//! keyring.
//!
//! Three sub-actions: `set-key <profile>`, `get-key <profile> [--show]`,
//! `delete-key <profile>`. The `<profile>` argument is a clap-validated
//! enum with currently a single legal value (`azure`); future endpoint
//! types extend the enum here AND extend the keyring service-name
//! resolver below.
//!
//! Spec: `claude-code-config / Config Subcommand For Keyring Management`.

use std::io::{self, Read};
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};

use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};
use codebus_core::config::{ClaudeCodeConfig, default_config_path, load_claude_code_config};

/// Default keyring service when the user has not configured an azure
/// profile yet. Production callers SHOULD set `claude_code.azure.keyring_service`
/// explicitly; this default exists so `codebus config set-key azure`
/// works on a fresh install before the user has edited config.yaml.
const DEFAULT_AZURE_KEYRING_SERVICE: &str = "codebus-claude-azure";

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Read a key from stdin (echo disabled) and write it to the OS
    /// keyring entry for the given profile.
    SetKey(SetKeyArgs),
    /// Report whether the keyring entry exists. `--show` additionally
    /// prints the key value verbatim.
    GetKey(GetKeyArgs),
    /// Remove the keyring entry. Idempotent — exits 0 whether or not
    /// the entry existed.
    DeleteKey(DeleteKeyArgs),
}

#[derive(Debug, Args)]
pub struct SetKeyArgs {
    /// Endpoint profile to set the key for. Only `azure` is supported
    /// right now; other endpoint types arrive as separate changes.
    pub profile: Profile,
}

#[derive(Debug, Args)]
pub struct GetKeyArgs {
    pub profile: Profile,
    /// Print the key value verbatim instead of just `set` / `unset`.
    #[arg(long)]
    pub show: bool,
}

#[derive(Debug, Args)]
pub struct DeleteKeyArgs {
    pub profile: Profile,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Profile {
    Azure,
}

pub async fn run(args: ConfigArgs) -> ExitCode {
    match args.action {
        ConfigAction::SetKey(a) => run_set_key(a),
        ConfigAction::GetKey(a) => run_get_key(a),
        ConfigAction::DeleteKey(a) => run_delete_key(a),
    }
}

fn run_set_key(args: SetKeyArgs) -> ExitCode {
    let service = match resolve_keyring_service(args.profile) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let key = match read_stdin_key() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: read key from stdin: {e}");
            return ExitCode::from(1);
        }
    };
    if key.is_empty() {
        eprintln!("error: empty key rejected");
        return ExitCode::from(1);
    }
    match store_azure_key(&service, &key) {
        Ok(()) => {
            println!("key stored");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: store key in keyring: {e}");
            eprintln!(
                "hint: if your OS does not provide a keyring backend (headless Linux, CI, sandboxes), \
                export CODEBUS_AZURE_KEY=<your-key> instead — codebus reads that env var as a fallback."
            );
            ExitCode::from(1)
        }
    }
}

fn run_get_key(args: GetKeyArgs) -> ExitCode {
    let service = match resolve_keyring_service(args.profile) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match probe_keyring_only(&service) {
        Ok(Some(value)) => {
            if args.show {
                println!("{value}");
            } else {
                println!("set");
            }
            ExitCode::SUCCESS
        }
        Ok(None) => {
            println!("unset");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: probe keyring: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_delete_key(args: DeleteKeyArgs) -> ExitCode {
    let service = match resolve_keyring_service(args.profile) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match delete_azure_key(&service) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: delete keyring entry: {e}");
            ExitCode::from(1)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_keyring_service(profile: Profile) -> Result<String, ExitCode> {
    match profile {
        Profile::Azure => read_azure_keyring_service_from_config(),
    }
}

/// Resolve `claude_code.azure.keyring_service` from `~/.codebus/config.yaml`.
///
/// Three outcomes per spec `cli / Config Parse Failure Aborts Invocation`:
///
/// - Config file absent OR `azure` block missing → return the default
///   service name (first-time setup: the user has not yet picked a
///   custom keyring service, so the well-known default is correct).
/// - Config file exists AND parses cleanly AND has a populated
///   `azure.keyring_service` → return that value.
/// - Config file exists AND fails to parse → emit a stderr error AND
///   return `Err(ExitCode::from(2))`. **No silent fallback** — that was
///   the bug that caused the wrong keyring entry to be deleted in the
///   prior `claude-code-endpoint-profiles` apply session.
fn read_azure_keyring_service_from_config() -> Result<String, ExitCode> {
    let path = match default_config_path() {
        Some(p) => p,
        // No home directory resolvable → behave as if no config exists.
        // Fresh-setup default is correct here.
        None => return Ok(DEFAULT_AZURE_KEYRING_SERVICE.to_string()),
    };
    let cfg: ClaudeCodeConfig = match load_claude_code_config(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "error: claude_code config parse failed at {}: {e}",
                path.display()
            );
            return Err(ExitCode::from(2));
        }
    };
    if let Some(az) = cfg.azure.as_ref() {
        if !az.keyring_service.is_empty() {
            return Ok(az.keyring_service.clone());
        }
    }
    Ok(DEFAULT_AZURE_KEYRING_SERVICE.to_string())
}

fn read_stdin_key() -> io::Result<String> {
    if is_stdin_terminal() {
        // Echo-disabled prompt for interactive sessions.
        rpassword::prompt_password("Enter API key: ").map(|s| s.trim().to_string())
    } else {
        // Pipe / file input: read until EOF (used by integration tests).
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        // Drop trailing newlines so `echo "key" | codebus config set-key
        // azure` works without the user remembering to use `printf`.
        while buf.ends_with('\n') || buf.ends_with('\r') {
            buf.pop();
        }
        Ok(buf)
    }
}

fn is_stdin_terminal() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Config absent → resolver returns the claude default service name.
    /// Hermetic: point `CODEBUS_HOME` at a fresh temp dir with no config so
    /// the result does not depend on the dev's real `~/.codebus`.
    #[test]
    fn resolve_keyring_service_returns_claude_default_when_no_config() {
        let dir = std::env::temp_dir().join(format!("codebus-cli-cfg-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let prev = std::env::var("CODEBUS_HOME").ok();
        unsafe {
            std::env::set_var("CODEBUS_HOME", &dir);
        }
        let resolved = resolve_keyring_service(Profile::Azure).unwrap_or_else(|_| "ERR".into());
        unsafe {
            match prev {
                Some(v) => std::env::set_var("CODEBUS_HOME", v),
                None => std::env::remove_var("CODEBUS_HOME"),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(resolved, DEFAULT_AZURE_KEYRING_SERVICE);
        assert_eq!(DEFAULT_AZURE_KEYRING_SERVICE, "codebus-claude-azure");
    }
}
