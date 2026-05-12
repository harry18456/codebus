//! Integration tests for spec `cli / Config Parse Failure Aborts Invocation`.
//!
//! When `~/.codebus/config.yaml` exists but fails to parse (yaml syntax
//! error OR schema validation failure such as an invalid `SystemModel`
//! variant), every codebus subcommand SHALL exit non-zero with stderr
//! identifying the section AND the parse-error detail, AND SHALL NOT
//! perform any side effect that depends on the broken section.
//!
//! Side effects we assert do NOT happen:
//! - `claude` child spawn (we point `CODEBUS_CLAUDE_BIN` at the
//!   `mock-claude` binary whose first action is to write
//!   `CODEBUS_MOCK_LOG`; the log's absence is our spawn-counter).
//! - keyring delete (we plant a real keyring entry under a unique service
//!   name, run `codebus config delete-key azure`, and re-probe the entry).

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use codebus_core::config::keyring::{delete_azure_key, probe_keyring_only, store_azure_key};
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Yaml syntax error (missing colon) — `pii` key has no `:`.
const BROKEN_YAML_SYNTAX: &str = "pii\n  scanner: regex_basic\n";

/// Schema validation failure — `claude_code.system.goal.model` is not a
/// recognised `SystemModel` variant.
const BROKEN_SCHEMA: &str = "claude_code:\n  active: system\n  system:\n    goal:  { model: gpt-4,    effort: high }\n    query: { model: haiku-4-5,  effort: low }\n    fix:   { model: sonnet-4-6, effort: medium }\n";

// === Section A: yaml syntax error aborts every verb ===

