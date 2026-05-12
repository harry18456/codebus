//! Integration tests for `codebus config` subcommand.
//!
//! These tests touch the real OS keyring (no mock is available in
//! keyring v3 without writing one ourselves). Each test invents a
//! unique service name so concurrent runs cannot collide AND the test
//! cleans up its entry on exit via `delete-key`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");

/// `CODEBUS_HOME` content seeded so the binary picks up our custom
/// `keyring_service`. The azure profile lives in cold storage here
/// (active stays system) so the existing default verbs still resolve;
/// only the `keyring_service` field is read by the config subcommand.
fn write_config_with_service(home: &std::path::Path, service: &str) {
    let cfg_dir = home.join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let cfg_path = cfg_dir.join("config.yaml");
    let body = format!(
        "claude_code:\n  active: system\n  system:\n    goal:  {{ model: opus-4-6, effort: high }}\n    query: {{ model: haiku-4-5,  effort: low }}\n    fix:   {{ model: sonnet-4-6, effort: medium }}\n  azure:\n    base_url: https://placeholder.example.com/anthropic\n    keyring_service: {service}\n    goal:  {{ model: dep-opus, effort: high }}\n    query: {{ model: dep-haiku, effort: low }}\n    fix:   {{ model: dep-sonnet, effort: medium }}\n"
    );
    std::fs::write(cfg_path, body).unwrap();
}

fn unique_service() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("codebus-test-{pid}-{n}-{ts}")
}

fn run_codebus(home: &std::path::Path, args: &[&str], stdin_body: Option<&str>) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args).env("CODEBUS_HOME", home);
    match stdin_body {
        Some(body) => {
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            let mut child = cmd.spawn().expect("spawn codebus");
            {
                let mut stdin = child.stdin.take().expect("stdin piped");
                stdin.write_all(body.as_bytes()).unwrap();
            }
            child.wait_with_output().expect("wait codebus")
        }
        None => cmd.output().expect("run codebus"),
    }
}

fn cleanup(home: &PathBuf, service: &str) {
    // Best-effort cleanup; we have already asserted the relevant outcome
    // by the time this runs. `service` is the unique keyring name we
    // generated — keeping it in scope keeps the lifetime expectation
    // explicit even though the binary reads it back from CODEBUS_HOME.
    let _ = run_codebus(home, &["config", "delete-key", "azure"], None);
    let _ = service;
}

/// Spec: set-key → get-key (default) → get-key --show → delete-key →
/// get-key (default) round-trip.
#[test]
fn set_get_show_delete_round_trip() {
    let home = TempDir::new().unwrap();
    let service = unique_service();
    write_config_with_service(home.path(), &service);

    // set-key
    let out = run_codebus(home.path(), &["config", "set-key", "azure"], Some("sk-round-trip\n"));
    assert!(
        out.status.success(),
        "set-key failed: stderr={}, stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("key stored"));

    // get-key (default: only set/unset)
    let out = run_codebus(home.path(), &["config", "get-key", "azure"], None);
    assert!(out.status.success(), "get-key failed: {:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim() == "set",
        "expected `set`, got `{}`",
        stdout.trim()
    );
    // Default get-key SHALL NOT leak the key value.
    assert!(
        !stdout.contains("sk-round-trip"),
        "key value leaked through default get-key: {stdout}"
    );

    // get-key --show
    let out = run_codebus(home.path(), &["config", "get-key", "azure", "--show"], None);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim() == "sk-round-trip",
        "--show should print key value, got `{}`",
        stdout.trim()
    );

    // delete-key
    let out = run_codebus(home.path(), &["config", "delete-key", "azure"], None);
    assert!(out.status.success(), "delete-key failed: {:?}", out);

    // get-key after delete
    let out = run_codebus(home.path(), &["config", "get-key", "azure"], None);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "unset");

    cleanup(&home.path().to_path_buf(), &service);
}

/// Spec: delete-key on absent entry SHALL exit 0 (idempotent).
#[test]
fn delete_key_is_idempotent_on_absent_entry() {
    let home = TempDir::new().unwrap();
    let service = unique_service();
    write_config_with_service(home.path(), &service);

    // No prior set-key — entry does not exist.
    let out = run_codebus(home.path(), &["config", "delete-key", "azure"], None);
    assert!(
        out.status.success(),
        "delete-key on absent entry must exit 0: {:?}",
        out
    );

    cleanup(&home.path().to_path_buf(), &service);
}

/// Spec: unknown profile value (e.g. `bedrock`) SHALL be rejected by
/// clap with a non-zero exit and a clap error message on stderr.
#[test]
fn unknown_profile_value_rejected_by_clap() {
    let home = TempDir::new().unwrap();
    let out = run_codebus(home.path(), &["config", "set-key", "bedrock"], None);
    assert!(!out.status.success(), "bedrock must be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid")
            || stderr.contains("possible values")
            || stderr.contains("'bedrock'"),
        "expected clap rejection mentioning bedrock or possible values; got: {stderr}"
    );
}

/// Spec: `codebus config --help` lists the three sub-actions.
#[test]
fn config_help_lists_three_sub_actions() {
    let out = Command::new(BIN)
        .args(["config", "--help"])
        .output()
        .expect("run codebus");
    assert!(out.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    for action in ["set-key", "get-key", "delete-key"] {
        assert!(
            combined.contains(action),
            "config --help missing `{action}`:\n{combined}"
        );
    }
}
