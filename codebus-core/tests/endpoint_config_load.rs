//! Integration tests for `config::load_claude_code_config` over the unified
//! `agent.providers.*` schema. Lives in `tests/` so the assertions exercise
//! the public API surface (and the test names — the verification target named
//! in tasks.md — are visible in `cargo test --test endpoint_config_load`).

use std::fs;

use codebus_core::config::{
    ActiveProfile, ClaudeCodeConfig, Verb, load_claude_code_config,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Spec: REMOVED `Legacy Config Schema Warning Without Rewrite`.
///
/// A legacy top-level `claude_code` block is treated as an absent `agent`
/// block: `load_claude_code_config` SHALL return the built-in default, SHALL
/// NOT print any migration warning, and SHALL leave the on-disk file
/// byte-for-byte unchanged.
#[test]
fn legacy_claude_code_falls_back_to_default_without_rewrite() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "claude_code:\n  goal:\n    model: opus\n    effort: high\n  query:\n    model: haiku\n    effort: low\n";
    fs::write(&path, body).unwrap();

    let hash_before = sha256(&fs::read(&path).unwrap());
    let cfg = load_claude_code_config(&path).expect("legacy load falls back to default");
    let hash_after = sha256(&fs::read(&path).unwrap());

    // File byte-for-byte unchanged (loader never rewrites).
    assert_eq!(
        hash_before, hash_after,
        "legacy schema must not rewrite ~/.codebus/config.yaml"
    );
    // Legacy block is ignored → built-in default.
    assert_eq!(cfg, ClaudeCodeConfig::default());
}

/// Spec: `Endpoint Profile Schema` — a complete `agent.providers.claude`
/// system profile loads and resolves to the translated `--model` value.
#[test]
fn agent_schema_system_profile_loads_and_resolves() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: high   }\n        query:  { model: haiku-4-5,  effort: low    }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high   }\n";
    fs::write(&path, body).unwrap();

    let cfg = load_claude_code_config(&path).expect("new-schema load succeeds");
    assert_eq!(cfg.active, ActiveProfile::System);
    assert_eq!(
        cfg.resolve(Verb::Goal).model.as_deref(),
        Some("claude-opus-4-6")
    );
    assert_eq!(
        cfg.resolve(Verb::Query).model.as_deref(),
        Some("claude-haiku-4-5")
    );
}

/// Spec: `Endpoint Profile Schema` — active profile missing a required verb
/// sub-block is rejected (does not silently fall back to defaults).
#[test]
fn agent_schema_active_system_missing_verb_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    // system active but `verify` sub-block absent.
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:  { model: opus-4-6,   effort: high }\n        query: { model: haiku-4-5,  effort: low }\n        fix:   { model: sonnet-4-6, effort: medium }\n";
    fs::write(&path, body).unwrap();

    let err = load_claude_code_config(&path).expect_err("missing verify must reject");
    assert!(
        format!("{err}").contains("verify"),
        "error must name the missing verify field"
    );
}

/// Spec: `Endpoint Profile Schema` Effort Closed-Set Validation — an active
/// profile verb with an out-of-set effort (`ultra`) is rejected at load.
#[test]
fn agent_schema_invalid_effort_in_active_system_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: ultra  }\n        query:  { model: haiku-4-5,  effort: low    }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high   }\n";
    fs::write(&path, body).unwrap();

    let err = load_claude_code_config(&path).expect_err("out-of-set effort must reject");
    let msg = format!("{err}");
    assert!(
        msg.contains("system.goal.effort"),
        "error must name the offending field: {msg}"
    );
}

/// Spec: `Endpoint Profile Schema` — `auto` is NOT a valid effort (the Claude
/// CLI `--effort` does not accept it); it is rejected at load.
#[test]
fn agent_schema_auto_effort_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: high }\n        query:  { model: haiku-4-5,  effort: auto }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high }\n";
    fs::write(&path, body).unwrap();

    let err = load_claude_code_config(&path).expect_err("auto effort must reject");
    let msg = format!("{err}");
    assert!(
        msg.contains("system.query.effort"),
        "error must name the offending field: {msg}"
    );
}

/// Spec: `Endpoint Profile Schema` — the five valid efforts (incl. `xhigh` /
/// `max`, absent from the built-in defaults) all load successfully.
#[test]
fn agent_schema_five_valid_efforts_load() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: xhigh  }\n        query:  { model: haiku-4-5,  effort: max    }\n        fix:    { model: sonnet-4-6, effort: high   }\n        verify: { model: opus-4-6,   effort: low    }\n";
    fs::write(&path, body).unwrap();

    let cfg = load_claude_code_config(&path).expect("xhigh / max are valid efforts");
    assert_eq!(cfg.resolve(Verb::Goal).effort.as_deref(), Some("xhigh"));
    assert_eq!(cfg.resolve(Verb::Query).effort.as_deref(), Some("max"));
}

/// Spec: `Endpoint Profile Schema` — out-of-set effort in the ACTIVE azure
/// profile is rejected.
#[test]
fn agent_schema_invalid_effort_in_active_azure_rejected() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: azure\n      azure:\n        base_url: https://x.example/anthropic\n        keyring_service: codebus-azure\n        goal:   { model: dep-a, effort: high   }\n        query:  { model: dep-a, effort: low    }\n        fix:    { model: dep-a, effort: ultra  }\n        verify: { model: dep-a, effort: high   }\n";
    fs::write(&path, body).unwrap();

    let err = load_claude_code_config(&path).expect_err("azure out-of-set effort must reject");
    let msg = format!("{err}");
    assert!(
        msg.contains("azure.fix.effort"),
        "error must name the offending field: {msg}"
    );
}

/// Spec: `Endpoint Profile Schema` — validation applies to the ACTIVE profile
/// only; an out-of-set effort in a cold-storage (non-active) profile SHALL NOT
/// block the load.
#[test]
fn agent_schema_cold_storage_invalid_effort_does_not_block() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    // active = system (valid); azure is cold storage with an out-of-set effort.
    let body = "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: system\n      system:\n        goal:   { model: opus-4-6,   effort: high   }\n        query:  { model: haiku-4-5,  effort: low    }\n        fix:    { model: sonnet-4-6, effort: medium }\n        verify: { model: opus-4-6,   effort: high   }\n      azure:\n        base_url: https://x.example/anthropic\n        keyring_service: codebus-azure\n        goal:   { model: dep-a, effort: ultra }\n        query:  { model: dep-a, effort: low   }\n        fix:    { model: dep-a, effort: high  }\n        verify: { model: dep-a, effort: high  }\n";
    fs::write(&path, body).unwrap();

    let cfg = load_claude_code_config(&path)
        .expect("cold-storage out-of-set effort must NOT block the active system load");
    assert_eq!(cfg.active, ActiveProfile::System);
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
