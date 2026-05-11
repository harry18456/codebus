//! `load_global_config` / `save_global_config` IPC commands.
//!
//! `~/.codebus/config.yaml` is read as a YAML document, converted to a
//! JSON Value for the IPC payload, and written back atomically on save.
//! The app validates only the `app.*` namespace; all other sections pass
//! through unchanged so the file can carry sections the app does not yet
//! know about without losing them on round-trip (spec rule "round-trip
//! 不掉欄位").

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use codebus_core::config::default_config_path;

use super::IpcResult;
use crate::config::{AppConfig, read_app_config};
use crate::error::AppError;

/// Frontend-facing payload. Mirrors the on-disk YAML shape as a JSON tree.
/// Tauri serializes this as JSON across the IPC boundary; the disk format
/// remains YAML.
pub type GlobalConfig = serde_json::Value;

/// Default in-memory payload when no file exists yet. The CLI's
/// `write_starter_config_if_missing` is the canonical default writer, but
/// the app may run before the CLI ever has — fall back to a payload that
/// at minimum carries the `app.*` defaults so the Settings UI renders.
fn default_payload() -> GlobalConfig {
    serde_json::json!({
        "app": serde_json::to_value(AppConfig::default()).unwrap(),
    })
}

/// Read a YAML file and convert to a JSON Value.
fn yaml_to_json(text: &str) -> Result<GlobalConfig, AppError> {
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|e| AppError::ConfigParse {
            message: e.to_string(),
        })?;
    serde_json::to_value(yaml).map_err(|e| AppError::ConfigParse {
        message: format!("yaml→json: {e}"),
    })
}

/// Write a JSON Value out as YAML.
fn json_to_yaml(payload: &GlobalConfig) -> Result<String, AppError> {
    serde_yaml::to_string(payload).map_err(|e| AppError::ConfigParse {
        message: format!("json→yaml: {e}"),
    })
}

pub(crate) fn load_global_config_at(path: &Path) -> IpcResult<GlobalConfig> {
    let payload = match fs::read_to_string(path) {
        Ok(text) => yaml_to_json(&text)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => default_payload(),
        Err(err) => return Err(AppError::from(err)),
    };
    let _ = read_app_config(&payload)?;
    Ok(payload)
}

pub(crate) fn save_global_config_at(
    path: &Path,
    payload: &GlobalConfig,
) -> IpcResult<()> {
    // Validate first — surfaces `AppError::Invalid` / `ConfigParse` to the
    // caller without ever touching disk.
    let app_cfg = read_app_config(payload)?;

    // Enrich the payload so the on-disk YAML always carries a fully
    // populated `app.*` section. Without this, a partial frontend patch
    // (e.g. user only changed `pass_threshold`) round-trips through disk
    // as a missing-field YAML — the next load then fails to deserialize
    // because of the absent sibling field.
    let mut enriched = payload.clone();
    let enriched_app = serde_json::to_value(&app_cfg).map_err(|e| {
        AppError::ConfigParse {
            message: format!("app→json: {e}"),
        }
    })?;
    if let Some(obj) = enriched.as_object_mut() {
        obj.insert("app".to_string(), enriched_app);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(AppError::from)?;
    }
    let yaml_text = json_to_yaml(&enriched)?;
    let tmp: PathBuf = path.with_extension("yaml.tmp");
    fs::write(&tmp, yaml_text).map_err(AppError::from)?;
    fs::rename(&tmp, path).map_err(AppError::from)?;
    Ok(())
}

fn global_config_path() -> IpcResult<PathBuf> {
    default_config_path().ok_or_else(|| AppError::Internal {
        message: "home directory unavailable".into(),
    })
}

#[tauri::command]
pub async fn load_global_config() -> IpcResult<GlobalConfig> {
    let path = global_config_path()?;
    load_global_config_at(&path)
}

#[tauri::command]
pub async fn save_global_config(config: GlobalConfig) -> IpcResult<()> {
    let path = global_config_path()?;
    save_global_config_at(&path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn config_path(tmp: &TempDir) -> PathBuf {
        tmp.path().join("config.yaml")
    }

    #[test]
    fn missing_file_returns_default_payload_with_app_defaults() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = load_global_config_at(&path).unwrap();
        assert_eq!(payload["app"]["quiz"]["pass_threshold"], json!(80));
        assert_eq!(payload["app"]["quiz"]["default_length"], json!(5));
    }

    #[test]
    fn round_trip_preserves_unknown_sections() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "pii": { "scanner": "regex_basic", "on_hit": "warn" },
            "claude_code": { "goal": { "model": "opus", "effort": "high" } },
            "log": { "sink": "~/.codebus/logs/" },
            "app": { "quiz": { "pass_threshold": 70, "default_length": 4 } },
            // Section the app does NOT know about — must survive round-trip.
            "future_thing": { "knob": 42 },
        });

        save_global_config_at(&path, &payload).unwrap();
        let loaded = load_global_config_at(&path).unwrap();

        assert_eq!(loaded["future_thing"]["knob"], json!(42));
        assert_eq!(loaded["app"]["quiz"]["pass_threshold"], json!(70));
        assert_eq!(loaded["claude_code"]["goal"]["model"], json!("opus"));
    }

    #[test]
    fn save_with_partial_app_payload_enriches_to_full_yaml() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        // Frontend may patch just `pass_threshold` — the on-disk YAML
        // MUST still contain both fields after save so the next load does
        // not stumble on a half-populated quiz section.
        let payload = json!({ "app": { "quiz": { "pass_threshold": 70 } } });

        save_global_config_at(&path, &payload).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();

        assert!(on_disk.contains("pass_threshold"));
        assert!(
            on_disk.contains("default_length"),
            "save must enrich app.* with both fields, got: {on_disk}"
        );

        let reloaded = load_global_config_at(&path).unwrap();
        assert_eq!(reloaded["app"]["quiz"]["pass_threshold"], json!(70));
        assert_eq!(reloaded["app"]["quiz"]["default_length"], json!(5));
    }

    #[test]
    fn save_rejects_invalid_app_threshold_and_leaves_disk_untouched() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);

        // Plant a valid baseline.
        let baseline =
            json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } } });
        save_global_config_at(&path, &baseline).unwrap();
        let baseline_disk = fs::read_to_string(&path).unwrap();

        let bad = json!({ "app": { "quiz": { "pass_threshold": 200, "default_length": 5 } } });
        let err = save_global_config_at(&path, &bad).expect_err("must reject 200");
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "app.quiz.pass_threshold"));

        // Disk content unchanged.
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, baseline_disk, "save failure must not corrupt disk");
    }

    #[test]
    fn save_uses_atomic_write_no_leftover_tmp() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload =
            json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } } });
        save_global_config_at(&path, &payload).unwrap();
        let tmp_path = path.with_extension("yaml.tmp");
        assert!(!tmp_path.exists(), "atomic rename must remove .tmp");
        assert!(path.exists());
    }

    #[test]
    fn load_invalid_threshold_surfaces_as_invalid() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        fs::write(
            &path,
            "app:\n  quiz:\n    pass_threshold: 200\n    default_length: 5\n",
        )
        .unwrap();
        let err = load_global_config_at(&path).expect_err("invalid threshold must fail");
        assert!(matches!(
            err,
            AppError::Invalid { ref field, .. } if field == "app.quiz.pass_threshold"
        ));
    }
}
