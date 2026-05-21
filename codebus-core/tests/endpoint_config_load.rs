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

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
