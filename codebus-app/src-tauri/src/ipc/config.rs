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
use codebus_core::config::endpoint::{
    parse_claude_code_yaml, parse_codex_yaml, read_active_provider,
};

use super::IpcResult;
use crate::config::{AppConfig, read_app_config, resolve_quiz_default_length};
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
        "quiz": { "default_length": codebus_core::config::quiz::DEFAULT_QUIZ_LENGTH },
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
    // Validate the shared quiz.default_length (incl. a legacy
    // app.quiz.default_length still on disk) so an out-of-range value
    // surfaces on load rather than silently defaulting.
    let _ = resolve_quiz_default_length(&payload)?;
    Ok(payload)
}

pub(crate) fn save_global_config_at(path: &Path, payload: &GlobalConfig) -> IpcResult<()> {
    // Validate first — surfaces `AppError::Invalid` / `ConfigParse` to the
    // caller without ever touching disk.
    let app_cfg = read_app_config(payload)?;
    // Resolve the shared quiz length (migrating a legacy
    // app.quiz.default_length forward) and reject an out-of-range value
    // before any disk write.
    let quiz_default_length = resolve_quiz_default_length(payload)?;
    // Also validate the `claude_code.*` block via codebus-core's
    // endpoint parser so an incomplete azure profile (active=azure with
    // empty base_url, deployment names, etc.) is rejected at write time
    // instead of producing a yaml the CLI will fail-loud on next load.
    validate_claude_code(payload)?;

    // Enrich the payload so the on-disk YAML always carries a fully
    // populated `app.*` section. Without this, a partial frontend patch
    // (e.g. user only changed `pass_threshold`) round-trips through disk
    // as a missing-field YAML — the next load then fails to deserialize
    // because of the absent sibling field.
    let mut enriched = payload.clone();
    let enriched_app = serde_json::to_value(&app_cfg).map_err(|e| AppError::ConfigParse {
        message: format!("app→json: {e}"),
    })?;
    if let Some(obj) = enriched.as_object_mut() {
        // Enriched `app` comes from an AppConfig that no longer carries
        // `default_length`, so a legacy `app.quiz.default_length` is
        // dropped here. Write the resolved length to the shared top-level
        // `quiz.*` key — this is the one-time migration landing point.
        obj.insert("app".to_string(), enriched_app);
        // Merge the resolved `quiz.default_length` into the existing `quiz`
        // object rather than replacing the whole namespace — replacing
        // would silently drop other `quiz.*` keys (notably
        // `quiz.content_verify`) that the user just set in the Settings
        // UI. Discovered via manual e2e of settings-config-frontend.
        let mut quiz_obj = obj
            .get("quiz")
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        quiz_obj.insert(
            "default_length".to_string(),
            serde_json::Value::from(quiz_default_length),
        );
        obj.insert("quiz".to_string(), serde_json::Value::Object(quiz_obj));
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

/// Run the codebus-core endpoint parser against the about-to-be-saved
/// payload's `agent` section, if any. Rejects an incomplete active endpoint
/// profile with `AppError::Invalid` so the frontend can surface inline error
/// messages instead of writing yaml the CLI will refuse to load.
///
/// Missing `agent` section → no validation (legitimate first-time setup
/// before the user touches endpoint settings).
fn validate_claude_code(payload: &GlobalConfig) -> IpcResult<()> {
    let agent_value = match payload.get("agent") {
        Some(v) => v,
        None => return Ok(()),
    };
    let agent_body = serde_yaml::to_string(agent_value).map_err(|e| AppError::Invalid {
        field: "agent".into(),
        message: format!("failed to serialise for validation: {e}"),
    })?;
    // `agent_value` is the body of the `agent` mapping; re-nest it under the
    // top-level `agent:` key so the loader sees the full document shape.
    let inner = agent_body
        .lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("  {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let wrapped = format!("agent:\n{inner}\n");
    // Always run the claude parser: it validates a claude block AND rejects an
    // unsupported `active_provider` name (e.g. `gemini`).
    if let Err(e) = parse_claude_code_yaml(&wrapped) {
        return Err(AppError::Invalid {
            field: "agent".into(),
            message: e.to_string(),
        });
    }
    // When codex is the active provider, validate its block via the codex
    // parser too — the claude parser treats a codex config as `Missing` and
    // would otherwise let an incomplete codex block through.
    let is_codex = read_active_provider(&wrapped)
        .map(|p| p == "codex")
        .unwrap_or(false);
    if is_codex {
        return match parse_codex_yaml(&wrapped) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(AppError::Invalid {
                field: "agent".into(),
                message: "active_provider is `codex` but `agent.providers.codex` is missing".into(),
            }),
            Err(e) => Err(AppError::Invalid {
                field: "agent".into(),
                message: e.to_string(),
            }),
        };
    }
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
        // default_length moved out of app.* into the shared quiz.* key.
        assert_eq!(payload["quiz"]["default_length"], json!(5));
    }

    #[test]
    fn round_trip_preserves_unknown_sections() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "pii": { "scanner": "regex_basic", "on_hit": "warn" },
            "claude_code": {
                "active": "system",
                "system": {
                    "goal":   { "model": "opus-4-6",   "effort": "high"   },
                    "query":  { "model": "haiku-4-5",  "effort": "low"    },
                    "fix":    { "model": "sonnet-4-6", "effort": "medium" },
                    "verify": { "model": "opus-4-6",   "effort": "high"   }
                }
            },
            "log": { "sink": "~/.codebus/logs/" },
            "app": { "quiz": { "pass_threshold": 70, "default_length": 4 } },
            // Section the app does NOT know about — must survive round-trip.
            "future_thing": { "knob": 42 },
        });

        save_global_config_at(&path, &payload).unwrap();
        let loaded = load_global_config_at(&path).unwrap();

        assert_eq!(loaded["future_thing"]["knob"], json!(42));
        assert_eq!(loaded["app"]["quiz"]["pass_threshold"], json!(70));
        assert_eq!(
            loaded["claude_code"]["system"]["goal"]["model"],
            json!("opus-4-6")
        );
    }

    /// verify-stage-independent-model-toggle: the `hooks` namespace
    /// SHALL round-trip cleanly including unknown subkeys so future
    /// hook toggles can be added without losing data on save/reload.
    /// Spec scenario "Hooks namespace survives save".
    #[test]
    fn hooks_namespace_round_trip_with_known_and_unknown_subkeys() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "claude_code": {
                "active": "system",
                "system": {
                    "goal":   { "model": "opus-4-6",   "effort": "high"   },
                    "query":  { "model": "haiku-4-5",  "effort": "low"    },
                    "fix":    { "model": "sonnet-4-6", "effort": "medium" },
                    "verify": { "model": "opus-4-6",   "effort": "high"   }
                }
            },
            "hooks": {
                "read_image_block": false,
                "future_hook_toggle": true
            }
        });

        save_global_config_at(&path, &payload).unwrap();
        let loaded = load_global_config_at(&path).unwrap();

        assert_eq!(loaded["hooks"]["read_image_block"], json!(false));
        assert_eq!(
            loaded["hooks"]["future_hook_toggle"],
            json!(true),
            "unknown hook subkey must survive save→load"
        );
    }

    /// Spec: `save_global_config` SHALL reject an incomplete azure
    /// profile (active=azure with empty base_url etc.) so an invalid
    /// yaml never lands on disk.
    #[test]
    fn save_rejects_incomplete_azure_active_profile() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "agent": {
                "active_provider": "claude",
                "providers": {
                    "claude": {
                        "active": "azure",
                        "system": {
                            "goal":  { "model": "opus-4-6",   "effort": "high" },
                            "query": { "model": "haiku-4-5",  "effort": "low" },
                            "fix":   { "model": "sonnet-4-6", "effort": "medium" }
                        },
                        "azure": {
                            "base_url": "",  // ← empty: invalid for active=azure
                            "keyring_service": "codebus-azure",
                            "goal":  { "model": "dep-x", "effort": "high" },
                            "query": { "model": "dep-y", "effort": "low" },
                            "fix":   { "model": "dep-z", "effort": "medium" }
                        }
                    }
                }
            },
            "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } }
        });
        let err =
            save_global_config_at(&path, &payload).expect_err("incomplete azure must be rejected");
        assert!(
            matches!(err, AppError::Invalid { ref field, .. } if field == "agent"),
            "expected Invalid(agent), got {err:?}"
        );
        // Disk file SHALL NOT be created.
        assert!(!path.exists(), "save failure must not write yaml");
    }

    /// Sanity: a fully-populated azure profile survives the validation
    /// gate and round-trips cleanly.
    #[test]
    fn save_accepts_complete_azure_active_profile() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "agent": {
                "active_provider": "claude",
                "providers": {
                    "claude": {
                        "active": "azure",
                        "system": {
                            "goal":   { "model": "opus-4-6",   "effort": "high"   },
                            "query":  { "model": "haiku-4-5",  "effort": "low"    },
                            "fix":    { "model": "sonnet-4-6", "effort": "medium" },
                            "verify": { "model": "opus-4-6",   "effort": "high"   }
                        },
                        "azure": {
                            "base_url": "https://x.example.com/anthropic",
                            "keyring_service": "codebus-azure",
                            "goal":   { "model": "dep-x", "effort": "high"   },
                            "query":  { "model": "dep-y", "effort": "low"    },
                            "fix":    { "model": "dep-z", "effort": "medium" },
                            "verify": { "model": "dep-x", "effort": "high"   }
                        }
                    }
                }
            },
            "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } }
        });
        save_global_config_at(&path, &payload).expect("complete profile accepted");
        let loaded = load_global_config_at(&path).expect("reloads cleanly");
        assert_eq!(
            loaded["agent"]["providers"]["claude"]["azure"]["base_url"],
            json!("https://x.example.com/anthropic")
        );
    }

    #[test]
    fn save_with_partial_app_payload_enriches_to_full_yaml() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        // Frontend may patch just `pass_threshold`. After save the on-disk
        // YAML MUST carry the enriched `app.quiz.pass_threshold` AND the
        // shared `quiz.default_length` so the next load is well-formed.
        let payload = json!({ "app": { "quiz": { "pass_threshold": 70 } } });

        save_global_config_at(&path, &payload).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();

        assert!(on_disk.contains("pass_threshold"));
        assert!(
            on_disk.contains("default_length"),
            "save must write shared quiz.default_length, got: {on_disk}"
        );

        let reloaded = load_global_config_at(&path).unwrap();
        assert_eq!(reloaded["app"]["quiz"]["pass_threshold"], json!(70));
        // default_length now lives in the shared quiz.* namespace.
        assert_eq!(reloaded["quiz"]["default_length"], json!(5));
    }

    /// Regression: settings-config-frontend manual e2e revealed that the
    /// previous unconditional `obj.insert("quiz", { default_length })`
    /// destroyed sibling `quiz.*` keys (notably `quiz.content_verify`),
    /// so the Settings UI could toggle Quiz content verify, click Save,
    /// reopen — and the key was silently absent on disk. The enrichment
    /// must MERGE `default_length` into the existing quiz object.
    #[test]
    fn save_preserves_quiz_sibling_keys() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 80 } },
            "quiz": { "default_length": 7, "content_verify": true },
        });
        save_global_config_at(&path, &payload).unwrap();
        let reloaded = load_global_config_at(&path).unwrap();
        assert_eq!(reloaded["quiz"]["default_length"], json!(7));
        assert_eq!(
            reloaded["quiz"]["content_verify"],
            json!(true),
            "quiz.content_verify must survive save→load round-trip"
        );
    }

    /// Migration: a stale pre-v3-app-quiz config with
    /// `app.quiz.default_length` is migrated forward on save — the value
    /// lands in the shared `quiz.default_length` and the legacy
    /// `app.quiz.default_length` is dropped.
    #[test]
    fn save_migrates_legacy_app_default_length_to_shared_key() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let legacy = json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 7 } } });

        save_global_config_at(&path, &legacy).unwrap();
        let reloaded = load_global_config_at(&path).unwrap();

        assert_eq!(reloaded["quiz"]["default_length"], json!(7));
        assert!(
            reloaded["app"]["quiz"].get("default_length").is_none(),
            "legacy app.quiz.default_length must be dropped, got: {}",
            reloaded["app"]["quiz"]
        );
        assert_eq!(reloaded["app"]["quiz"]["pass_threshold"], json!(80));
    }

    #[test]
    fn save_rejects_invalid_app_threshold_and_leaves_disk_untouched() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);

        // Plant a valid baseline.
        let baseline = json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } } });
        save_global_config_at(&path, &baseline).unwrap();
        let baseline_disk = fs::read_to_string(&path).unwrap();

        let bad = json!({ "app": { "quiz": { "pass_threshold": 200, "default_length": 5 } } });
        let err = save_global_config_at(&path, &bad).expect_err("must reject 200");
        assert!(
            matches!(err, AppError::Invalid { ref field, .. } if field == "app.quiz.pass_threshold")
        );

        // Disk content unchanged.
        let after = fs::read_to_string(&path).unwrap();
        assert_eq!(after, baseline_disk, "save failure must not corrupt disk");
    }

    #[test]
    fn save_uses_atomic_write_no_leftover_tmp() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } } });
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

    /// Spec (codex-settings-ui): save validates the codex provider block via
    /// `parse_codex_yaml` — a full codex config saves cleanly.
    fn codex_system_payload(verify_model: &str) -> GlobalConfig {
        json!({
            "agent": {
                "active_provider": "codex",
                "providers": {
                    "codex": {
                        "active": "system",
                        "system": {
                            "goal":   { "model": "gpt-5.5", "effort": "high" },
                            "query":  { "model": "gpt-5.5", "effort": "low" },
                            "fix":    { "model": "gpt-5.5", "effort": "medium" },
                            "verify": { "model": verify_model, "effort": "high" }
                        }
                    }
                }
            }
        })
    }

    #[test]
    fn save_accepts_valid_codex_config() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        save_global_config_at(&path, &codex_system_payload("gpt-5.5"))
            .expect("valid codex config must save");
        assert!(path.exists());

        // Observable on-disk result the GUI promises: the codex provider block
        // is persisted as YAML with active_provider switched to codex.
        let yaml = std::fs::read_to_string(&path).unwrap();
        assert!(yaml.contains("active_provider: codex"), "yaml:\n{yaml}");
        assert!(yaml.contains("gpt-5.5"), "yaml:\n{yaml}");

        // End-to-end: the SAME file, read back through the REAL core provider
        // loader the CLI uses, resolves to the codex provider with the model
        // the GUI wrote. This closes the GUI-write -> core-consume loop without
        // a webview (only the literal render is left for a manual smoke).
        let provider =
            codebus_core::agent::load_provider_config(&path).expect("core loads saved config");
        assert!(
            matches!(provider, codebus_core::agent::ProviderConfig::Codex(_)),
            "core must select the codex provider from the GUI-saved config",
        );
        let resolved = provider.resolve(codebus_core::config::Verb::Goal);
        assert_eq!(resolved.model.as_deref(), Some("gpt-5.5"));
    }

    #[test]
    fn save_rejects_codex_with_missing_verb_model() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        // verify model empty → active codex system profile incomplete.
        let err = save_global_config_at(&path, &codex_system_payload(""))
            .expect_err("incomplete codex profile must reject");
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "agent"));
        assert!(!path.exists(), "rejected config must not touch disk");
    }
}
