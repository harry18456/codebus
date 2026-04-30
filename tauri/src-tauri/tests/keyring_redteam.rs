//! Red-team coverage for `keyring.rs` provider_id validation. Backs
//! spec `keyring-integration` Requirement: Tauri keyring plugin
//! commands — provider_id regex `^[a-z][a-z0-9-]{2,40}$`.
//!
//! Every case asserts that an invalid provider_id is rejected with
//! `KEYRING_INVALID_PROVIDER_ID` BEFORE any OS keychain primitive is
//! invoked. Validation is a pure function; the host-side namespace
//! prefix `codebus.<provider_id>.api_key` is constructed only after
//! validation succeeds, so a rejected id MUST NOT touch the keychain.
//!
//! Happy path coverage lives separately (see Task 1.4 in tasks.md).

use codebus_lib::keyring::{
    keyring_delete_inner, keyring_get_inner, keyring_set_inner, validate_provider_id,
    KEYRING_ENTRY_MISSING, KEYRING_INVALID_PROVIDER_ID,
};

const SENTINEL_KEY: &str = "sk-test-do-not-leak";

/// Build a provider_id unique to (process, test) so concurrent
/// `cargo test` runs and parallel test threads cannot collide on the
/// OS keychain. The format keeps the regex `^[a-z][a-z0-9-]{2,40}$`
/// satisfied: a leading lowercase letter, then digits / hyphens.
fn unique_id(suffix: &str) -> String {
    let pid = std::process::id();
    format!("rtt-{pid}-{suffix}")
}

/// RAII cleanup so the OS keychain entry is removed even if the test
/// panics on assertion failure. Without this a flaky test would leak
/// sentinel credentials across runs.
struct CleanupGuard(String);
impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = keyring_delete_inner(&self.0);
    }
}

/// 14 red-team provider_id values that MUST be rejected.
fn invalid_provider_ids() -> Vec<(&'static str, &'static str)> {
    vec![
        // Length violations (regex requires 3..=41 chars total)
        ("empty string", ""),
        ("single char", "a"),
        ("two chars", "ab"),
        // Path traversal & separators
        ("dotdot escape", "..escape"),
        ("forward slash", "name/sub"),
        ("backslash", "name\\sub"),
        // Control / non-printable characters
        ("newline", "valid\nname"),
        ("null byte", "valid\0name"),
        ("unicode control SOH", "valid\u{0001}name"),
        // Shell metacharacters
        ("shell metachar dollar paren", "$(echo)"),
        // Windows reserved device name (case-folded match)
        ("windows reserved con", "con"),
        // Regex anchor violations
        ("starts with digit", "1abc"),
        ("starts with hyphen", "-abc"),
        ("uppercase letter", "Abc"),
    ]
}

#[test]
fn red_team_count_is_14() {
    assert_eq!(invalid_provider_ids().len(), 14);
}

#[test]
fn validate_provider_id_rejects_all_red_team_cases() {
    for (label, id) in invalid_provider_ids() {
        let result = validate_provider_id(id);
        assert!(
            result.is_err(),
            "[{label}] validate_provider_id({id:?}) must return Err, got Ok"
        );
        assert_eq!(
            result.unwrap_err(),
            KEYRING_INVALID_PROVIDER_ID,
            "[{label}] error code mismatch"
        );
    }
}

#[test]
fn keyring_set_rejects_all_red_team_cases() {
    for (label, id) in invalid_provider_ids() {
        let resp = keyring_set_inner(id, SENTINEL_KEY);
        assert!(!resp.ok, "[{label}] keyring_set_inner({id:?}) must return ok=false");
        assert_eq!(
            resp.code.as_deref(),
            Some(KEYRING_INVALID_PROVIDER_ID),
            "[{label}] keyring_set_inner code mismatch"
        );
        assert!(
            resp.api_key.is_none(),
            "[{label}] error response must not carry api_key"
        );
    }
}

