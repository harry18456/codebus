//! goal-content-verify task 5.1 (design D6; spec `cli` / Goal Content
//! Verify CLI Behavior). Asserts the `codebus goal` thin wrapper
//! resolves `goal.content_verify` from the shared `goal.*` config and
//! threads the originating goal text into `run_goal` (so the off-goal
//! check can run), without adding any new top-level subcommand.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_codebus");
const MOCK_CLAUDE: &str = env!("CARGO_BIN_EXE_mock-claude");

fn run_init(repo: &Path) -> Output {
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    Command::new(BIN)
        .args(["init", "--no-obsidian-register"])
        .env("CODEBUS_HOME", home.path())
        .current_dir(repo)
        .output()
        .expect("run codebus init")
}

fn run_goal_cfg(repo: &Path, goal_text: &str, behavior: &str, cfg: Option<&str>) -> Output {
    let log = repo.join("mock-claude.log");
    let _ = fs::remove_file(&log);
    let home = TempDir::new().expect("isolated CODEBUS_HOME");
    if let Some(body) = cfg {
        let cb = home.path().join(".codebus");
        fs::create_dir_all(&cb).unwrap();
        fs::write(cb.join("config.yaml"), body).unwrap();
    }
    Command::new(BIN)
        .args(["--no-obsidian-register", "--no-fix", "goal", goal_text])
        .current_dir(repo)
        .env("CODEBUS_CLAUDE_BIN", MOCK_CLAUDE)
        .env("CODEBUS_HOME", home.path())
        .env("CODEBUS_MOCK_BEHAVIOR", behavior)
        .env("CODEBUS_MOCK_LOG", &log)
        .output()
        .expect("run codebus goal")
}

/// Spec scenario: No new subcommand is registered.
#[test]
fn goal_help_registers_no_content_verify_subcommand() {
    let out = Command::new(BIN).arg("--help").output().expect("codebus --help");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("goal"), "goal subcommand must still exist:\n{help}");
    assert!(
        !help.to_lowercase().contains("content-verify")
            && !help.to_lowercase().contains("content_verify"),
        "no content-verify subcommand may be registered:\n{help}"
    );
}

/// Spec scenario: Enabled CLI runs the stage and surfaces the stream —
/// the CLI resolves `goal.content_verify: true` and threads the
/// originating goal text into `run_goal` (visible in the verify spawn's
/// `goal=<text>` prompt). RED until the CLI resolves the config.
#[test]
fn goal_config_true_threads_goal_text_into_verify_spawn() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());
    let out = run_goal_cfg(
        tmp.path(),
        "describe auth",
        "goal-verify-clean",
        Some("goal:\n  content_verify: true\n"),
    );
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dump = fs::read_to_string(tmp.path().join("mock-claude.log")).unwrap_or_default();
    assert!(
        dump.contains("/codebus-goal verify: goal=describe auth"),
        "CLI must resolve config AND thread the originating goal text into the verify spawn:\n{dump}"
    );
}

/// Spec scenario: Default off leaves CLI flow unchanged — absent config
/// means no verify spawn and the existing exit / auto_commit behavior.
#[test]
fn goal_config_absent_leaves_flow_unchanged() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("README.md"), b"# h").unwrap();
    assert!(run_init(tmp.path()).status.success());
    let out = run_goal_cfg(tmp.path(), "describe auth", "goal-verify-flag", None);
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let dump = fs::read_to_string(tmp.path().join("mock-claude.log")).unwrap_or_default();
    assert!(
        !dump.contains("/codebus-goal verify:"),
        "no verify spawn without goal.content_verify:\n{dump}"
    );
}
