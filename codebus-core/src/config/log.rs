//! `log.*` config loader for v3-run-log Log Configuration Schema.
//!
//! Schema:
//! ```yaml
//! # ~/.codebus/config.yaml
//! log:
//!   sink: jsonl    # or "null" (quoted!) to opt out
//!   dir: ~/path    # optional override; default <vault>/.codebus/log/
//! ```
//!
//! All fields optional. Missing file / missing section / missing field all
//! fall through to default `Jsonl { dir: None }`. Unknown keys silently
//! ignored. Unknown sink discriminator (e.g. `sink: otel`) → parse Err →
//! caller stderr-warns + uses default.
//!
//! `sink: null` (bare YAML null literal) falls through to default just like
//! `pii.scanner: null` — same foot-gun avoidance pattern. Users opt out via
//! the quoted string `sink: "null"`.

use crate::log::SinkConfig;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogConfig {
    pub sink: SinkConfig,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            sink: SinkConfig::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    log: Option<SinkConfig>,
}

/// Load `log.*` config from `path`. Returns defaults when the file does not
/// exist OR the `log` section is absent. Returns `Err` only when the file
/// exists but cannot be read or is structurally invalid YAML — callers
/// SHALL fall back to defaults on `Err` after a stderr warning.
pub fn load_log_config(path: &Path) -> Result<LogConfig, super::ConfigLoadError> {
    let body = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LogConfig::default());
        }
        Err(err) => return Err(super::ConfigLoadError::Io(err)),
    };
    let file: ConfigFile = serde_yaml::from_str(&body).map_err(super::ConfigLoadError::YamlParse)?;
    let mut cfg = LogConfig::default();
    if let Some(sink) = file.log {
        cfg.sink = sink;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write_yaml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("config.yaml");
        fs::write(&p, body).unwrap();
        p
    }

    /// Spec: "Default config when file missing"
    #[test]
    fn default_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let cfg = load_log_config(&tmp.path().join("nope.yaml")).unwrap();
        assert_eq!(cfg, LogConfig::default());
        assert_eq!(cfg.sink, SinkConfig::Jsonl { dir: None });
    }

    #[test]
    fn default_when_log_section_absent() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "pii:\n  scanner: regex_basic\n");
        let cfg = load_log_config(&p).unwrap();
        assert_eq!(cfg, LogConfig::default());
    }

    /// Spec: "Explicit none sink opts out" — aligns with `pii.scanner: none`
    /// foot-gun avoidance.
    #[test]
    fn explicit_none_sink() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "log:\n  sink: none\n");
        let cfg = load_log_config(&p).unwrap();
        assert_eq!(cfg.sink, SinkConfig::Null {});
    }

    /// Spec: "Custom dir path is honored"
    #[test]
    fn explicit_jsonl_with_dir() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "log:\n  sink: jsonl\n  dir: /var/log/codebus\n");
        let cfg = load_log_config(&p).unwrap();
        assert_eq!(
            cfg.sink,
            SinkConfig::Jsonl {
                dir: Some(PathBuf::from("/var/log/codebus"))
            }
        );
    }

    /// Spec: "Bare YAML null in sink position returns parse error".
    /// Mirror of the `pii.scanner: null` foot-gun case.
    #[test]
    fn yaml_null_literal_returns_parse_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "log:\n  sink: null\n");
        let result = load_log_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Spec: "Unknown sink discriminator returns parse error"
    #[test]
    fn unknown_sink_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(tmp.path(), "log:\n  sink: otel\n");
        let result = load_log_config(&p);
        assert!(matches!(
            result,
            Err(super::super::ConfigLoadError::YamlParse(_))
        ));
    }

    /// Spec: "Unknown subkey silently ignored"
    #[test]
    fn unknown_subkey_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let p = write_yaml(
            tmp.path(),
            "log:\n  sink: jsonl\n  retention_days: 30\n",
        );
        let cfg = load_log_config(&p).unwrap();
        assert_eq!(cfg.sink, SinkConfig::Jsonl { dir: None });
    }
}
