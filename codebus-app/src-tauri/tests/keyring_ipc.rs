//! Integration tests for the keyring IPC commands defined in
//! `codebus-app/src-tauri/src/ipc/keyring.rs`.
//!
//! These tests touch the real OS keyring (no mock backend exists in
//! `keyring` v3.x without adding one ourselves). Each test invents a
//! unique service name so concurrent runs cannot collide AND every test
//! cleans up its planted entry on exit.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// `cargo test` parallelises by default and these tests mutate the
/// process-wide `CODEBUS_HOME` env var. Without a lock, calls race and
/// the IPC's `resolve_keyring_service` reads a different test's
/// CODEBUS_HOME than the one we just planted.
fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

use codebus_app_tauri_lib::error::AppError;
use codebus_app_tauri_lib::ipc::REGISTERED_COMMANDS;
use codebus_app_tauri_lib::ipc::keyring::{
    KeyStatus, delete_endpoint_key, get_endpoint_key, set_endpoint_key,
};
use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};
use tempfile::TempDir;

fn unique_service() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("codebus-test-app-{pid}-{n}-{ts}")
}

/// Helper: write a yaml file under a temp CODEBUS_HOME that references
/// the given keyring service name. The IPC commands resolve the service
/// from this yaml when CODEBUS_HOME points here.
fn setup_home_with_service(service: &str) -> TempDir {
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let body = format!(
        "claude_code:\n  active: system\n  system:\n    goal:  {{ model: opus-4-6,   effort: high }}\n    query: {{ model: haiku-4-5,  effort: low }}\n    fix:   {{ model: sonnet-4-6, effort: medium }}\n  azure:\n    base_url: https://placeholder.example.com/anthropic\n    keyring_service: {service}\n    goal:  {{ model: dep-opus, effort: high }}\n    query: {{ model: dep-haiku, effort: low }}\n    fix:   {{ model: dep-sonnet, effort: medium }}\n",
    );
    std::fs::write(cfg_dir.join("config.yaml"), body).unwrap();
    home
}

/// Run an async test body with `CODEBUS_HOME` scoped to the given
/// TempDir for the duration. The keyring IPC commands consult
/// `default_config_path()` which honors `CODEBUS_HOME`.
fn with_home<F, R>(home: &TempDir, f: F) -> R
where
    F: std::future::Future<Output = R> + Send,
    R: Send + 'static,
{
    let _guard = serial_lock();
    let prev = std::env::var("CODEBUS_HOME").ok();
    unsafe {
        std::env::set_var("CODEBUS_HOME", home.path());
    }
    let result = tauri::async_runtime::block_on(f);
    unsafe {
        match prev {
            Some(v) => std::env::set_var("CODEBUS_HOME", v),
            None => std::env::remove_var("CODEBUS_HOME"),
        }
    }
    result
}

/// Spec: `IPC Command Registry` (fix-app-quiz modified) —
/// 23 commands total (9 foundation + 6 workspace + 2 chat + 6 quiz) AND
/// the three keyring command names are part of the registered set.
#[test]
fn registered_commands_includes_three_keyring_names() {
    let names: std::collections::HashSet<&str> = REGISTERED_COMMANDS.iter().copied().collect();
    assert_eq!(names.len(), 23);
    for required in [
        "set_endpoint_key",
        "get_endpoint_key",
        "delete_endpoint_key",
    ] {
        assert!(names.contains(required), "missing command: {required}");
    }
}

/// Spec: round-trip `set` → `get` (Set) → `delete` → `get` (Unset).
#[test]
fn round_trip_set_get_delete_against_real_keyring() {
    let service = unique_service();
    let home = setup_home_with_service(&service);

    with_home(&home, async {
        // Initially unset.
        let status = get_endpoint_key("azure".into()).await.unwrap();
        assert_eq!(status, KeyStatus::Unset);

        // Set.
        set_endpoint_key("azure".into(), "sk-app-ipc".into())
            .await
            .unwrap();
        let status = get_endpoint_key("azure".into()).await.unwrap();
        assert_eq!(status, KeyStatus::Set);

        // Cross-check via codebus-core helper that the entry is real.
        let probed = probe_keyring_only(&service).unwrap();
        assert_eq!(probed.as_deref(), Some("sk-app-ipc"));

        // Delete.
        delete_endpoint_key("azure".into()).await.unwrap();
        let status = get_endpoint_key("azure".into()).await.unwrap();
        assert_eq!(status, KeyStatus::Unset);
    });

    // Belt-and-braces cleanup.
    let _ = delete_azure_key(&service);
}

