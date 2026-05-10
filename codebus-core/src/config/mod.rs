//! Global config loaded from `~/.codebus/config.yaml`.
//!
//! v3-config surface:
//! - `lint.fix.*` — fix-loop config (see [`lint_fix`])
//! - `pii.*` — PII scanner config (see [`pii`])
//! - `claude_code.*` — per-verb agent model/effort (see [`claude_code`])
//!
//! Each sub-module owns its own `Default`, loader, and forward-compat
//! tolerance (missing file / missing section / missing field / unknown
//! key). `ConfigLoadError` is defined here so all loaders share a single
//! error type.
//!
//! `default_config_path()` resolves to `<home>/.codebus/config.yaml`.

pub mod claude_code;
pub mod global_starter;
pub mod lint_fix;
pub mod pii;

pub use claude_code::{ClaudeCodeConfig, VerbAgentConfig, load_claude_code_config};
pub use global_starter::{StarterOutcome, write_starter_config_if_missing};
pub use lint_fix::{LintFixConfig, load_lint_fix_config};
pub use pii::{PiiConfig, PiiScannerKind, load_pii_config};

use std::path::PathBuf;

/// Default config path: `~/.codebus/config.yaml`. Returns `None` if the
/// home directory cannot be resolved.
///
/// Honors the `CODEBUS_HOME` env var when set non-empty — useful as a CI /
/// container relocation knob and (importantly) as a clean test hook on
/// Windows, where `dirs::home_dir()` ignores `HOME` / `USERPROFILE`
/// overrides because it consults `SHGetKnownFolderPath`. v2 carry pattern.
pub fn default_config_path() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CODEBUS_HOME") {
        if !custom.is_empty() {
            return Some(PathBuf::from(custom).join(".codebus").join("config.yaml"));
        }
    }
    dirs::home_dir().map(|h| h.join(".codebus").join("config.yaml"))
}

/// Shared error type for all config loaders. `Io` is reserved for read
/// failures other than `NotFound` (callers translate `NotFound` to default
/// internally). `YamlParse` covers structural and discriminator-mismatch
/// errors raised by `serde_yaml`.
#[derive(Debug)]
pub enum ConfigLoadError {
    Io(std::io::Error),
    YamlParse(serde_yaml::Error),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLoadError::Io(e) => write!(f, "config file io: {e}"),
            ConfigLoadError::YamlParse(e) => write!(f, "config file yaml parse: {e}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}
