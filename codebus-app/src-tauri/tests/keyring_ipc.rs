//! Integration tests for the keyring IPC commands defined in
//! `codebus-app/src-tauri/src/ipc/keyring.rs`.
//!
//! These tests touch the real OS keyring (no mock backend exists in
//! `keyring` v3.x without adding one ourselves). Each test invents a unique
//! service name so concurrent runs cannot collide AND every test cleans up
//! its planted entry on exit.
//!
//! The commands take the keyring `service` name directly (the Settings editor
//! passes the active provider's `azure.keyring_service`), so these tests no
//! longer set up a `CODEBUS_HOME` config — there is no config resolution step.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use codebus_app_tauri_lib::error::AppError;
use codebus_app_tauri_lib::ipc::REGISTERED_COMMANDS;
use codebus_app_tauri_lib::ipc::keyring::{
    KeyStatus, delete_endpoint_key, get_endpoint_key, set_endpoint_key,
};
use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};

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

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// These tests share the process-wide OS keyring. Even with unique service
/// names, concurrent set/get/delete against the Windows Credential Manager
/// can race, so keyring-touching tests acquire this lock to run serially.
fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// Spec: `IPC Command Registry` — 32 commands total AND the three keyring
/// command names are part of the registered set.
#[test]
fn registered_commands_includes_three_keyring_names() {
    let names: std::collections::HashSet<&str> = REGISTERED_COMMANDS.iter().copied().collect();
    assert_eq!(names.len(), 32);
    for required in ["set_endpoint_key", "get_endpoint_key", "delete_endpoint_key"] {
        assert!(names.contains(required), "missing command: {required}");
    }
}

/// Spec: round-trip `set` → `get` (Set) → `delete` → `get` (Unset) under the
/// service name passed by the caller.
#[test]
fn round_trip_set_get_delete_against_real_keyring() {
    let _g = serial_lock();
    let service = unique_service();

    block_on(async {
        assert_eq!(get_endpoint_key(service.clone()).await.unwrap(), KeyStatus::Unset);

        set_endpoint_key(service.clone(), "sk-app-ipc".into()).await.unwrap();
        assert_eq!(get_endpoint_key(service.clone()).await.unwrap(), KeyStatus::Set);

        // Cross-check via codebus-core helper that the entry is real and under
        // exactly the service the caller named.
        assert_eq!(probe_keyring_only(&service).unwrap().as_deref(), Some("sk-app-ipc"));

        delete_endpoint_key(service.clone()).await.unwrap();
        assert_eq!(get_endpoint_key(service.clone()).await.unwrap(), KeyStatus::Unset);
    });

    let _ = delete_azure_key(&service);
}

/// claude and codex pass DISTINCT service names, so a key set under one does
/// not appear under the other — the whole point of the per-provider default.
#[test]
fn keys_under_distinct_services_do_not_collide() {
    let _g = serial_lock();
    let claude = format!("{}-claude", unique_service());
    let codex = format!("{}-codex", unique_service());

    block_on(async {
        set_endpoint_key(claude.clone(), "sk-claude".into()).await.unwrap();
        // codex service still unset despite claude being set.
        assert_eq!(get_endpoint_key(codex.clone()).await.unwrap(), KeyStatus::Unset);

        set_endpoint_key(codex.clone(), "sk-codex".into()).await.unwrap();
        assert_eq!(probe_keyring_only(&claude).unwrap().as_deref(), Some("sk-claude"));
        assert_eq!(probe_keyring_only(&codex).unwrap().as_deref(), Some("sk-codex"));
    });

    let _ = delete_azure_key(&claude);
    let _ = delete_azure_key(&codex);
}

/// Spec: `delete_endpoint_key` is idempotent.
#[test]
fn delete_endpoint_key_is_idempotent_when_entry_absent() {
    let _g = serial_lock();
    let service = unique_service();
    block_on(async {
        delete_endpoint_key(service).await.unwrap();
    });
}

/// An empty / whitespace service name SHALL reject with
/// `AppError::Invalid { field: "service" }` across all three commands rather
/// than touching a blank keyring entry.
#[test]
fn empty_service_rejected_by_all_three_commands() {
    block_on(async {
        for blank in ["", "   "] {
            let err = set_endpoint_key(blank.into(), "sk-x".into()).await.unwrap_err();
            assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "service"));
            let err = get_endpoint_key(blank.into()).await.unwrap_err();
            assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "service"));
            let err = delete_endpoint_key(blank.into()).await.unwrap_err();
            assert!(matches!(err, AppError::Invalid { ref field, .. } if field == "service"));
        }
    });
}

/// Spec: `get_endpoint_key` SHALL NOT return the key value — the serialised
/// reply is only `{"kind":"set"}`, never the key string.
#[test]
fn get_endpoint_key_response_does_not_contain_key_value() {
    let _g = serial_lock();
    let service = unique_service();
    let plant = "sk-NEVER-LEAK-rA9k2hX";
    store_azure_key(&service, plant).expect("plant key");

    let response_json = block_on(async {
        let status = get_endpoint_key(service.clone()).await.unwrap();
        serde_json::to_string(&status).unwrap()
    });

    assert!(
        !response_json.contains(plant),
        "get_endpoint_key response leaked key value: {response_json}"
    );
    assert_eq!(response_json, r#"{"kind":"set"}"#);

    let _ = delete_azure_key(&service);
}
