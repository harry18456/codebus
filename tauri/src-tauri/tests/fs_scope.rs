//! capabilities/fs-scope.json — backs SHALL clauses in
//! openspec/changes/m1-power-on/specs/tauri-shell/spec.md
//!   Requirement: Filesystem scope restricts access
//!
//! Tauri 2 replaces tauri.conf.json's `fs.scope` with a capability JSON
//! that declares `fs:scope` allow / deny lists.  These tests validate the
//! capability JSON's *structure* at compile/test time so a malformed
//! capability cannot silently ship — runtime enforcement is exercised in
//! the integration tests invoked via `cargo tauri dev`.

use std::path::PathBuf;

use serde_json::Value;

fn load_fs_scope() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("capabilities")
        .join("fs-scope.json");
    let body = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn fs_scope_capability_has_identifier_and_schema() {
    let cap = load_fs_scope();
    assert_eq!(
        cap.get("identifier").and_then(|v| v.as_str()),
        Some("fs-scope"),
        "capability identifier must be exactly 'fs-scope'",
    );
    assert!(
        cap.get("$schema").is_some(),
        "capability must reference a JSON schema so IDE validation catches drift",
    );
}

#[test]
fn fs_scope_targets_main_window() {
    let cap = load_fs_scope();
    let windows = cap
        .get("windows")
        .and_then(|v| v.as_array())
        .expect("capability must declare target windows");
    assert!(
        windows.iter().any(|w| w.as_str() == Some("main")),
        "fs-scope must apply to the main window",
    );
}

#[test]
fn fs_scope_has_allow_entry_for_workspace_root() {
    let cap = load_fs_scope();
    let permissions = cap
        .get("permissions")
        .and_then(|v| v.as_array())
        .expect("permissions array required");
    let scope_perm = permissions
        .iter()
        .find(|p| p.get("identifier").and_then(|v| v.as_str()) == Some("fs:scope"))
        .expect("permissions must include an fs:scope entry");
    let allow = scope_perm
        .get("allow")
        .and_then(|v| v.as_array())
        .expect("fs:scope must declare an allow list");
    assert!(
        !allow.is_empty(),
        "allow list must not be empty — empty = no access, not all access",
    );
    let has_workspace_glob = allow.iter().any(|entry| {
        entry
            .get("path")
            .and_then(|p| p.as_str())
            .is_some_and(|p| p.contains("workspaces") || p.contains("$APPDATA"))
    });
    assert!(
        has_workspace_glob,
        "allow list must include a workspace-rooted glob (saw: {allow:?})",
    );
}

#[test]
fn fs_scope_denies_git_and_dotfiles() {
    let cap = load_fs_scope();
    let permissions = cap
        .get("permissions")
        .and_then(|v| v.as_array())
        .expect("permissions array required");
    let scope_perm = permissions
        .iter()
        .find(|p| p.get("identifier").and_then(|v| v.as_str()) == Some("fs:scope"))
        .expect("fs:scope entry required");
    let deny = scope_perm
        .get("deny")
        .and_then(|v| v.as_array())
        .expect("fs:scope must declare a deny list for .git / dotfiles");
    let deny_paths: Vec<&str> = deny
        .iter()
        .filter_map(|e| e.get("path").and_then(|p| p.as_str()))
        .collect();
    assert!(
        deny_paths.iter().any(|p| p.contains(".git")),
        "deny list must block .git (saw: {deny_paths:?})",
    );
}
