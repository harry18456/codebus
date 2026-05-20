//! Integration tests for `config::load_claude_code_config` covering the
//! legacy-schema migration warning. Lives in `tests/` so the assertions
//! exercise the public API surface (and so the test names — which form
//! the verification target named in tasks.md — are visible in
//! `cargo test --test endpoint_config_load`).

use std::fs;

use codebus_core::config::{
    ActiveProfile, ClaudeCodeConfig, LEGACY_MIGRATION_WARNING, load_claude_code_config_into,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Spec: Legacy Config Schema Warning Without Rewrite.
///
/// When `~/.codebus/config.yaml` contains the pre-profile schema (top-level
/// `goal` / `query` / `fix` keys under `claude_code`), `load_claude_code_config`
/// SHALL:
/// - emit a migration warning containing the literal "migration" keyword
///   AND a concrete new-schema example,
/// - leave the on-disk file byte-for-byte unchanged,
/// - return a config whose active profile is `System`.
#[test]
fn legacy_schema_warns_without_rewrite() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "claude_code:\n  goal:\n    model: opus\n    effort: high\n  query:\n    model: haiku\n    effort: low\n  fix:\n    model: sonnet\n    effort: medium\n";
    fs::write(&path, body).unwrap();

    let hash_before = sha256(&fs::read(&path).unwrap());
    let mut sink: Vec<u8> = Vec::new();
    let cfg = load_claude_code_config_into(&path, &mut sink).expect("legacy load succeeds");
    let hash_after = sha256(&fs::read(&path).unwrap());

    // 1) File byte-for-byte unchanged.
    assert_eq!(
        hash_before, hash_after,
        "legacy schema detection must not rewrite ~/.codebus/config.yaml"
    );

    // 2) Returned config has active=System and matches the built-in default.
    assert_eq!(cfg.active, ActiveProfile::System);
    assert_eq!(cfg, ClaudeCodeConfig::default());

    // 3) Warning text contains the migration keyword AND the concrete new
    //    schema example (the literal substring `active: system`).
    let warn = String::from_utf8(sink).expect("warning text is UTF-8");
    let lower = warn.to_lowercase();
    assert!(
        lower.contains("migrate") || lower.contains("migration") || lower.contains("legacy"),
        "warning missing migration keyword: {warn}"
    );
    assert!(
        warn.contains("active: system"),
        "warning missing new-schema example: {warn}"
    );
    // The shared constant lives in lib code — cross-check the integration
    // test sees the same text the public binary surfaces.
    assert!(warn.contains(LEGACY_MIGRATION_WARNING.lines().next().unwrap()));
}

/// Sanity: new-schema files do NOT trigger a migration warning. Pairs with
/// `legacy_schema_warns_without_rewrite` to prove the warning is gated on
/// the actual legacy shape (no false positives on healthy files).
#[test]
fn new_schema_load_emits_no_warning() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("config.yaml");
    let body = "claude_code:\n  active: system\n  system:\n    goal:   { model: opus-4-6,   effort: high   }\n    query:  { model: haiku-4-5,  effort: low    }\n    fix:    { model: sonnet-4-6, effort: medium }\n    verify: { model: opus-4-6,   effort: high   }\n";
    fs::write(&path, body).unwrap();

    let mut sink: Vec<u8> = Vec::new();
    let cfg = load_claude_code_config_into(&path, &mut sink).unwrap();
    assert_eq!(cfg.active, ActiveProfile::System);
    assert!(
        sink.is_empty(),
        "new-schema load must not emit warnings; got: {}",
        String::from_utf8_lossy(&sink)
    );
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