#[test]
fn keyring_get_rejects_all_red_team_cases() {
    for (label, id) in invalid_provider_ids() {
        let resp = keyring_get_inner(id);
        assert!(!resp.ok, "[{label}] keyring_get_inner({id:?}) must return ok=false");
        assert_eq!(
            resp.code.as_deref(),
            Some(KEYRING_INVALID_PROVIDER_ID),
            "[{label}] keyring_get_inner code mismatch"
        );
    }
}

#[test]
fn keyring_delete_rejects_all_red_team_cases() {
    for (label, id) in invalid_provider_ids() {
        let resp = keyring_delete_inner(id);
        assert!(
            !resp.ok,
            "[{label}] keyring_delete_inner({id:?}) must return ok=false"
        );
        assert_eq!(
            resp.code.as_deref(),
            Some(KEYRING_INVALID_PROVIDER_ID),
            "[{label}] keyring_delete_inner code mismatch"
        );
    }
}

// ───── Happy-path scenarios (Task 1.4 / 1.5) ────────────────────────
//
// These tests exercise the OS keychain backend on the host running
// `cargo test`. They are intentionally not gated by `#[ignore]` so
// the local Windows / macOS / Linux developer iteration loop catches
// regressions; CI matrix coverage lives in tasks.md task 12.5.

#[test]
fn happy_set_then_get_round_trips_value() {
    let id = unique_id("rtg");
    let _guard = CleanupGuard(id.clone());

    let set_resp = keyring_set_inner(&id, "sk-rtg-sentinel");
    assert!(set_resp.ok, "set_inner failed: code={:?}", set_resp.code);

    let get_resp = keyring_get_inner(&id);
    assert!(get_resp.ok, "get_inner failed: code={:?}", get_resp.code);
    assert_eq!(get_resp.api_key.as_deref(), Some("sk-rtg-sentinel"));
    assert!(get_resp.code.is_none());
}

#[test]
fn happy_set_delete_get_returns_entry_missing() {
    let id = unique_id("sdg");
    let _guard = CleanupGuard(id.clone());

    assert!(keyring_set_inner(&id, "sk-sdg").ok);
    assert!(keyring_delete_inner(&id).ok);

    let resp = keyring_get_inner(&id);
    assert!(!resp.ok);
    assert_eq!(resp.code.as_deref(), Some(KEYRING_ENTRY_MISSING));
    assert!(resp.api_key.is_none());
}

#[test]
fn happy_delete_unset_id_succeeds_idempotent() {
    let id = unique_id("idem");
    // No prior set — guard is still in place to clean up the (rare)
    // case where another test process raced an entry in between.
    let _guard = CleanupGuard(id.clone());

    let resp = keyring_delete_inner(&id);
    assert!(resp.ok, "deleting unset id must succeed: code={:?}", resp.code);
    assert!(resp.code.is_none());
}

#[test]
fn happy_multiple_ids_do_not_interfere() {
    let id_a = unique_id("multi-a");
    let id_b = unique_id("multi-b");
    let _ga = CleanupGuard(id_a.clone());
    let _gb = CleanupGuard(id_b.clone());

    assert!(keyring_set_inner(&id_a, "value-A").ok);
    assert!(keyring_set_inner(&id_b, "value-B").ok);

    let got_a = keyring_get_inner(&id_a);
    let got_b = keyring_get_inner(&id_b);
    assert_eq!(got_a.api_key.as_deref(), Some("value-A"));
    assert_eq!(got_b.api_key.as_deref(), Some("value-B"));

    // Deleting A must not remove B.
    assert!(keyring_delete_inner(&id_a).ok);
    let got_a_after = keyring_get_inner(&id_a);
    assert_eq!(got_a_after.code.as_deref(), Some(KEYRING_ENTRY_MISSING));
    let got_b_after = keyring_get_inner(&id_b);
    assert_eq!(got_b_after.api_key.as_deref(), Some("value-B"));
}