#[test]
fn yaml_syntax_error_aborts_goal_before_spawn() {
    let _g = serial_lock();
    let (home, mock_log) = setup_with_config(BROKEN_YAML_SYNTAX);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["goal", "ingest something"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .current_dir(repo.path())
        .output()
        .expect("run codebus goal");

    assert!(
        !out.status.success(),
        "goal must exit non-zero on yaml syntax error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.to_lowercase().contains("config"),
        "stderr should mention config: {stderr}"
    );
    assert!(
        !mock_log.exists(),
        "claude spawn happened — mock-claude wrote {mock_log:?}"
    );
}

#[test]
fn yaml_syntax_error_aborts_query_before_spawn() {
    let _g = serial_lock();
    let (home, mock_log) = setup_with_config(BROKEN_YAML_SYNTAX);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["query", "what is X"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .current_dir(repo.path())
        .output()
        .expect("run codebus query");

    assert!(!out.status.success(), "query must exit non-zero");
    assert!(!mock_log.exists(), "claude spawn happened on query path");
}

#[test]
fn yaml_syntax_error_aborts_fix_before_spawn() {
    let _g = serial_lock();
    let (home, mock_log) = setup_with_config(BROKEN_YAML_SYNTAX);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["fix"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .current_dir(repo.path())
        .output()
        .expect("run codebus fix");

    assert!(!out.status.success(), "fix must exit non-zero");
    assert!(!mock_log.exists(), "claude spawn happened on fix path");
}

// === Section B: schema validation failure aborts every verb ===

#[test]
fn invalid_system_model_aborts_goal_before_spawn() {
    let _g = serial_lock();
    let (home, mock_log) = setup_with_config(BROKEN_SCHEMA);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["goal", "ingest"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .current_dir(repo.path())
        .output()
        .expect("run codebus goal");

    assert!(
        !out.status.success(),
        "goal must exit non-zero on schema error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("gpt-4") || stderr.to_lowercase().contains("variant"),
        "stderr should mention the invalid variant: {stderr}"
    );
    assert!(!mock_log.exists());
}

#[test]
fn invalid_system_model_aborts_query_before_spawn() {
    let _g = serial_lock();
    let (home, mock_log) = setup_with_config(BROKEN_SCHEMA);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["query", "x"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .current_dir(repo.path())
        .output()
        .expect("run codebus query");

    assert!(!out.status.success());
    assert!(!mock_log.exists());
}

// === Section C: `config delete-key` MUST NOT delete keyring on broken yaml ===

#[test]
fn yaml_syntax_error_aborts_config_delete_key_without_touching_keyring() {
    let _g = serial_lock();
    // Plant a real keyring entry with a unique service name; we'll
    // verify it survives the delete-key invocation.
    let service = unique_service();
    store_azure_key(&service, "sentinel-survives").expect("plant key");

    // Now write a config that REFERENCES this service name AND has a
    // yaml syntax error. The bug we're testing is that the broken yaml
    // → silent fallback to `codebus-azure` default → delete-key targets
    // the wrong service. Here we control the keyring_service via the
    // config; if config loading fails-loud, no delete happens at all.
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let broken = format!(
        "claude_code:\n  active: system\n  system:\n    goal:  {{ model: opus-4-6,   effort: high }}\n    query: {{ model: haiku-4-5,  effort: low }}\n    fix:   {{ model: sonnet-4-6, effort: medium }}\n  azure:\n    base_url: https://x.example.com/anthropic\n    keyring_service: {service}\n    goal:  {{ model: dep-opus, effort: high }}\n    query: {{ model: dep-haiku, effort: low }}\n    fix:   {{ model: dep-sonnet, effort: medium }}\npii\n  scanner: regex_basic\n",
    );
    std::fs::write(cfg_dir.join("config.yaml"), broken).unwrap();

    let out = Command::new(BIN)
        .args(["config", "delete-key", "azure"])
        .env("CODEBUS_HOME", home.path())
        .output()
        .expect("run codebus config");
    assert!(
        !out.status.success(),
        "delete-key must exit non-zero on yaml syntax error"
    );

    let after = probe_keyring_only(&service).expect("probe keyring");
    assert_eq!(
        after.as_deref(),
        Some("sentinel-survives"),
        "keyring entry was deleted despite broken yaml — the fallback bug is back"
    );

    // Cleanup.
    let _ = delete_azure_key(&service);
}

// === Section D: forward-compat sanity — unknown key does NOT fail-loud ===

#[test]
fn legal_yaml_with_unknown_key_does_not_fail_loud() {
    let _g = serial_lock();
    let legal_with_unknown = "claude_code:\n  active: system\n  system:\n    goal:  { model: opus-4-6,   effort: high }\n    query: { model: haiku-4-5,  effort: low }\n    fix:   { model: sonnet-4-6, effort: medium }\nfuture_section:\n  knob: 42\n";
    let (home, mock_log) = setup_with_config(legal_with_unknown);
    let repo = prepare_clean_vault(home.path());

    let out = Command::new(BIN)
        .args(["query", "ping"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        .env("CODEBUS_MOCK_BEHAVIOR", "success-noop")
        .current_dir(repo.path())
        .output()
        .expect("run codebus query");

    assert!(
        out.status.success(),
        "query SHOULD succeed when yaml is valid + has unknown keys; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        mock_log.exists(),
        "claude was NOT spawned on legal config — forward-compat regression"
    );
}

// === Helpers ===

fn setup_with_config(yaml_body: &str) -> (TempDir, PathBuf) {
    let home = TempDir::new().unwrap();
    let cfg_dir = home.path().join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(cfg_dir.join("config.yaml"), yaml_body).unwrap();
    let mock_log = home.path().join("mock.log");
    (home, mock_log)
}

/// Prepare a fresh repo with a working `.codebus/` vault so the verbs'
/// vault precondition is satisfied. We init via `codebus init` against
/// a SEPARATE home dir so the init step doesn't see the broken config
/// we're testing. Then the verb call uses the broken-config home.
fn prepare_clean_vault(_broken_home_unused: &std::path::Path) -> TempDir {
    let repo = TempDir::new().unwrap();
    std::fs::write(repo.path().join("README.md"), b"# hello").unwrap();
    let init_home = TempDir::new().unwrap();
    let init_out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", init_home.path())
        .current_dir(repo.path())
        .output()
        .expect("run codebus init");
    assert!(
        init_out.status.success(),
        "init must succeed against clean config; stderr: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );
    // Keep init_home alive for the duration of the test — drop after
    // the repo by leaking. TempDir is RAII; we leak by mem::forget so
    // the init artifacts stay on disk while the verb under test runs.
    std::mem::forget(init_home);
    repo
}

fn unique_service() -> String {
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("codebus-test-parse-{pid}-{ts}")
}

fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}