/// Spec: `delete_endpoint_key` is idempotent.
#[test]
fn delete_endpoint_key_is_idempotent_when_entry_absent() {
    let service = unique_service();
    let home = setup_home_with_service(&service);

    with_home(&home, async {
        // No prior set — delete still returns Ok.
        delete_endpoint_key("azure".into()).await.unwrap();
    });
}

/// Spec: `Unknown profile value rejected` — all three commands SHALL
/// reject `bedrock` / `vertex` / other with `AppError::Invalid`.
#[test]
fn unknown_profile_rejected_by_all_three_commands() {
    let home = setup_home_with_service(&unique_service());

    with_home(&home, async {
        let err = set_endpoint_key("bedrock".into(), "sk-x".into())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "profile"));

        let err = get_endpoint_key("vertex".into()).await.unwrap_err();
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "profile"));

        let err = delete_endpoint_key("openai".into()).await.unwrap_err();
        assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "profile"));
    });
}

/// Spec: `Config parse failure aborts keyring command`. Plant a real
/// keyring entry, point CODEBUS_HOME at a broken-yaml config, then
/// confirm `delete_endpoint_key` fails AND the entry survives.
#[test]
fn config_parse_failure_aborts_delete_without_touching_keyring() {
    let service = unique_service();
    store_azure_key(&service, "sentinel-survives").expect("plant key");

    // Setup a CODEBUS_HOME whose config.yaml is yaml-syntactically broken
    // (missing colon on `pii` key) AND references our `service` name
    // inside the azure block. The IPC's resolve helper SHALL emit
    // ConfigParse before reaching the keyring code path; if the bug
    // is reintroduced (silent fallback to `codebus-azure` default),
    // delete-key would target the wrong service and our planted entry
    // would survive ANYWAY. So we also check that the IPC raised
    // ConfigParse explicitly.
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let broken = format!(
        "claude_code:\n  active: system\n  system:\n    goal:  {{ model: opus-4-6,   effort: high }}\n    query: {{ model: haiku-4-5,  effort: low }}\n    fix:   {{ model: sonnet-4-6, effort: medium }}\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: {service}\n    goal:  {{ model: dep-opus, effort: high }}\n    query: {{ model: dep-haiku, effort: low }}\n    fix:   {{ model: dep-sonnet, effort: medium }}\npii\n  scanner: regex_basic\n",
    );
    std::fs::write(cfg_dir.join("config.yaml"), broken).unwrap();

    with_home(&home, async {
        let err = delete_endpoint_key("azure".into()).await.unwrap_err();
        assert!(
            matches!(err, AppError::ConfigParse { .. }),
            "expected ConfigParse, got {err:?}"
        );
    });

    let after = probe_keyring_only(&service).unwrap();
    assert_eq!(
        after.as_deref(),
        Some("sentinel-survives"),
        "keyring entry was deleted despite broken yaml — fallback bug regressed"
    );

    // Cleanup.
    let _ = delete_azure_key(&service);
}

/// Spec: `get_endpoint_key` SHALL NOT return the key value. We plant a
/// distinctive key, call get_endpoint_key, serialise its result, and
/// scan the output for the plant value. The serialised reply MUST
/// contain only `{"kind":"set"}`-shaped payload — never the key string.
#[test]
fn get_endpoint_key_response_does_not_contain_key_value() {
    let service = unique_service();
    let home = setup_home_with_service(&service);
    let plant = "sk-NEVER-LEAK-rA9k2hX";
    store_azure_key(&service, plant).expect("plant key");

    let response_json = with_home(&home, async {
        let status = get_endpoint_key("azure".into()).await.unwrap();
        serde_json::to_string(&status).unwrap()
    });

    assert!(
        !response_json.contains(plant),
        "get_endpoint_key response leaked key value: {response_json}"
    );
    assert_eq!(response_json, r#"{"kind":"set"}"#);

    let _ = delete_azure_key(&service);
}
