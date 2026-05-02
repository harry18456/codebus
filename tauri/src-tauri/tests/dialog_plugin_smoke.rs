//! Backs SHALL clauses in
//! openspec/changes/entry-workspace-onramp/specs/workspace-onramp/spec.md
//!   Requirement: Tauri host wires the dialog plugin
//!
//! Static smoke covering the three host-side wiring points the
//! Requirement Scenario asserts: Cargo.toml dependency, lib.rs builder
//! chain, and capabilities/default.json permission grant. Runtime
//! "open the picker" behavior is exercised manually in task 6.4
//! (cargo tauri dev) — not feasible in `cargo test` without a real
//! display server.

use std::path::PathBuf;

use serde_json::Value;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_text(rel_path: &str) -> String {
    let path = manifest_dir().join(rel_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn cargo_toml_lists_tauri_plugin_dialog() {
    let body = read_text("Cargo.toml");
    assert!(
        body.contains("tauri-plugin-dialog"),
        "Cargo.toml must list tauri-plugin-dialog as a dependency",
    );
}

#[test]
fn lib_rs_registers_dialog_plugin() {
    let body = read_text("src/lib.rs");
    assert!(
        body.contains("tauri_plugin_dialog::init()"),
        "lib.rs builder chain must include tauri_plugin_dialog::init()",
    );
}

#[test]
fn default_capability_grants_dialog_default() {
    let body = read_text("capabilities/default.json");
    let cap: Value = serde_json::from_str(&body)
        .expect("capabilities/default.json must be valid JSON");
    let perms = cap
        .get("permissions")
        .and_then(|v| v.as_array())
        .expect("default capability must declare a permissions array");
    let has_dialog_default = perms
        .iter()
        .any(|p| p.as_str() == Some("dialog:default"));
    assert!(
        has_dialog_default,
        "default capability must include 'dialog:default' (saw: {perms:?})",
    );
}

// Runtime smoke (`tauri::Builder::default().plugin(...).build()`) is
// intentionally NOT in `cargo test`: the test executable transitively
// links wry → WebView2 runtime DLLs, and those load failures
// (STATUS_ENTRYPOINT_NOT_FOUND) on a dev box without the WebView2
// runtime hide the actual config drift this file is meant to catch.
// Real runtime verification happens via `cargo tauri dev` (task 6.4)
// where the WebView2 runtime is guaranteed.
