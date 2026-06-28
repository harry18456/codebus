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

use codebus_core::config::{CONFIG_HEADER, default_config_path};
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

/// Reserved synthetic payload key carrying the backend's built-in PII pattern
/// count to the frontend Settings UI (`regex_basic · N patterns`). It is NOT
/// user config: injected on load so N is never hard-coded in the UI, and
/// stripped on save so it never lands in `~/.codebus/config.yaml`. The double
/// underscore marks it as codebus-reserved, not a user YAML section.
const PII_PATTERN_COUNT_KEY: &str = "__pii_pattern_count";

/// Attach the live `builtin_pattern_count()` to a load payload so the frontend
/// can render the pattern count from the backend rather than a hard-coded
/// literal. A non-object payload is returned unchanged (defensive; the loaded
/// config is always a YAML mapping in practice).
fn inject_pattern_count(mut payload: GlobalConfig) -> GlobalConfig {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            PII_PATTERN_COUNT_KEY.to_string(),
            serde_json::Value::from(codebus_core::pii::builtin_pattern_count()),
        );
    }
    payload
}

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
        // Strip the synthetic pattern-count key the load path injects — it is
        // backend-derived metadata, never user config, and SHALL NOT persist.
        obj.remove(PII_PATTERN_COUNT_KEY);
        // config-save-robustness: drop empty / whitespace-only patterns_extra
        // before write. A blank PII rule (an unfilled Settings row) is an empty
        // regex that matches zero-width at every character — left on disk it
        // would make the next mirror scan pathologically slow. The scanner has
        // its own guards, but keeping the file clean stops the bad value at the
        // source.
        if let Some(pii) = obj.get_mut("pii").and_then(serde_json::Value::as_object_mut)
            && let Some(extras) = pii
                .get_mut("patterns_extra")
                .and_then(serde_json::Value::as_array_mut)
        {
            extras.retain(|v| v.as_str().is_none_or(|s| !s.trim().is_empty()));
        }
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
    // config-save-robustness: prepend the shared CONFIG_HEADER so an app-saved
    // config and a CLI-written starter share one header-plus-values shape. The
    // header is a YAML comment, so it does not affect the next load. (serde_yaml
    // strips comments on serialize, which is why the header is re-applied here
    // rather than carried through the payload.)
    let yaml_text = format!("{CONFIG_HEADER}\n{}", json_to_yaml(&enriched)?);
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
    let payload = load_global_config_at(&path)?;
    Ok(inject_pattern_count(payload))
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
    fn inject_pattern_count_carries_live_builtin_count() {
        let injected = inject_pattern_count(json!({ "pii": { "scanner": "regex_basic" } }));
        assert_eq!(
            injected[PII_PATTERN_COUNT_KEY],
            json!(codebus_core::pii::builtin_pattern_count())
        );
    }

    #[test]
    fn save_strips_synthetic_pattern_count_key() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        // Frontend echoes back the injected key on save — it MUST NOT persist.
        let payload = json!({
            "__pii_pattern_count": 13,
            "app": { "quiz": { "pass_threshold": 80 } },
        });
        save_global_config_at(&path, &payload).unwrap();
        let reloaded = load_global_config_at(&path).unwrap();
        assert!(
            reloaded.get(PII_PATTERN_COUNT_KEY).is_none(),
            "synthetic key leaked to disk: {reloaded:?}"
        );
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

    /// Spec ADDED requirement *Settings Language Override*: `app.locale_override`
    /// must persist through save→load with sibling `app.quiz.*` keys intact.
    #[test]
    fn save_persists_locale_override_with_sibling_app_quiz_keys() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 70 }, "locale_override": "en" },
        });

        save_global_config_at(&path, &payload).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            on_disk.contains("locale_override"),
            "locale_override must be written, got:\n{on_disk}"
        );

        let reloaded = load_global_config_at(&path).unwrap();
        assert_eq!(reloaded["app"]["locale_override"], json!("en"));
        assert_eq!(
            reloaded["app"]["quiz"]["pass_threshold"],
            json!(70),
            "app.quiz.pass_threshold must survive locale_override write"
        );
    }

    /// Spec scenario "Auto option follows the system locale": switching back
    /// to Auto persists as null (not absent garbage) so a downstream reader
    /// observes the explicit "auto" choice.
    #[test]
    fn save_writes_null_when_locale_override_unset() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "app": { "quiz": { "pass_threshold": 80 }, "locale_override": serde_json::Value::Null },
        });
        save_global_config_at(&path, &payload).unwrap();
        let reloaded = load_global_config_at(&path).unwrap();
        assert!(reloaded["app"]["locale_override"].is_null());
    }

    /// Spec scenario "Legacy config without locale_override round-trips
    /// safely": a config written by a pre-change version (no key) loads
    /// without error and the field comes through as null.
    #[test]
    fn load_legacy_config_without_locale_override_is_null() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        std::fs::write(
            &path,
            "app:\n  quiz:\n    pass_threshold: 80\n    default_length: 5\n",
        )
        .unwrap();
        let loaded = load_global_config_at(&path).expect("legacy must load");
        // Schema parses with locale_override defaulted to None → re-saved
        // payload reads back as null on the next round-trip. On this initial
        // load the raw value is absent (the file never wrote it).
        assert!(loaded["app"]["locale_override"].is_null() || loaded["app"].get("locale_override").is_none());
        assert_eq!(loaded["app"]["quiz"]["pass_threshold"], json!(80));
    }

    /// Invalid locale_override values surface as ConfigParse, never silently
    /// coerced. Backs spec failure mode "Zod parse 失敗 → 整個 settings load
    /// 失敗".
    #[test]
    fn load_rejects_invalid_locale_override_string() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        std::fs::write(
            &path,
            "app:\n  quiz:\n    pass_threshold: 80\n  locale_override: fr\n",
        )
        .unwrap();
        let err = load_global_config_at(&path).expect_err("`fr` must be rejected");
        assert!(
            matches!(err, AppError::ConfigParse { .. }),
            "expected ConfigParse, got {err:?}"
        );
    }

    /// config-save-robustness: an empty / whitespace-only `pii.patterns_extra`
    /// entry (e.g. a blank Settings row) MUST be dropped before write so it
    /// never lands on disk.
    #[test]
    fn save_drops_empty_patterns_extra() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({
            "pii": { "scanner": "regex_basic", "patterns_extra": ["", "   ", "real-pattern"] },
            "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } }
        });
        save_global_config_at(&path, &payload).unwrap();
        let reloaded = load_global_config_at(&path).unwrap();
        assert_eq!(
            reloaded["pii"]["patterns_extra"],
            json!(["real-pattern"]),
            "empty / whitespace patterns must be dropped, got {:?}",
            reloaded["pii"]["patterns_extra"]
        );
    }

    /// config-save-robustness: the saved YAML begins with the shared
    /// `CONFIG_HEADER` (same constant the CLI starter uses) and still loads
    /// back cleanly because the header is a comment.
    #[test]
    fn save_prepends_shared_config_header() {
        let tmp = TempDir::new().unwrap();
        let path = config_path(&tmp);
        let payload = json!({ "app": { "quiz": { "pass_threshold": 80, "default_length": 5 } } });
        save_global_config_at(&path, &payload).unwrap();
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            on_disk.starts_with(codebus_core::config::CONFIG_HEADER),
            "saved YAML must begin with CONFIG_HEADER, got: {:?}",
            &on_disk[..on_disk.len().min(160)]
        );
        load_global_config_at(&path).expect("reloads cleanly with header prepended");
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
