//! Integration tests for spec `Scoped Environment Injection At Spawn`.
//!
//! Spawn `claude` via the `mock-claude` test binary (selected through
//! `CODEBUS_CLAUDE_BIN`), have it dump the three azure env vars it
//! received to a log file, then assert:
//!
//! 1. Azure variant injection puts all three vars in the child env.
//! 2. The parent process env is NOT modified (no leak from
//!    `Command::envs`).

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use codebus_core::agent::{EnvOverrides, InvokeAgentOptions, invoke};
use tempfile::TempDir;

const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Spec: `for_azure` injects 3 env vars and the child sees them all.
#[test]
fn invoke_passes_env_overrides_to_command() {
    let _guard = serial_lock();

    // Pre-condition: parent shell has neither var set so we can detect
    // any pollution caused by `Command::env`. Save + restore.
    let saved_url = scoped_var_take("ANTHROPIC_BASE_URL");
    let saved_key = scoped_var_take("ANTHROPIC_API_KEY");
    let saved_dis = scoped_var_take("CLAUDE_CODE_DISABLE_ADVISOR_TOOL");

    let tmp = TempDir::new().unwrap();
    let log_path: PathBuf = tmp.path().join("mock.log");
    unsafe {
        std::env::set_var("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE);
        std::env::set_var("CODEBUS_MOCK_LOG", &log_path);
    }

    let report = invoke(
        InvokeAgentOptions {
            slash_command: "/codebus-query \"ping\"".into(),
            vault_root: tmp.path().to_path_buf(),
            toolset: &["Read"],
            bash_whitelist: None,
            model: Some("dep-opus".into()),
            effort: Some("high".into()),
            env: EnvOverrides::for_azure(
                "https://example.cognitiveservices.azure.com/anthropic",
                "sk-injection-test",
            ),
        },
        |_event| {},
        None,
    )
    .expect("invoke spawn-and-wait succeeds against mock-claude");
    assert!(report.exit.success(), "mock-claude should exit 0");

    let log = fs::read_to_string(&log_path).expect("mock-claude wrote log");
    assert!(
        log.contains(
            "env_ANTHROPIC_BASE_URL=https://example.cognitiveservices.azure.com/anthropic"
        ),
        "child missing ANTHROPIC_BASE_URL injection:\n{log}"
    );
    assert!(
        log.contains("env_ANTHROPIC_API_KEY=sk-injection-test"),
        "child missing ANTHROPIC_API_KEY injection:\n{log}"
    );
    assert!(
        log.contains("env_CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1"),
        "child missing CLAUDE_CODE_DISABLE_ADVISOR_TOOL=1 injection:\n{log}"
    );

    // Spec: parent shell env unchanged.
    assert!(
        std::env::var("ANTHROPIC_BASE_URL").is_err(),
        "ANTHROPIC_BASE_URL leaked into parent env"
    );
    assert!(
        std::env::var("ANTHROPIC_API_KEY").is_err(),
        "ANTHROPIC_API_KEY leaked into parent env"
    );
    assert!(
        std::env::var("CLAUDE_CODE_DISABLE_ADVISOR_TOOL").is_err(),
        "CLAUDE_CODE_DISABLE_ADVISOR_TOOL leaked into parent env"
    );

    // Restore.
    unsafe {
        std::env::remove_var("CODEBUS_CLAUDE_BIN");
        std::env::remove_var("CODEBUS_MOCK_LOG");
    }
    scoped_var_restore("ANTHROPIC_BASE_URL", saved_url);
    scoped_var_restore("ANTHROPIC_API_KEY", saved_key);
    scoped_var_restore("CLAUDE_CODE_DISABLE_ADVISOR_TOOL", saved_dis);
}

/// Spec: System profile injects no env (parent env still wins for the
/// child's view of ANTHROPIC_API_KEY).
#[test]
fn for_system_does_not_inject_env() {
    let _guard = serial_lock();

    let tmp = TempDir::new().unwrap();
    let log_path: PathBuf = tmp.path().join("mock.log");
    unsafe {
        std::env::set_var("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE);
        std::env::set_var("CODEBUS_MOCK_LOG", &log_path);
    }

    let report = invoke(
        InvokeAgentOptions {
            slash_command: "/x".into(),
            vault_root: tmp.path().to_path_buf(),
            toolset: &["Read"],
            bash_whitelist: None,
            model: None,
            effort: None,
            env: EnvOverrides::for_system(),
        },
        |_event| {},
        None,
    )
    .expect("invoke spawn-and-wait succeeds");
    assert!(report.exit.success());

    let log = fs::read_to_string(&log_path).expect("mock-claude wrote log");
    // None of the three azure env vars should appear with values we set
    // (system profile never calls `cmd.env(...)` for them). Parent shell
    // env may still legitimately leak them in if the developer running
    // tests has them exported — that's expected inheritance behavior.
    // We assert the codebus-injected sentinel value is absent.
    assert!(
        !log.contains("sk-injection-test"),
        "system profile leaked an api key value into child:\n{log}"
    );

    unsafe {
        std::env::remove_var("CODEBUS_CLAUDE_BIN");
        std::env::remove_var("CODEBUS_MOCK_LOG");
    }
}

// ---------------------------------------------------------------------------
// Test serialisation helpers
// ---------------------------------------------------------------------------

/// `cargo test` parallelises by default. Both tests in this file mutate
/// process-wide env vars (`CODEBUS_CLAUDE_BIN`, `CODEBUS_MOCK_LOG`) so
/// they SHALL NOT run concurrently with each other.
fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn scoped_var_take(name: &str) -> Option<String> {
    let v = std::env::var(name).ok();
    unsafe { std::env::remove_var(name) };
    v
}

fn scoped_var_restore(name: &str, value: Option<String>) {
    unsafe {
        match value {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }
}
