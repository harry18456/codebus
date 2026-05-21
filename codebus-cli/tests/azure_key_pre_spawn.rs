//! Integration test for spec `OS Keyring Integration With Env Fallback`
//! Scenario "Neither source available aborts before spawn".
//!
//! Drives the `query` verb with an azure-profile config whose keyring
//! entry doesn't exist AND `CODEBUS_AZURE_KEY` env var is unset. Spawn
//! SHALL NOT happen: we verify by pointing `CODEBUS_CLAUDE_BIN` at the
//! `mock-claude` test binary with a side-effect (writing a log file)
//! and asserting the log is absent.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

/// Spec: Azure profile is active, no key resolvable from either source
/// → exit non-zero AND child process is NOT spawned. We assert "child
/// not spawned" via the mock-claude side-effect: when mock-claude runs,
/// it writes `CODEBUS_MOCK_LOG`; if the file is absent post-invocation
/// the child never executed.
#[test]
fn azure_profile_missing_key_aborts_before_spawn() {
    let _guard = serial_lock();

    let home = TempDir::new().unwrap();
    write_azure_active_config(home.path());

    // Create a working vault (query verb's vault precondition).
    let repo = TempDir::new().unwrap();
    let init_out = Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo.path())
        .output()
        .expect("init");
    assert!(
        init_out.status.success(),
        "init must succeed first: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    // Init wrote the starter config under CODEBUS_HOME; overwrite it
    // with our azure-active config (the starter is system-active).
    write_azure_active_config(home.path());

    let mock_log: PathBuf = home.path().join("mock.log");
    assert!(!mock_log.exists(), "pre-condition: mock log must not exist");

    let prev_env = std::env::var("CODEBUS_AZURE_KEY").ok();
    unsafe {
        std::env::remove_var("CODEBUS_AZURE_KEY");
    }

    // Run the query verb with no API key reachable. Point
    // CODEBUS_CLAUDE_BIN at mock-claude so we can detect a spawn that
    // shouldn't happen.
    let out = Command::new(BIN)
        .args(["query", "ping"])
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_MOCK_LOG", &mock_log)
        // Ensure absolutely no inherited value leaks in.
        .env_remove("CODEBUS_AZURE_KEY")
        .current_dir(repo.path())
        .output()
        .expect("run codebus query");

    // Restore env state.
    unsafe {
        match prev_env {
            Some(v) => std::env::set_var("CODEBUS_AZURE_KEY", v),
            None => { /* already unset */ }
        }
    }

    assert!(
        !out.status.success(),
        "query must exit non-zero on missing key"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("EndpointKeyMissing"),
        "stderr should name EndpointKeyMissing, got: {stderr}"
    );
    // Spec: spawn counter = 0. mock-claude writes mock.log when it runs;
    // its absence is the strongest available signal that the spawn was
    // skipped.
    assert!(
        !mock_log.exists(),
        "spawn happened — mock-claude wrote {mock_log:?}; key-missing should abort before spawn"
    );
}

fn write_azure_active_config(home: &std::path::Path) {
    let cfg_dir = home.join(".codebus");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    let cfg_path = cfg_dir.join("config.yaml");
    let service = unique_service();
    let body = format!(
        "agent:\n  active_provider: claude\n  providers:\n    claude:\n      active: azure\n      system:\n        goal:   {{ model: opus-4-6,   effort: high   }}\n        query:  {{ model: haiku-4-5,  effort: low    }}\n        fix:    {{ model: sonnet-4-6, effort: medium }}\n        verify: {{ model: opus-4-6,   effort: high   }}\n      azure:\n        base_url: https://placeholder.example.com/anthropic\n        keyring_service: {service}\n        goal:   {{ model: dep-opus,   effort: high   }}\n        query:  {{ model: dep-haiku,  effort: low    }}\n        fix:    {{ model: dep-sonnet, effort: medium }}\n        verify: {{ model: dep-opus,   effort: high   }}\n"
    );
    std::fs::write(cfg_path, body).unwrap();
}

fn unique_service() -> String {
    let pid = std::process::id();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("codebus-test-missing-{pid}-{ts}")
}

fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}
